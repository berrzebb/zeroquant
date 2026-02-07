"""
ML 모델 훈련 및 ONNX 변환 모듈.

지원 모델:
- XGBoost
- LightGBM
- Random Forest (sklearn)
"""

import json
import os
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Optional

import numpy as np
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import accuracy_score, classification_report, f1_score
from sklearn.model_selection import train_test_split

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

try:
    import onnx
    from onnxmltools import convert_xgboost, convert_lightgbm
    from onnxmltools.convert.common.data_types import FloatTensorType
    from skl2onnx import convert_sklearn
    from skl2onnx.common.data_types import FloatTensorType as SklearnFloatTensorType
    HAS_ONNX = True
except ImportError:
    HAS_ONNX = False


class ModelType(str, Enum):
    """지원하는 모델 타입."""
    XGBOOST = "xgboost"
    LIGHTGBM = "lightgbm"
    RANDOM_FOREST = "random_forest"


@dataclass
class TrainingConfig:
    """훈련 설정."""
    model_type: ModelType = ModelType.XGBOOST
    test_size: float = 0.2
    random_state: int = 42
    n_estimators: int = 100
    max_depth: int = 6
    learning_rate: float = 0.1
    early_stopping_rounds: int = 10
    verbose: bool = True

    # XGBoost 전용
    xgb_params: dict = field(default_factory=lambda: {
        "objective": "multi:softmax",
        "num_class": 3,
        "eval_metric": "mlogloss",
    })

    # LightGBM 전용
    lgb_params: dict = field(default_factory=lambda: {
        "objective": "multiclass",
        "num_class": 3,
        "metric": "multi_logloss",
        "verbosity": -1,
    })


@dataclass
class TrainingResult:
    """훈련 결과."""
    model_type: str
    accuracy: float
    f1_score: float
    classification_report: str
    feature_importance: dict[str, float]
    training_time: float
    model_path: Optional[str] = None
    onnx_path: Optional[str] = None


