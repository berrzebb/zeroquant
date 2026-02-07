"""
ML 모델을 위한 Feature Engineering 모듈.

중요: 이 모듈은 Rust의 trader-analytics/src/ml/features.rs와
      정확히 동일한 계산 로직을 사용해야 합니다.
      불일치 시 예측 정확도가 저하됩니다.

Feature 목록 (총 22개):
- SMA 비율 (4): sma_5_ratio, sma_10_ratio, sma_20_ratio, sma_50_ratio
- EMA 비율 (2): ema_12_ratio, ema_26_ratio
- RSI (1): rsi (0-1 정규화)
- MACD (2): macd_histogram, macd_signal_ratio
- Bollinger Bands (2): bb_percent_b, bb_bandwidth
- ATR (1): atr_ratio
- 수익률 (3): return_1, return_5, return_10
- 로그 수익률 (3): log_return_1, log_return_5, log_return_10
- 캔들 특성 (4): body_ratio, upper_shadow_ratio, lower_shadow_ratio, volume_change
"""

import math
from dataclasses import dataclass, field
from typing import Optional

import numpy as np
import polars as pl

from .data_fetcher import OhlcvData


@dataclass
class FeatureConfig:
    """Feature 추출 설정. Rust FeatureConfig와 동일해야 함."""

    sma_periods: list[int] = field(default_factory=lambda: [5, 10, 20, 50])
    ema_periods: list[int] = field(default_factory=lambda: [12, 26])
    rsi_period: int = 14
    macd_params: tuple[int, int, int] = (12, 26, 9)  # fast, slow, signal
    bb_period: int = 20
    bb_std_dev: float = 2.0
    atr_period: int = 14
    return_periods: list[int] = field(default_factory=lambda: [1, 5, 10])

    def min_klines_required(self) -> int:
        """Feature 추출에 필요한 최소 캔들 수."""
        return max(
            max(self.sma_periods, default=0),
            max(self.ema_periods, default=0),
            self.rsi_period + 1,
            self.macd_params[1] + self.macd_params[2],
            self.bb_period,
            self.atr_period,
            max(self.return_periods, default=0) + 1,
            50,  # 기본 최소값
        )

    def feature_count(self) -> int:
        """예상 Feature 벡터 크기."""
        return (
            len(self.sma_periods)  # SMA 비율
            + len(self.ema_periods)  # EMA 비율
            + 1  # RSI
            + 2  # MACD (histogram, signal_ratio)
            + 2  # Bollinger (%B, bandwidth)
            + 1  # ATR 비율
            + len(self.return_periods)  # 수익률
            + len(self.return_periods)  # 로그 수익률
            + 4  # 캔들 특성
        )

    def feature_names(self) -> list[str]:
        """Feature 이름 목록."""
        names = []
        for p in self.sma_periods:
            names.append(f"sma_{p}_ratio")
        for p in self.ema_periods:
            names.append(f"ema_{p}_ratio")
        names.append("rsi")
        names.extend(["macd_histogram", "macd_signal_ratio"])
        names.extend(["bb_percent_b", "bb_bandwidth"])
        names.append("atr_ratio")
        for p in self.return_periods:
            names.append(f"return_{p}")
        for p in self.return_periods:
            names.append(f"log_return_{p}")
        names.extend(["body_ratio", "upper_shadow_ratio", "lower_shadow_ratio", "volume_change"])
        return names


@dataclass
class FeatureVector:
    """Feature 벡터."""

    values: np.ndarray  # float32
    names: list[str]

    def __len__(self) -> int:
        return len(self.values)

    def standardize(self) -> "FeatureVector":
        """표준화 (평균 0, 표준편차 1)."""
        mean = np.mean(self.values)
        std = np.std(self.values)
        if std > 0:
            self.values = (self.values - mean) / std
        return self


