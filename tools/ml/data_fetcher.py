"""
Yahoo Finance 데이터 수집 모듈.

백테스트 및 ML 모델 훈련을 위한 OHLCV 데이터를 Yahoo Finance에서 가져옵니다.

사용 예시:
    from data_fetcher import YahooDataFetcher

    fetcher = YahooDataFetcher()
    df = fetcher.fetch("AAPL", period="5y")
    df = fetcher.fetch_multiple(["AAPL", "MSFT", "GOOGL"], period="2y")
"""

import yfinance as yf
import pandas as pd
import numpy as np
from datetime import datetime, timedelta
from typing import List, Optional, Dict, Union
from pathlib import Path
import logging
import json

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class YahooDataFetcher:
    """Yahoo Finance 데이터 수집기."""

    # 인기 ETF/주식 심볼
    POPULAR_SYMBOLS = {
        "us_index": ["SPY", "QQQ", "DIA", "IWM"],
        "us_leverage": ["TQQQ", "SQQQ", "UPRO", "SPXU", "SOXL", "SOXS"],
        "us_sector": ["XLK", "XLF", "XLE", "XLV", "XLI", "XLP", "XLY", "XLU", "XLB", "XLRE"],
        "us_bond": ["TLT", "IEF", "SHY", "BND", "LQD", "HYG", "TIP"],
        "us_commodity": ["GLD", "SLV", "USO", "UNG"],
        "us_international": ["VEA", "VWO", "EFA", "EEM"],
        "crypto": ["BTC-USD", "ETH-USD"],
        "kr_etf": [
            "069500.KS",  # KODEX 200
            "122630.KS",  # KODEX 레버리지
            "252670.KS",  # KODEX 200선물인버스2X
            "379800.KS",  # KODEX 미국S&P500TR
        ],
    }

    def __init__(self, cache_dir: Optional[str] = None):
        """
        Args:
            cache_dir: 캐시 디렉토리 경로 (None이면 캐싱 비활성화)
        """
        self.cache_dir = Path(cache_dir) if cache_dir else None
        if self.cache_dir:
            self.cache_dir.mkdir(parents=True, exist_ok=True)

    def fetch(
        self,
        symbol: str,
        period: str = "5y",
        interval: str = "1d",
        start: Optional[str] = None,
        end: Optional[str] = None,
        auto_adjust: bool = True,
    ) -> pd.DataFrame:
        """
        단일 심볼의 OHLCV 데이터 수집.

        Args:
            symbol: 티커 심볼 (예: "AAPL", "SPY", "BTC-USD")
            period: 데이터 기간 (1d, 5d, 1mo, 3mo, 6mo, 1y, 2y, 5y, 10y, ytd, max)
            interval: 데이터 간격 (1m, 2m, 5m, 15m, 30m, 60m, 90m, 1h, 1d, 5d, 1wk, 1mo, 3mo)
            start: 시작일 (YYYY-MM-DD) - period 대신 사용
            end: 종료일 (YYYY-MM-DD) - start와 함께 사용
            auto_adjust: 수정주가 사용 여부

        Returns:
            OHLCV 데이터프레임 (columns: Open, High, Low, Close, Volume)
        """
        logger.info(f"Fetching {symbol} data (period={period}, interval={interval})")

        # 캐시 확인
        cache_key = f"{symbol}_{period}_{interval}"
        if self.cache_dir:
            cache_file = self.cache_dir / f"{cache_key}.parquet"
            if cache_file.exists():
                logger.info(f"Loading from cache: {cache_file}")
                return pd.read_parquet(cache_file)

        try:
            ticker = yf.Ticker(symbol)

            if start and end:
                df = ticker.history(start=start, end=end, interval=interval, auto_adjust=auto_adjust)
            else:
                df = ticker.history(period=period, interval=interval, auto_adjust=auto_adjust)

            if df.empty:
                logger.warning(f"No data found for {symbol}")
                return pd.DataFrame()

            # 컬럼 정리
            df = df[["Open", "High", "Low", "Close", "Volume"]]
            df.index.name = "Date"

            # 캐시 저장
            if self.cache_dir:
                df.to_parquet(cache_file)
                logger.info(f"Saved to cache: {cache_file}")

            logger.info(f"Fetched {len(df)} rows for {symbol}")
            return df

        except Exception as e:
            logger.error(f"Error fetching {symbol}: {e}")
            return pd.DataFrame()

    def fetch_multiple(
        self,
        symbols: List[str],
        period: str = "5y",
        interval: str = "1d",
        **kwargs
    ) -> Dict[str, pd.DataFrame]:
        """
        여러 심볼의 데이터 수집.

        Args:
            symbols: 티커 심볼 리스트
            period: 데이터 기간
            interval: 데이터 간격
            **kwargs: fetch()에 전달할 추가 인자

        Returns:
            {symbol: DataFrame} 딕셔너리
        """
        results = {}
        for symbol in symbols:
            df = self.fetch(symbol, period=period, interval=interval, **kwargs)
            if not df.empty:
                results[symbol] = df
        return results

    def fetch_category(
        self,
        category: str,
        period: str = "5y",
        interval: str = "1d",
    ) -> Dict[str, pd.DataFrame]:
        """
        카테고리별 데이터 수집.

        Args:
            category: 카테고리 이름 (us_index, us_leverage, us_sector, etc.)
            period: 데이터 기간
            interval: 데이터 간격

        Returns:
            {symbol: DataFrame} 딕셔너리
        """
        if category not in self.POPULAR_SYMBOLS:
            raise ValueError(f"Unknown category: {category}. Available: {list(self.POPULAR_SYMBOLS.keys())}")

        symbols = self.POPULAR_SYMBOLS[category]
        return self.fetch_multiple(symbols, period=period, interval=interval)

    def get_info(self, symbol: str) -> Dict:
        """티커 정보 조회."""
        ticker = yf.Ticker(symbol)
        return ticker.info

    def save_to_csv(self, df: pd.DataFrame, filepath: str) -> None:
        """데이터프레임을 CSV로 저장."""
        df.to_csv(filepath)
        logger.info(f"Saved to {filepath}")

    def save_to_parquet(self, df: pd.DataFrame, filepath: str) -> None:
        """데이터프레임을 Parquet으로 저장."""
        df.to_parquet(filepath)
        logger.info(f"Saved to {filepath}")


