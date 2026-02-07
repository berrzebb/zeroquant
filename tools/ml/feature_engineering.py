"""
피처 엔지니어링 모듈.

OHLCV 데이터에서 ML 모델 훈련을 위한 기술적 지표(피처)를 추출합니다.
Rust의 FeatureExtractor와 동일한 피처를 생성하여 호환성을 보장합니다.

사용 예시:
    from feature_engineering import FeatureEngineer

    engineer = FeatureEngineer()
    features_df = engineer.extract_features(ohlcv_df)
    X, y = engineer.prepare_ml_data(features_df, target_horizon=5)
"""

import pandas as pd
import numpy as np
from typing import Optional, List, Tuple
import logging

# ta 라이브러리 임포트
try:
    import ta
    from ta.trend import SMAIndicator, EMAIndicator, MACD, ADXIndicator
    from ta.momentum import RSIIndicator, StochasticOscillator, ROCIndicator
    from ta.volatility import BollingerBands, AverageTrueRange
    from ta.volume import OnBalanceVolumeIndicator, VolumeWeightedAveragePrice
except ImportError:
    ta = None

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class FeatureEngineer:
    """
    기술적 지표 기반 피처 엔지니어링.

    Rust의 FeatureExtractor와 동일한 피처 세트를 생성합니다.
    """

    def __init__(
        self,
        # 이동평균 기간
        sma_periods: List[int] = [5, 10, 20, 50, 200],
        ema_periods: List[int] = [12, 26],
        # RSI 기간
        rsi_period: int = 14,
        # MACD 파라미터
        macd_fast: int = 12,
        macd_slow: int = 26,
        macd_signal: int = 9,
        # 볼린저 밴드
        bb_period: int = 20,
        bb_std: int = 2,
        # ATR
        atr_period: int = 14,
        # 스토캐스틱
        stoch_period: int = 14,
        stoch_smooth: int = 3,
        # ADX
        adx_period: int = 14,
    ):
        self.sma_periods = sma_periods
        self.ema_periods = ema_periods
        self.rsi_period = rsi_period
        self.macd_fast = macd_fast
        self.macd_slow = macd_slow
        self.macd_signal = macd_signal
        self.bb_period = bb_period
        self.bb_std = bb_std
        self.atr_period = atr_period
        self.stoch_period = stoch_period
        self.stoch_smooth = stoch_smooth
        self.adx_period = adx_period

    def extract_features(self, df: pd.DataFrame, dropna: bool = True) -> pd.DataFrame:
        """
        OHLCV 데이터에서 모든 피처 추출.

        Args:
            df: OHLCV 데이터프레임 (columns: Open, High, Low, Close, Volume)
            dropna: NaN 행 제거 여부

        Returns:
            피처가 추가된 데이터프레임
        """
        if ta is None:
            raise ImportError("ta library not found. Install with: pip install ta")

        result = df.copy()

        # 기본 OHLCV
        close = result["Close"]
        high = result["High"]
        low = result["Low"]
        volume = result["Volume"]

        logger.info("Extracting features...")

        # 1. 가격 변화율 (Returns)
        result["returns_1d"] = close.pct_change(1)
        result["returns_5d"] = close.pct_change(5)
        result["returns_10d"] = close.pct_change(10)
        result["returns_20d"] = close.pct_change(20)

        # 2. 로그 수익률
        result["log_returns_1d"] = np.log(close / close.shift(1))

        # 3. 이동평균 (SMA)
        for period in self.sma_periods:
            sma = SMAIndicator(close, window=period)
            result[f"sma_{period}"] = sma.sma_indicator()
            # 가격 대비 SMA 비율
            result[f"price_sma_{period}_ratio"] = close / result[f"sma_{period}"]

        # 4. 지수이동평균 (EMA)
        for period in self.ema_periods:
            ema = EMAIndicator(close, window=period)
            result[f"ema_{period}"] = ema.ema_indicator()

        # 5. MACD
        macd = MACD(close, window_slow=self.macd_slow, window_fast=self.macd_fast, window_sign=self.macd_signal)
        result["macd"] = macd.macd()
        result["macd_signal"] = macd.macd_signal()
        result["macd_histogram"] = macd.macd_diff()

        # 6. RSI
        rsi = RSIIndicator(close, window=self.rsi_period)
        result["rsi"] = rsi.rsi()

        # 7. 볼린저 밴드
        bb = BollingerBands(close, window=self.bb_period, window_dev=self.bb_std)
        result["bb_upper"] = bb.bollinger_hband()
        result["bb_middle"] = bb.bollinger_mavg()
        result["bb_lower"] = bb.bollinger_lband()
        result["bb_width"] = (result["bb_upper"] - result["bb_lower"]) / result["bb_middle"]
        result["bb_pct"] = (close - result["bb_lower"]) / (result["bb_upper"] - result["bb_lower"])

        # 8. ATR (Average True Range)
        atr = AverageTrueRange(high, low, close, window=self.atr_period)
        result["atr"] = atr.average_true_range()
        result["atr_pct"] = result["atr"] / close  # ATR as percentage of price

        # 9. 스토캐스틱
        stoch = StochasticOscillator(high, low, close, window=self.stoch_period, smooth_window=self.stoch_smooth)
        result["stoch_k"] = stoch.stoch()
        result["stoch_d"] = stoch.stoch_signal()

        # 10. ADX (Average Directional Index)
        adx = ADXIndicator(high, low, close, window=self.adx_period)
        result["adx"] = adx.adx()
        result["adx_pos"] = adx.adx_pos()
        result["adx_neg"] = adx.adx_neg()

        # 11. ROC (Rate of Change)
        roc = ROCIndicator(close, window=10)
        result["roc"] = roc.roc()

        # 12. OBV (On Balance Volume)
        obv = OnBalanceVolumeIndicator(close, volume)
        result["obv"] = obv.on_balance_volume()

        # 13. 변동성 지표
        result["volatility_20d"] = close.rolling(window=20).std() / close.rolling(window=20).mean()

        # 14. 모멘텀 지표
        result["momentum_10d"] = close - close.shift(10)
        result["momentum_20d"] = close - close.shift(20)

        # 15. 고가/저가 대비 위치
        result["high_low_ratio"] = (close - low) / (high - low + 1e-10)

        # 16. 거래량 지표
        result["volume_sma_20"] = volume.rolling(window=20).mean()
        result["volume_ratio"] = volume / result["volume_sma_20"]

        # 17. 가격 갭
        result["gap"] = df["Open"] - df["Close"].shift(1)
        result["gap_pct"] = result["gap"] / df["Close"].shift(1)

        # 18. 캔들 패턴 특성
        result["body"] = close - df["Open"]
        result["body_pct"] = result["body"] / df["Open"]
        result["upper_shadow"] = high - np.maximum(close, df["Open"])
        result["lower_shadow"] = np.minimum(close, df["Open"]) - low

        logger.info(f"Extracted {len(result.columns) - len(df.columns)} features")

        if dropna:
            result = result.dropna()
            logger.info(f"After dropna: {len(result)} rows")

        return result

    def prepare_ml_data(
        self,
        df: pd.DataFrame,
        target_horizon: int = 5,
        target_type: str = "direction",
        feature_columns: Optional[List[str]] = None,
    ) -> Tuple[np.ndarray, np.ndarray]:
        """
        ML 모델 훈련을 위한 X, y 데이터 준비.

        Args:
            df: 피처가 추출된 데이터프레임
            target_horizon: 예측 기간 (일)
            target_type: 타겟 유형
                - "direction": 방향 (1: 상승, 0: 하락)
                - "returns": 수익률
                - "multi_class": 다중 클래스 (0: 하락, 1: 횡보, 2: 상승)
            feature_columns: 사용할 피처 컬럼 리스트 (None이면 자동 선택)

        Returns:
            (X, y) 튜플
        """
        result = df.copy()

        # 타겟 생성
        future_returns = result["Close"].shift(-target_horizon) / result["Close"] - 1

        if target_type == "direction":
            result["target"] = (future_returns > 0).astype(int)
        elif target_type == "returns":
            result["target"] = future_returns
        elif target_type == "multi_class":
            # 횡보 범위: -1% ~ +1%
            result["target"] = pd.cut(
                future_returns,
                bins=[-np.inf, -0.01, 0.01, np.inf],
                labels=[0, 1, 2]
            ).astype(int)
        else:
            raise ValueError(f"Unknown target_type: {target_type}")

        # NaN 제거 (미래 데이터가 없는 마지막 행들)
        result = result.dropna()

        # 피처 선택
        if feature_columns is None:
            # OHLCV와 target을 제외한 모든 컬럼
            exclude_cols = ["Open", "High", "Low", "Close", "Volume", "target"]
            feature_columns = [c for c in result.columns if c not in exclude_cols]

        X = result[feature_columns].values
        y = result["target"].values

        logger.info(f"Prepared ML data - X: {X.shape}, y: {y.shape}")
        logger.info(f"Target distribution: {np.unique(y, return_counts=True)}")

        return X, y, feature_columns

    def get_feature_names(self) -> List[str]:
        """생성되는 피처 이름 목록 반환."""
        # 더미 데이터로 피처 이름 추출
        dummy_df = pd.DataFrame({
            "Open": [100] * 250,
            "High": [105] * 250,
            "Low": [95] * 250,
            "Close": [102] * 250,
            "Volume": [1000000] * 250,
        })
        result = self.extract_features(dummy_df, dropna=False)
        exclude_cols = ["Open", "High", "Low", "Close", "Volume"]
        return [c for c in result.columns if c not in exclude_cols]


