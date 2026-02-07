"""
ONNX 모델 훈련 스크립트.

Yahoo Finance 데이터로 ML 모델을 훈련하고 ONNX 형식으로 내보냅니다.
훈련된 모델은 Rust의 OnnxPredictor에서 사용할 수 있습니다.

사용 예시:
    # 기본 훈련 (XGBoost)
    python train_model.py --symbol SPY --model xgboost

    # Random Forest
    python train_model.py --symbol QQQ --model random_forest --period 5y

    # LightGBM with hyperparameter tuning
    python train_model.py --symbol AAPL --model lightgbm --tune

    # 여러 심볼로 훈련
    python train_model.py --symbols SPY,QQQ,IWM --model xgboost
"""

import argparse
import os
import sys
from pathlib import Path
from datetime import datetime
from typing import Optional, List, Dict, Any, Tuple
import json
import logging
import warnings

import numpy as np
import pandas as pd
import joblib

# ML 라이브러리
from sklearn.model_selection import train_test_split, cross_val_score, TimeSeriesSplit
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import accuracy_score, classification_report, roc_auc_score
from sklearn.ensemble import RandomForestClassifier, GradientBoostingClassifier

# XGBoost / LightGBM
try:
    import xgboost as xgb
    HAS_XGBOOST = True
except ImportError:
    HAS_XGBOOST = False

try:
    import lightgbm as lgb
    HAS_LIGHTGBM = True
except ImportError:
    HAS_LIGHTGBM = False

# ONNX 변환
import onnx
from skl2onnx import convert_sklearn
from skl2onnx.common.data_types import FloatTensorType

try:
    from onnxmltools import convert_xgboost, convert_lightgbm
    from onnxmltools.convert.common.data_types import FloatTensorType as OnnxFloatTensorType
    HAS_ONNXMLTOOLS = True
except ImportError:
    HAS_ONNXMLTOOLS = False

# ONNX Runtime (검증용)
import onnxruntime as ort

# 로컬 모듈
from data_fetcher import YahooDataFetcher
from feature_engineering import FeatureEngineer

warnings.filterwarnings("ignore")
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s")
logger = logging.getLogger(__name__)