class ModelTrainer:
    """ML 모델 훈련기."""

    def __init__(
        self,
        config: Optional[TrainingConfig] = None,
        output_dir: str = "models",
    ):
        self.config = config or TrainingConfig()
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

        self.model = None
        self.feature_names: list[str] = []

    def train(
        self,
        X: np.ndarray,
        y: np.ndarray,
        feature_names: Optional[list[str]] = None,
    ) -> TrainingResult:
        """
        모델 훈련.

        Args:
            X: Feature 행렬 (n_samples, n_features)
            y: 라벨 배열 (n_samples,)
            feature_names: Feature 이름 리스트

        Returns:
            TrainingResult: 훈련 결과
        """
        self.feature_names = feature_names or [f"feature_{i}" for i in range(X.shape[1])]

        # 데이터 분할
        X_train, X_test, y_train, y_test = train_test_split(
            X, y,
            test_size=self.config.test_size,
            random_state=self.config.random_state,
            stratify=y,
        )

        start_time = datetime.now()

        # 모델 훈련
        if self.config.model_type == ModelType.XGBOOST:
            self._train_xgboost(X_train, y_train, X_test, y_test)
        elif self.config.model_type == ModelType.LIGHTGBM:
            self._train_lightgbm(X_train, y_train, X_test, y_test)
        else:
            self._train_random_forest(X_train, y_train)

        training_time = (datetime.now() - start_time).total_seconds()

        # 평가
        y_pred = self.predict(X_test)
        accuracy = accuracy_score(y_test, y_pred)
        f1 = f1_score(y_test, y_pred, average="weighted")
        report = classification_report(y_test, y_pred)

        # Feature 중요도
        importance = self._get_feature_importance()

        if self.config.verbose:
            print(f"\n=== Training Results ({self.config.model_type.value}) ===")
            print(f"Accuracy: {accuracy:.4f}")
            print(f"F1 Score: {f1:.4f}")
            print(f"Training time: {training_time:.2f}s")
            print(f"\n{report}")

        return TrainingResult(
            model_type=self.config.model_type.value,
            accuracy=accuracy,
            f1_score=f1,
            classification_report=report,
            feature_importance=importance,
            training_time=training_time,
        )

    def predict(self, X: np.ndarray) -> np.ndarray:
        """예측."""
        if self.model is None:
            raise ValueError("Model not trained. Call train() first.")

        if self.config.model_type == ModelType.XGBOOST:
            dmatrix = xgb.DMatrix(X)
            return self.model.predict(dmatrix).astype(int)
        elif self.config.model_type == ModelType.LIGHTGBM:
            proba = self.model.predict(X)
            return np.argmax(proba, axis=1)
        else:
            return self.model.predict(X)

    def predict_proba(self, X: np.ndarray) -> np.ndarray:
        """확률 예측."""
        if self.model is None:
            raise ValueError("Model not trained. Call train() first.")

        if self.config.model_type == ModelType.XGBOOST:
            dmatrix = xgb.DMatrix(X)
            # XGBoost는 predict로 클래스만 반환하므로 predict_proba 사용
            # multi:softprob 목표 함수 필요
            return self.model.predict(dmatrix)
        elif self.config.model_type == ModelType.LIGHTGBM:
            return self.model.predict(X)
        else:
            return self.model.predict_proba(X)

    def save_model(self, name: str) -> tuple[str, Optional[str]]:
        """
        모델 저장 (네이티브 + ONNX).

        Args:
            name: 모델 이름

        Returns:
            (native_path, onnx_path) 튜플
        """
        if self.model is None:
            raise ValueError("Model not trained. Call train() first.")

        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        model_name = f"{name}_{timestamp}"

        # 네이티브 형식 저장
        native_path = self._save_native(model_name)

        # ONNX 변환 및 저장
        onnx_path = None
        if HAS_ONNX:
            onnx_path = self._save_onnx(model_name)

        # 메타데이터 저장
        self._save_metadata(model_name, native_path, onnx_path)

        return native_path, onnx_path

    def _train_xgboost(
        self,
        X_train: np.ndarray,
        y_train: np.ndarray,
        X_test: np.ndarray,
        y_test: np.ndarray,
    ) -> None:
        """XGBoost 훈련."""
        if not HAS_XGBOOST:
            raise ImportError("xgboost is not installed")

        dtrain = xgb.DMatrix(X_train, label=y_train)
        dtest = xgb.DMatrix(X_test, label=y_test)

        params = {
            **self.config.xgb_params,
            "max_depth": self.config.max_depth,
            "learning_rate": self.config.learning_rate,
        }

        self.model = xgb.train(
            params,
            dtrain,
            num_boost_round=self.config.n_estimators,
            evals=[(dtest, "test")],
            early_stopping_rounds=self.config.early_stopping_rounds,
            verbose_eval=self.config.verbose,
        )

    def _train_lightgbm(
        self,
        X_train: np.ndarray,
        y_train: np.ndarray,
        X_test: np.ndarray,
        y_test: np.ndarray,
    ) -> None:
        """LightGBM 훈련."""
        if not HAS_LIGHTGBM:
            raise ImportError("lightgbm is not installed")

        train_data = lgb.Dataset(X_train, label=y_train)
        valid_data = lgb.Dataset(X_test, label=y_test, reference=train_data)

        params = {
            **self.config.lgb_params,
            "max_depth": self.config.max_depth,
            "learning_rate": self.config.learning_rate,
            "n_estimators": self.config.n_estimators,
        }

        self.model = lgb.train(
            params,
            train_data,
            valid_sets=[valid_data],
            callbacks=[
                lgb.early_stopping(self.config.early_stopping_rounds),
            ] if self.config.early_stopping_rounds > 0 else None,
        )

    def _train_random_forest(
        self,
        X_train: np.ndarray,
        y_train: np.ndarray,
    ) -> None:
        """Random Forest 훈련."""
        self.model = RandomForestClassifier(
            n_estimators=self.config.n_estimators,
            max_depth=self.config.max_depth,
            random_state=self.config.random_state,
            n_jobs=-1,
        )
        self.model.fit(X_train, y_train)

    def _get_feature_importance(self) -> dict[str, float]:
        """Feature 중요도 반환."""
        if self.model is None:
            return {}

        if self.config.model_type == ModelType.XGBOOST:
            importance = self.model.get_score(importance_type="weight")
            # Feature 이름 매핑
            result = {}
            for i, name in enumerate(self.feature_names):
                key = f"f{i}"
                result[name] = importance.get(key, 0.0)
            return result
        elif self.config.model_type == ModelType.LIGHTGBM:
            importance = self.model.feature_importance()
            return dict(zip(self.feature_names, importance.tolist()))
        else:
            importance = self.model.feature_importances_
            return dict(zip(self.feature_names, importance.tolist()))

    def _save_native(self, model_name: str) -> str:
        """네이티브 형식으로 저장."""
        if self.config.model_type == ModelType.XGBOOST:
            path = self.output_dir / f"{model_name}.xgb"
            self.model.save_model(str(path))
        elif self.config.model_type == ModelType.LIGHTGBM:
            path = self.output_dir / f"{model_name}.lgb"
            self.model.save_model(str(path))
        else:
            import joblib
            path = self.output_dir / f"{model_name}.joblib"
            joblib.dump(self.model, str(path))

        return str(path)

    def _save_onnx(self, model_name: str) -> Optional[str]:
        """ONNX 형식으로 저장."""
        if not HAS_ONNX:
            print("Warning: onnx/onnxmltools not installed, skipping ONNX export")
            return None

        path = self.output_dir / f"{model_name}.onnx"
        n_features = len(self.feature_names)

        try:
            if self.config.model_type == ModelType.XGBOOST:
                initial_type = [("input", FloatTensorType([None, n_features]))]
                onnx_model = convert_xgboost(self.model, initial_types=initial_type)
            elif self.config.model_type == ModelType.LIGHTGBM:
                initial_type = [("input", FloatTensorType([None, n_features]))]
                onnx_model = convert_lightgbm(self.model, initial_types=initial_type)
            else:
                initial_type = [("input", SklearnFloatTensorType([None, n_features]))]
                onnx_model = convert_sklearn(
                    self.model,
                    initial_types=initial_type,
                    target_opset=12,
                )

            onnx.save(onnx_model, str(path))
            return str(path)

        except Exception as e:
            print(f"Warning: ONNX conversion failed: {e}")
            return None

    def _save_metadata(
        self,
        model_name: str,
        native_path: str,
        onnx_path: Optional[str],
    ) -> None:
        """메타데이터 저장."""
        metadata = {
            "model_name": model_name,
            "model_type": self.config.model_type.value,
            "native_path": native_path,
            "onnx_path": onnx_path,
            "feature_names": self.feature_names,
            "feature_count": len(self.feature_names),
            "created_at": datetime.now().isoformat(),
            "config": {
                "n_estimators": self.config.n_estimators,
                "max_depth": self.config.max_depth,
                "learning_rate": self.config.learning_rate,
            },
        }

        path = self.output_dir / f"{model_name}_metadata.json"
        with open(path, "w", encoding="utf-8") as f:
            json.dump(metadata, f, indent=2)