class FeatureExtractor:
    """
    Kline 데이터에서 ML Feature를 추출하는 클래스.

    Rust features.rs와 동일한 계산 로직 사용.
    """

    def __init__(self, config: Optional[FeatureConfig] = None):
        self.config = config or FeatureConfig()

    def extract(self, data: OhlcvData) -> FeatureVector:
        """
        OhlcvData에서 Feature 추출.

        Args:
            data: OHLCV 데이터

        Returns:
            FeatureVector: Feature 벡터

        Raises:
            ValueError: 데이터가 부족한 경우
        """
        min_required = self.config.min_klines_required()
        if len(data) < min_required:
            raise ValueError(
                f"Insufficient data: required {min_required}, actual {len(data)}"
            )

        closes = np.array(data.closes, dtype=np.float64)
        highs = np.array(data.highs, dtype=np.float64)
        lows = np.array(data.lows, dtype=np.float64)
        opens = np.array(data.opens, dtype=np.float64)
        volumes = np.array(data.volumes, dtype=np.float64)

        current_close = closes[-1]
        current_high = highs[-1]
        current_low = lows[-1]
        current_open = opens[-1]

        features = []

        # 1. SMA 비율 (price / SMA - 1)
        for period in self.config.sma_periods:
            sma = self._calculate_sma(closes, period)
            ratio = (current_close / sma - 1.0) if sma > 0 else 0.0
            features.append(ratio)

        # 2. EMA 비율
        for period in self.config.ema_periods:
            ema = self._calculate_ema(closes, period)
            ratio = (current_close / ema - 1.0) if ema > 0 else 0.0
            features.append(ratio)

        # 3. RSI (0-1 정규화)
        rsi = self._calculate_rsi(closes, self.config.rsi_period)
        features.append(rsi / 100.0)

        # 4. MACD features
        fast, slow, signal = self.config.macd_params
        macd_hist, macd_signal_ratio = self._calculate_macd(closes, fast, slow, signal)
        features.append(macd_hist)
        features.append(macd_signal_ratio)

        # 5. Bollinger Bands features
        bb_percent_b, bb_bandwidth = self._calculate_bollinger(
            closes, self.config.bb_period, self.config.bb_std_dev
        )
        features.append(bb_percent_b)
        features.append(bb_bandwidth)

        # 6. ATR 비율
        atr = self._calculate_atr(highs, lows, closes, self.config.atr_period)
        atr_ratio = atr / current_close if current_close > 0 else 0.0
        features.append(atr_ratio)

        # 7. 수익률
        for period in self.config.return_periods:
            ret = self._calculate_return(closes, period)
            features.append(ret)

        # 8. 로그 수익률
        for period in self.config.return_periods:
            log_ret = self._calculate_log_return(closes, period)
            features.append(log_ret)

        # 9. 캔들 특성
        candle_range = current_high - current_low
        body = abs(current_close - current_open)

        # 몸통 비율
        body_ratio = body / candle_range if candle_range > 0 else 0.0
        features.append(body_ratio)

        # 윗꼬리 비율
        if current_close > current_open:
            upper_shadow = current_high - current_close
        else:
            upper_shadow = current_high - current_open
        upper_shadow_ratio = upper_shadow / candle_range if candle_range > 0 else 0.0
        features.append(upper_shadow_ratio)

        # 아랫꼬리 비율
        if current_close > current_open:
            lower_shadow = current_open - current_low
        else:
            lower_shadow = current_close - current_low
        lower_shadow_ratio = lower_shadow / candle_range if candle_range > 0 else 0.0
        features.append(lower_shadow_ratio)

        # 거래량 변화 비율
        prev_volume = volumes[-2] if len(volumes) >= 2 else volumes[-1]
        current_volume = volumes[-1]
        volume_change = (current_volume / prev_volume - 1.0) if prev_volume > 0 else 0.0
        volume_change = max(-2.0, min(2.0, volume_change))  # clamp
        features.append(volume_change)

        return FeatureVector(
            values=np.array(features, dtype=np.float32),
            names=self.config.feature_names(),
        )

    def extract_batch(self, data: OhlcvData, window_size: Optional[int] = None) -> list[FeatureVector]:
        """
        슬라이딩 윈도우로 여러 시점의 Feature 추출.

        Args:
            data: OHLCV 데이터
            window_size: 윈도우 크기 (None이면 min_klines_required 사용)

        Returns:
            각 시점의 Feature 벡터 리스트
        """
        window = window_size or self.config.min_klines_required()
        features = []

        for i in range(window, len(data) + 1):
            # 슬라이스 데이터 생성
            slice_data = OhlcvData(
                symbol=data.symbol,
                timeframe=data.timeframe,
                data=data.data.slice(i - window, window),
            )
            features.append(self.extract(slice_data))

        return features

    def extract_with_labels(
        self,
        data: OhlcvData,
        future_periods: int = 5,
        threshold: float = 0.02,
    ) -> tuple[np.ndarray, np.ndarray]:
        """
        Feature와 라벨 추출 (학습용).

        Args:
            data: OHLCV 데이터
            future_periods: 미래 수익률 계산 기간
            threshold: 상승/하락 판단 임계값

        Returns:
            (X, y): Feature 행렬과 라벨 배열
                    y: 0 = 하락, 1 = 보합, 2 = 상승
        """
        closes = np.array(data.closes, dtype=np.float64)
        window = self.config.min_klines_required()

        X_list = []
        y_list = []

        for i in range(window, len(data) - future_periods):
            # Feature 추출
            slice_data = OhlcvData(
                symbol=data.symbol,
                timeframe=data.timeframe,
                data=data.data.slice(i - window, window),
            )
            features = self.extract(slice_data)
            X_list.append(features.values)

            # 라벨 계산 (미래 수익률)
            future_return = (closes[i + future_periods] / closes[i]) - 1.0
            if future_return > threshold:
                label = 2  # 상승
            elif future_return < -threshold:
                label = 0  # 하락
            else:
                label = 1  # 보합
            y_list.append(label)

        return np.array(X_list), np.array(y_list)

    # === Private calculation methods (Rust와 동일) ===

    def _calculate_sma(self, data: np.ndarray, period: int) -> float:
        """단순 이동 평균."""
        if len(data) < period or period == 0:
            return 0.0
        return float(np.mean(data[-period:]))

    def _calculate_ema(self, data: np.ndarray, period: int) -> float:
        """지수 이동 평균."""
        if len(data) == 0 or period == 0:
            return 0.0

        multiplier = 2.0 / (period + 1.0)
        ema = data[0]

        for value in data[1:]:
            ema = (value - ema) * multiplier + ema

        return float(ema)

    def _calculate_rsi(self, closes: np.ndarray, period: int) -> float:
        """RSI 계산."""
        if len(closes) < period + 1:
            return 50.0  # Neutral

        gains = []
        losses = []

        for i in range(1, len(closes)):
            change = closes[i] - closes[i - 1]
            if change > 0:
                gains.append(change)
                losses.append(0.0)
            else:
                gains.append(0.0)
                losses.append(abs(change))

        # 마지막 period 개의 값 사용
        start = max(0, len(gains) - period)

        avg_gain = np.mean(gains[start:])
        avg_loss = np.mean(losses[start:])

        if avg_loss == 0:
            return 100.0

        rs = avg_gain / avg_loss
        return 100.0 - (100.0 / (1.0 + rs))

    def _calculate_macd(
        self,
        closes: np.ndarray,
        fast_period: int,
        slow_period: int,
        signal_period: int,
    ) -> tuple[float, float]:
        """MACD 계산."""
        fast_ema = self._calculate_ema(closes, fast_period)
        slow_ema = self._calculate_ema(closes, slow_period)
        macd_line = fast_ema - slow_ema

        # Signal line을 위해 MACD 히스토리 계산
        macd_history = []
        for i in range(slow_period, len(closes) + 1):
            fast = self._calculate_ema(closes[:i], fast_period)
            slow = self._calculate_ema(closes[:i], slow_period)
            macd_history.append(fast - slow)

        signal_line = (
            self._calculate_ema(np.array(macd_history), signal_period)
            if len(macd_history) >= signal_period
            else macd_line
        )

        histogram = macd_line - signal_line

        # 가격 대비 histogram 정규화
        current_price = closes[-1] if len(closes) > 0 else 1.0
        norm_histogram = (histogram / current_price * 100.0) if current_price > 0 else 0.0

        # Signal 비율
        signal_ratio = (
            (macd_line / signal_line - 1.0) if abs(signal_line) > 0.0001 else 0.0
        )
        signal_ratio = max(-1.0, min(1.0, signal_ratio))

        return norm_histogram, signal_ratio

    def _calculate_bollinger(
        self, closes: np.ndarray, period: int, std_dev_mult: float
    ) -> tuple[float, float]:
        """Bollinger Bands 계산."""
        if len(closes) < period:
            return 0.5, 0.0  # Neutral %B, zero bandwidth

        window = closes[-period:]
        sma = np.mean(window)
        std_dev = np.std(window)

        upper_band = sma + std_dev_mult * std_dev
        lower_band = sma - std_dev_mult * std_dev
        bandwidth = upper_band - lower_band

        current_price = closes[-1]

        # %B: 밴드 대비 가격 위치 (0 = 하단, 1 = 상단)
        percent_b = (
            (current_price - lower_band) / bandwidth if bandwidth > 0 else 0.5
        )
        percent_b = max(0.0, min(1.0, percent_b))

        # 중간 밴드 대비 bandwidth 퍼센트
        bandwidth_pct = bandwidth / sma if sma > 0 else 0.0

        return percent_b, bandwidth_pct

    def _calculate_atr(
        self,
        highs: np.ndarray,
        lows: np.ndarray,
        closes: np.ndarray,
        period: int,
    ) -> float:
        """ATR 계산."""
        if len(highs) < period + 1 or len(highs) != len(lows) or len(highs) != len(closes):
            return 0.0

        true_ranges = []

        for i in range(1, len(highs)):
            high_low = highs[i] - lows[i]
            high_close = abs(highs[i] - closes[i - 1])
            low_close = abs(lows[i] - closes[i - 1])

            tr = max(high_low, high_close, low_close)
            true_ranges.append(tr)

        # 마지막 period 개의 true range 평균
        if len(true_ranges) < period:
            return np.mean(true_ranges) if true_ranges else 0.0

        return float(np.mean(true_ranges[-period:]))

    def _calculate_return(self, closes: np.ndarray, period: int) -> float:
        """단순 수익률."""
        if len(closes) <= period:
            return 0.0

        current = closes[-1]
        past = closes[-1 - period]

        return (current / past - 1.0) if past > 0 else 0.0

    def _calculate_log_return(self, closes: np.ndarray, period: int) -> float:
        """로그 수익률."""
        if len(closes) <= period:
            return 0.0

        current = closes[-1]
        past = closes[-1 - period]

        return math.log(current / past) if past > 0 and current > 0 else 0.0


if __name__ == "__main__":
    # 테스트
    from .data_fetcher import DataFetcher

    fetcher = DataFetcher()
    symbols = fetcher.list_symbols()

    if symbols:
        symbol = symbols[0]
        print(f"Testing with symbol: {symbol}")

        data = fetcher.fetch_ohlcv(symbol, limit=200)
        print(f"Loaded {len(data)} candles")

        extractor = FeatureExtractor()
        print(f"Required candles: {extractor.config.min_klines_required()}")
        print(f"Feature count: {extractor.config.feature_count()}")

        features = extractor.extract(data)
        print(f"\nExtracted {len(features)} features:")
        for name, value in zip(features.names, features.values):
            print(f"  {name}: {value:.6f}")

        # 라벨 추출 테스트
        X, y = extractor.extract_with_labels(data)
        print(f"\nTraining data: X={X.shape}, y={y.shape}")
        print(f"Label distribution: 하락={sum(y==0)}, 보합={sum(y==1)}, 상승={sum(y==2)}")