class ModelTrainer:
    """ML 모델 훈련기."""

    SUPPORTED_MODELS = ["random_forest", "gradient_boosting", "xgboost", "lightgbm"]

    def __init__(
        self,
        output_dir: str = "./models",
        random_state: int = 42,
    ):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.random_state = random_state
        self.scaler = StandardScaler()
        self.feature_names: List[str] = []

    def load_data(
        self,
        symbols: List[str],
        period: str = "5y",
        target_horizon: int = 5,
        target_type: str = "direction",
    ) -> Tuple[np.ndarray, np.ndarray]:
        """데이터 로드 및 전처리."""
        logger.info(f"Loading data for {symbols}...")

        fetcher = YahooDataFetcher(cache_dir="./data_cache")
        engineer = FeatureEngineer()

        all_X = []
        all_y = []

        for symbol in symbols:
            try:
                df = fetcher.fetch(symbol, period=period)
                if df.empty:
                    logger.warning(f"No data for {symbol}, skipping")
                    continue

                features_df = engineer.extract_features(df)
                X, y, feature_names = engineer.prepare_ml_data(
                    features_df,
                    target_horizon=target_horizon,
                    target_type=target_type,
                )

                all_X.append(X)
                all_y.append(y)

                if not self.feature_names:
                    self.feature_names = feature_names

                logger.info(f"{symbol}: {X.shape[0]} samples")

            except Exception as e:
                logger.error(f"Error loading {symbol}: {e}")
                continue

        if not all_X:
            raise ValueError("No data loaded")

        X = np.vstack(all_X)
        y = np.hstack(all_y)

        logger.info(f"Total samples: {X.shape[0]}, Features: {X.shape[1]}")

        return X, y

    def train(
        self,
        X: np.ndarray,
        y: np.ndarray,
        model_type: str = "xgboost",
        test_size: float = 0.2,
        tune_hyperparams: bool = False,
    ) -> Any:
        """모델 훈련."""
        logger.info(f"Training {model_type} model...")

        # Train/Test 분할 (시계열이므로 시간순 분할)
        split_idx = int(len(X) * (1 - test_size))
        X_train, X_test = X[:split_idx], X[split_idx:]
        y_train, y_test = y[:split_idx], y[split_idx:]

        # 스케일링
        X_train_scaled = self.scaler.fit_transform(X_train)
        X_test_scaled = self.scaler.transform(X_test)

        # 모델 생성
        model = self._create_model(model_type, tune_hyperparams, X_train_scaled, y_train)

        # 훈련
        model.fit(X_train_scaled, y_train)

        # 평가
        y_pred = model.predict(X_test_scaled)
        y_prob = model.predict_proba(X_test_scaled) if hasattr(model, "predict_proba") else None

        accuracy = accuracy_score(y_test, y_pred)
        logger.info(f"Test Accuracy: {accuracy:.4f}")

        if y_prob is not None and len(np.unique(y_test)) == 2:
            auc = roc_auc_score(y_test, y_prob[:, 1])
            logger.info(f"Test AUC: {auc:.4f}")

        logger.info(f"\nClassification Report:\n{classification_report(y_test, y_pred)}")

        # Cross-validation (시계열 분할)
        tscv = TimeSeriesSplit(n_splits=5)
        cv_scores = cross_val_score(model, X_train_scaled, y_train, cv=tscv, scoring="accuracy")
        logger.info(f"CV Accuracy: {cv_scores.mean():.4f} (+/- {cv_scores.std() * 2:.4f})")

        return model

    def _create_model(
        self,
        model_type: str,
        tune: bool,
        X_train: np.ndarray,
        y_train: np.ndarray,
    ) -> Any:
        """모델 인스턴스 생성."""
        if model_type == "random_forest":
            params = {
                "n_estimators": 200,
                "max_depth": 10,
                "min_samples_split": 5,
                "min_samples_leaf": 2,
                "random_state": self.random_state,
                "n_jobs": -1,
            }
            return RandomForestClassifier(**params)

        elif model_type == "gradient_boosting":
            params = {
                "n_estimators": 200,
                "max_depth": 5,
                "learning_rate": 0.1,
                "random_state": self.random_state,
            }
            return GradientBoostingClassifier(**params)

        elif model_type == "xgboost":
            if not HAS_XGBOOST:
                raise ImportError("XGBoost not installed. Install with: pip install xgboost")
            params = {
                "n_estimators": 200,
                "max_depth": 6,
                "learning_rate": 0.1,
                "subsample": 0.8,
                "colsample_bytree": 0.8,
                "random_state": self.random_state,
                "use_label_encoder": False,
                "eval_metric": "logloss",
            }
            return xgb.XGBClassifier(**params)

        elif model_type == "lightgbm":
            if not HAS_LIGHTGBM:
                raise ImportError("LightGBM not installed. Install with: pip install lightgbm")
            params = {
                "n_estimators": 200,
                "max_depth": 6,
                "learning_rate": 0.1,
                "subsample": 0.8,
                "colsample_bytree": 0.8,
                "random_state": self.random_state,
                "verbose": -1,
            }
            return lgb.LGBMClassifier(**params)

        else:
            raise ValueError(f"Unknown model type: {model_type}. Supported: {self.SUPPORTED_MODELS}")

    def export_to_onnx(
        self,
        model: Any,
        model_type: str,
        model_name: str,
    ) -> str:
        """모델을 ONNX 형식으로 내보내기."""
        n_features = len(self.feature_names)
        onnx_path = self.output_dir / f"{model_name}.onnx"

        logger.info(f"Exporting to ONNX: {onnx_path}")

        if model_type in ["random_forest", "gradient_boosting"]:
            # sklearn 모델 변환
            initial_type = [("float_input", FloatTensorType([None, n_features]))]
            onnx_model = convert_sklearn(
                model,
                initial_types=initial_type,
                target_opset=12,
            )

        elif model_type == "xgboost":
            if not HAS_ONNXMLTOOLS:
                raise ImportError("onnxmltools not installed. Install with: pip install onnxmltools")
            initial_type = [("float_input", OnnxFloatTensorType([None, n_features]))]
            onnx_model = convert_xgboost(
                model,
                initial_types=initial_type,
                target_opset=12,
            )

        elif model_type == "lightgbm":
            if not HAS_ONNXMLTOOLS:
                raise ImportError("onnxmltools not installed. Install with: pip install onnxmltools")
            initial_type = [("float_input", OnnxFloatTensorType([None, n_features]))]
            onnx_model = convert_lightgbm(
                model,
                initial_types=initial_type,
                target_opset=12,
            )

        else:
            raise ValueError(f"ONNX export not supported for {model_type}")

        # 저장
        onnx.save_model(onnx_model, str(onnx_path))
        logger.info(f"ONNX model saved to {onnx_path}")

        # 검증
        self._verify_onnx(str(onnx_path), n_features)

        return str(onnx_path)

    def _verify_onnx(self, onnx_path: str, n_features: int) -> None:
        """ONNX 모델 검증."""
        logger.info("Verifying ONNX model...")

        # 모델 체크
        onnx_model = onnx.load(onnx_path)
        onnx.checker.check_model(onnx_model)

        # 추론 테스트
        session = ort.InferenceSession(onnx_path)
        input_name = session.get_inputs()[0].name

        # 더미 데이터로 테스트
        test_input = np.random.randn(1, n_features).astype(np.float32)
        outputs = session.run(None, {input_name: test_input})

        logger.info(f"ONNX verification passed. Output shape: {outputs[0].shape}")

    def save_metadata(
        self,
        model_name: str,
        model_type: str,
        symbols: List[str],
        metrics: Dict[str, float],
    ) -> str:
        """모델 메타데이터 저장."""
        metadata = {
            "model_name": model_name,
            "model_type": model_type,
            "symbols": symbols,
            "feature_names": self.feature_names,
            "n_features": len(self.feature_names),
            "metrics": metrics,
            "created_at": datetime.now().isoformat(),
            "scaler_path": f"{model_name}_scaler.joblib",
        }

        metadata_path = self.output_dir / f"{model_name}_metadata.json"
        with open(metadata_path, "w") as f:
            json.dump(metadata, f, indent=2)

        # 스케일러 저장
        scaler_path = self.output_dir / f"{model_name}_scaler.joblib"
        joblib.dump(self.scaler, scaler_path)

        logger.info(f"Metadata saved to {metadata_path}")
        return str(metadata_path)