def load_model(model_path: str, model_type: ModelType) -> Any:
    """저장된 모델 로드."""
    if model_type == ModelType.XGBOOST:
        if not HAS_XGBOOST:
            raise ImportError("xgboost is not installed")
        model = xgb.Booster()
        model.load_model(model_path)
        return model
    elif model_type == ModelType.LIGHTGBM:
        if not HAS_LIGHTGBM:
            raise ImportError("lightgbm is not installed")
        return lgb.Booster(model_file=model_path)
    else:
        import joblib
        return joblib.load(model_path)


if __name__ == "__main__":
    # 테스트: 랜덤 데이터로 훈련
    print("Testing ModelTrainer with random data...")

    # 랜덤 데이터 생성
    np.random.seed(42)
    n_samples = 1000
    n_features = 22

    X = np.random.randn(n_samples, n_features).astype(np.float32)
    y = np.random.randint(0, 3, n_samples)

    feature_names = [f"feature_{i}" for i in range(n_features)]

    # XGBoost 테스트
    if HAS_XGBOOST:
        print("\n=== XGBoost ===")
        trainer = ModelTrainer(
            config=TrainingConfig(model_type=ModelType.XGBOOST, verbose=False)
        )
        result = trainer.train(X, y, feature_names)
        print(f"Accuracy: {result.accuracy:.4f}")

        native_path, onnx_path = trainer.save_model("test_xgb")
        print(f"Saved: {native_path}")
        if onnx_path:
            print(f"ONNX: {onnx_path}")

    # LightGBM 테스트
    if HAS_LIGHTGBM:
        print("\n=== LightGBM ===")
        trainer = ModelTrainer(
            config=TrainingConfig(model_type=ModelType.LIGHTGBM, verbose=False)
        )
        result = trainer.train(X, y, feature_names)
        print(f"Accuracy: {result.accuracy:.4f}")

    # Random Forest 테스트
    print("\n=== Random Forest ===")
    trainer = ModelTrainer(
        config=TrainingConfig(model_type=ModelType.RANDOM_FOREST, verbose=False)
    )
    result = trainer.train(X, y, feature_names)
    print(f"Accuracy: {result.accuracy:.4f}")