def prepare_training_data(
    symbol: str,
    period: str = "5y",
    test_ratio: float = 0.2,
    val_ratio: float = 0.1,
) -> tuple:
    """
    훈련용 데이터 준비 (train/val/test 분할).

    Args:
        symbol: 티커 심볼
        period: 데이터 기간
        test_ratio: 테스트 데이터 비율
        val_ratio: 검증 데이터 비율

    Returns:
        (train_df, val_df, test_df) 튜플
    """
    fetcher = YahooDataFetcher()
    df = fetcher.fetch(symbol, period=period)

    if df.empty:
        raise ValueError(f"No data for {symbol}")

    n = len(df)
    test_size = int(n * test_ratio)
    val_size = int(n * val_ratio)
    train_size = n - test_size - val_size

    train_df = df.iloc[:train_size]
    val_df = df.iloc[train_size:train_size + val_size]
    test_df = df.iloc[train_size + val_size:]

    logger.info(f"Data split - Train: {len(train_df)}, Val: {len(val_df)}, Test: {len(test_df)}")

    return train_df, val_df, test_df


if __name__ == "__main__":
    # 테스트
    fetcher = YahooDataFetcher(cache_dir="./data_cache")

    # 단일 심볼
    df = fetcher.fetch("SPY", period="2y")
    print(f"SPY data shape: {df.shape}")
    print(df.head())

    # 카테고리
    us_index_data = fetcher.fetch_category("us_index", period="1y")
    for symbol, data in us_index_data.items():
        print(f"{symbol}: {data.shape}")