def main():
    parser = argparse.ArgumentParser(description="Train ML model and export to ONNX")
    parser.add_argument("--symbol", type=str, help="Single symbol to train on")
    parser.add_argument("--symbols", type=str, help="Comma-separated symbols (e.g., SPY,QQQ,IWM)")
    parser.add_argument("--model", type=str, default="xgboost", choices=ModelTrainer.SUPPORTED_MODELS)
    parser.add_argument("--period", type=str, default="5y", help="Data period (1y, 2y, 5y, 10y, max)")
    parser.add_argument("--horizon", type=int, default=5, help="Prediction horizon (days)")
    parser.add_argument("--output-dir", type=str, default="./models", help="Output directory")
    parser.add_argument("--tune", action="store_true", help="Enable hyperparameter tuning")
    parser.add_argument("--name", type=str, help="Model name (default: auto-generated)")

    args = parser.parse_args()

    # 심볼 처리
    if args.symbol:
        symbols = [args.symbol]
    elif args.symbols:
        symbols = [s.strip() for s in args.symbols.split(",")]
    else:
        # 기본 심볼
        symbols = ["SPY"]

    # 모델 이름
    if args.name:
        model_name = args.name
    else:
        symbol_str = "_".join(symbols[:3])
        model_name = f"{args.model}_{symbol_str}_{args.period}"

    logger.info(f"Training model: {model_name}")
    logger.info(f"Symbols: {symbols}")
    logger.info(f"Model type: {args.model}")
    logger.info(f"Period: {args.period}")
    logger.info(f"Horizon: {args.horizon} days")

    # 훈련
    trainer = ModelTrainer(output_dir=args.output_dir)

    # 데이터 로드
    X, y = trainer.load_data(symbols, period=args.period, target_horizon=args.horizon)

    # 모델 훈련
    model = trainer.train(X, y, model_type=args.model, tune_hyperparams=args.tune)

    # ONNX 내보내기
    onnx_path = trainer.export_to_onnx(model, args.model, model_name)

    # 메타데이터 저장
    trainer.save_metadata(
        model_name=model_name,
        model_type=args.model,
        symbols=symbols,
        metrics={"placeholder": 0.0},  # TODO: 실제 메트릭
    )

    logger.info(f"\n{'=' * 50}")
    logger.info(f"Training complete!")
    logger.info(f"ONNX model: {onnx_path}")
    logger.info(f"To use in Rust: Copy to crates/trader-analytics/models/")
    logger.info(f"{'=' * 50}")


if __name__ == "__main__":
    main()
