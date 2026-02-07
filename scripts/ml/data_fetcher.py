"""
데이터베이스에서 OHLCV 데이터를 로드하는 모듈.

TimescaleDB의 ohlcv 테이블에서 캔들 데이터를 조회합니다.
"""

import os
from dataclasses import dataclass
from datetime import datetime
from typing import Optional

import pandas as pd
import polars as pl
from sqlalchemy import create_engine, text


@dataclass
class OhlcvData:
    """OHLCV 캔들스틱 데이터."""

    symbol: str
    timeframe: str
    data: pl.DataFrame  # columns: timestamp, open, high, low, close, volume

    @property
    def closes(self) -> list[float]:
        return self.data["close"].to_list()

    @property
    def highs(self) -> list[float]:
        return self.data["high"].to_list()

    @property
    def lows(self) -> list[float]:
        return self.data["low"].to_list()

    @property
    def opens(self) -> list[float]:
        return self.data["open"].to_list()

    @property
    def volumes(self) -> list[float]:
        return self.data["volume"].to_list()

    def __len__(self) -> int:
        return len(self.data)


class DataFetcher:
    """TimescaleDB에서 OHLCV 데이터를 로드하는 클래스."""

    def __init__(self, database_url: Optional[str] = None):
        """
        DataFetcher 초기화.

        Args:
            database_url: PostgreSQL 연결 문자열.
                          None이면 DATABASE_URL 환경변수 사용.
        """
        self.database_url = database_url or os.getenv(
            "DATABASE_URL",
            "postgresql://trader:trader_secret@localhost:5432/trader"
        )
        self.engine = create_engine(self.database_url)

    def fetch_ohlcv(
        self,
        symbol: str,
        timeframe: str = "1d",
        start_date: Optional[datetime] = None,
        end_date: Optional[datetime] = None,
        limit: Optional[int] = None,
    ) -> OhlcvData:
        """
        OHLCV 데이터 조회.

        Args:
            symbol: 심볼 (예: "005930", "AAPL", "BTC/USDT")
            timeframe: 타임프레임 (예: "1d", "1h", "4h")
            start_date: 시작 날짜 (포함)
            end_date: 종료 날짜 (포함)
            limit: 최대 행 수

        Returns:
            OhlcvData: 캔들스틱 데이터
        """
        # 쿼리 조건 구성
        conditions = ["symbol = :symbol", "timeframe = :timeframe"]
        params: dict = {"symbol": symbol, "timeframe": timeframe}

        if start_date:
            conditions.append("open_time >= :start_date")
            params["start_date"] = start_date

        if end_date:
            conditions.append("open_time <= :end_date")
            params["end_date"] = end_date

        where_clause = " AND ".join(conditions)
        limit_clause = f"LIMIT {limit}" if limit else ""

        query = f"""
            SELECT
                open_time as timestamp,
                open::float8 as open,
                high::float8 as high,
                low::float8 as low,
                close::float8 as close,
                volume::float8 as volume
            FROM ohlcv
            WHERE {where_clause}
            ORDER BY open_time ASC
            {limit_clause}
        """

        with self.engine.connect() as conn:
            result = conn.execute(text(query), params)
            rows = result.fetchall()

        if not rows:
            # 빈 데이터 반환
            return OhlcvData(
                symbol=symbol,
                timeframe=timeframe,
                data=pl.DataFrame({
                    "timestamp": [],
                    "open": [],
                    "high": [],
                    "low": [],
                    "close": [],
                    "volume": [],
                })
            )

        # Polars DataFrame으로 변환
        df = pl.DataFrame({
            "timestamp": [row[0] for row in rows],
            "open": [row[1] for row in rows],
            "high": [row[2] for row in rows],
            "low": [row[3] for row in rows],
            "close": [row[4] for row in rows],
            "volume": [row[5] for row in rows],
        })

        return OhlcvData(symbol=symbol, timeframe=timeframe, data=df)

    def list_symbols(self, timeframe: str = "1d") -> list[str]:
        """
        데이터가 있는 심볼 목록 조회.

        Args:
            timeframe: 타임프레임 필터

        Returns:
            심볼 리스트
        """
        query = """
            SELECT DISTINCT symbol
            FROM ohlcv
            WHERE timeframe = :timeframe
            ORDER BY symbol
        """

        with self.engine.connect() as conn:
            result = conn.execute(text(query), {"timeframe": timeframe})
            return [row[0] for row in result.fetchall()]

    def get_date_range(
        self, symbol: str, timeframe: str = "1d"
    ) -> tuple[Optional[datetime], Optional[datetime]]:
        """
        심볼의 데이터 날짜 범위 조회.

        Args:
            symbol: 심볼
            timeframe: 타임프레임

        Returns:
            (시작일, 종료일) 튜플
        """
        query = """
            SELECT MIN(open_time), MAX(open_time)
            FROM ohlcv
            WHERE symbol = :symbol AND timeframe = :timeframe
        """

        with self.engine.connect() as conn:
            result = conn.execute(
                text(query), {"symbol": symbol, "timeframe": timeframe}
            )
            row = result.fetchone()
            if row:
                return row[0], row[1]
            return None, None

    def fetch_multiple_symbols(
        self,
        symbols: list[str],
        timeframe: str = "1d",
        start_date: Optional[datetime] = None,
        end_date: Optional[datetime] = None,
    ) -> dict[str, OhlcvData]:
        """
        여러 심볼의 OHLCV 데이터 조회.

        Args:
            symbols: 심볼 리스트
            timeframe: 타임프레임
            start_date: 시작 날짜
            end_date: 종료 날짜

        Returns:
            심볼 -> OhlcvData 딕셔너리
        """
        return {
            symbol: self.fetch_ohlcv(symbol, timeframe, start_date, end_date)
            for symbol in symbols
        }


if __name__ == "__main__":
    # 테스트
    fetcher = DataFetcher()

    print("=== Available Symbols ===")
    symbols = fetcher.list_symbols()
    print(f"Total symbols: {len(symbols)}")
    if symbols:
        print(f"First 10: {symbols[:10]}")

    if symbols:
        symbol = symbols[0]
        print(f"\n=== Data for {symbol} ===")
        data = fetcher.fetch_ohlcv(symbol, limit=10)
        print(f"Rows: {len(data)}")
        print(data.data)

        start, end = fetcher.get_date_range(symbol)
        print(f"Date range: {start} ~ {end}")