class SequenceFeatureEngineer:
    """
    시퀀스 기반 피처 엔지니어링 (LSTM/Transformer용).

    과거 N일간의 데이터를 시퀀스로 변환합니다.
    """

    def __init__(self, base_engineer: Optional[FeatureEngineer] = None):
        self.base_engineer = base_engineer or FeatureEngineer()

    def create_sequences(
        self,
        df: pd.DataFrame,
        sequence_length: int = 60,
        target_horizon: int = 5,
        target_type: str = "direction",
    ) -> Tuple[np.ndarray, np.ndarray]:
        """
        시퀀스 데이터 생성.

        Args:
            df: OHLCV 데이터프레임
            sequence_length: 시퀀스 길이 (과거 N일)
            target_horizon: 예측 기간
            target_type: 타겟 유형

        Returns:
            (X, y) 튜플 - X shape: (samples, sequence_length, features)
        """
        # 피처 추출
        features_df = self.base_engineer.extract_features(df, dropna=True)
        X_flat, y_flat, feature_names = self.base_engineer.prepare_ml_data(
            features_df, target_horizon=target_horizon, target_type=target_type
        )

        # 시퀀스 생성
        X_sequences = []
        y_sequences = []

        for i in range(sequence_length, len(X_flat)):
            X_sequences.append(X_flat[i - sequence_length:i])
            y_sequences.append(y_flat[i])

        X = np.array(X_sequences)
        y = np.array(y_sequences)

        logger.info(f"Created sequences - X: {X.shape}, y: {y.shape}")

        return X, y


if __name__ == "__main__":
    # 테스트
    from data_fetcher import YahooDataFetcher

    fetcher = YahooDataFetcher()
    df = fetcher.fetch("SPY", period="5y")

    engineer = FeatureEngineer()
    features_df = engineer.extract_features(df)
    print(f"Features shape: {features_df.shape}")
    print(f"Feature columns: {features_df.columns.tolist()}")

    X, y, feature_names = engineer.prepare_ml_data(features_df, target_horizon=5)
    print(f"X shape: {X.shape}, y shape: {y.shape}")
    print(f"Feature names: {feature_names[:10]}...")
