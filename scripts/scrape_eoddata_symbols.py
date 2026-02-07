#!/usr/bin/env python3
"""
EODData 거래소 심볼 스크래퍼.

https://www.eoddata.com/symbols.aspx에서 해외 거래소 심볼 정보를 수집합니다.

사용법:
    # 모든 주요 거래소 스크래핑
    python scrape_eoddata_symbols.py

    # 특정 거래소만 스크래핑
    python scrape_eoddata_symbols.py --exchanges NYSE NASDAQ

    # 결과 디렉토리 지정
    python scrape_eoddata_symbols.py --output-dir ./data/symbols

    # 지연 시간 설정 (초)
    python scrape_eoddata_symbols.py --delay 2.0

출력:
    - data/eod_{exchange}.csv: 각 거래소별 심볼 파일
    - data/eod_all_symbols.csv: 모든 심볼 통합 파일
"""

import argparse
import csv
import logging
import os
import re
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional
from urllib.parse import urlencode

import requests
from bs4 import BeautifulSoup

# 로깅 설정
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# 기본 설정
BASE_URL = "https://www.eoddata.com/symbols.aspx"
USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

# 주요 거래소 목록 (코드: 설명)
MAJOR_EXCHANGES = {
    "NYSE": "New York Stock Exchange",
    "NASDAQ": "NASDAQ Stock Exchange",
    "AMEX": "American Stock Exchange",
    "LSE": "London Stock Exchange",
    "TSX": "Toronto Stock Exchange",
    "ASX": "Australian Stock Exchange",
    "HKEX": "Hong Kong Stock Exchange",
    "SGX": "Singapore Exchange",
    "FOREX": "Foreign Exchange",
    "INDEX": "World Indices",
}

# Yahoo Finance 심볼 매핑 (거래소 코드 → Yahoo 접미사)
YAHOO_SUFFIX_MAP = {
    "NYSE": "",        # 미국 주식은 접미사 없음
    "NASDAQ": "",      # 미국 주식은 접미사 없음
    "AMEX": "",        # 미국 주식은 접미사 없음
    "LSE": ".L",       # 런던
    "TSX": ".TO",      # 토론토
    "ASX": ".AX",      # 호주
    "HKEX": ".HK",     # 홍콩
    "SGX": ".SI",      # 싱가포르
    "XETRA": ".DE",    # 독일
    "EURONEXT": ".PA", # 파리 (기본값)
    "SIX": ".SW",      # 스위스
    "JSE": ".JO",      # 남아공
    "NSE": ".NS",      # 인도 NSE
    "BSE": ".BO",      # 인도 BSE
    "KSE": ".KS",      # 한국 KOSPI
    "KOSDAQ": ".KQ",   # 한국 KOSDAQ
}


@dataclass
class Symbol:
    """심볼 정보."""
    ticker: str
    name: str
    exchange: str
    yahoo_symbol: Optional[str] = None

    def to_dict(self) -> dict:
        return {
            "ticker": self.ticker,
            "name": self.name,
            "exchange": self.exchange,
            "yahoo_symbol": self.yahoo_symbol or "",
        }


class EODDataScraper:
    """EODData 심볼 스크래퍼."""

    def __init__(self, delay: float = 1.0, max_retries: int = 3):
        """
        초기화.

        Args:
            delay: 요청 간 지연 시간 (초)
            max_retries: 최대 재시도 횟수
        """
        self.delay = delay
        self.max_retries = max_retries
        self.session = requests.Session()
        self.session.headers.update({
            "User-Agent": USER_AGENT,
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "en-US,en;q=0.5",
        })

    def _fetch_page(self, url: str, params: Optional[dict] = None) -> Optional[BeautifulSoup]:
        """
        페이지 가져오기 (재시도 포함).

        Args:
            url: 요청 URL
            params: 쿼리 파라미터

        Returns:
            BeautifulSoup 객체 또는 None
        """
        for attempt in range(self.max_retries):
            try:
                time.sleep(self.delay)
                response = self.session.get(url, params=params, timeout=30)
                response.raise_for_status()
                return BeautifulSoup(response.text, 'html.parser')
            except requests.RequestException as e:
                logger.warning(f"요청 실패 (시도 {attempt + 1}/{self.max_retries}): {e}")
                if attempt < self.max_retries - 1:
                    time.sleep(self.delay * 2)
        return None

    def get_available_exchanges(self) -> list[tuple[str, str]]:
        """
        사용 가능한 거래소 목록 조회.

        Returns:
            [(거래소 코드, 거래소 이름), ...] 리스트
        """
        soup = self._fetch_page(BASE_URL)
        if not soup:
            logger.error("거래소 목록 페이지 로드 실패")
            return []

        exchanges = []

        # 드롭다운에서 거래소 옵션 찾기
        select = soup.find('select', {'id': re.compile(r'.*Exchange.*', re.I)})
        if not select:
            # 대안: 모든 select 태그 검색
            for sel in soup.find_all('select'):
                if any(opt.text.strip() for opt in sel.find_all('option') if 'exchange' in opt.text.lower() or 'stock' in opt.text.lower()):
                    select = sel
                    break

        if select:
            for option in select.find_all('option'):
                code = option.get('value', '').strip()
                name = option.text.strip()
                if code and name:
                    exchanges.append((code, name))

        # 기본 거래소 목록 반환 (페이지 파싱 실패 시)
        if not exchanges:
            logger.warning("거래소 드롭다운을 찾지 못함. 기본 목록 사용")
            exchanges = list(MAJOR_EXCHANGES.items())

        logger.info(f"발견된 거래소: {len(exchanges)}개")
        return exchanges

    def scrape_exchange(self, exchange_code: str) -> list[Symbol]:
        """
        특정 거래소의 모든 심볼 스크래핑.

        Args:
            exchange_code: 거래소 코드 (예: NYSE, NASDAQ)

        Returns:
            Symbol 리스트
        """
        symbols = []
        letters = list("ABCDEFGHIJKLMNOPQRSTUVWXYZ") + ["0"]  # 숫자로 시작하는 심볼

        logger.info(f"[{exchange_code}] 스크래핑 시작...")

        for letter in letters:
            page_symbols = self._scrape_letter_page(exchange_code, letter)
            if page_symbols:
                symbols.extend(page_symbols)
                logger.debug(f"[{exchange_code}] {letter}: {len(page_symbols)}개 심볼")

        logger.info(f"[{exchange_code}] 스크래핑 완료: {len(symbols)}개 심볼")
        return symbols

    def _scrape_letter_page(self, exchange_code: str, letter: str) -> list[Symbol]:
        """
        특정 알파벳 페이지 스크래핑.

        Args:
            exchange_code: 거래소 코드
            letter: 알파벳 문자

        Returns:
            Symbol 리스트
        """
        # URL 구성: https://www.eoddata.com/symbols.aspx?e=NYSE&l=A
        params = {"e": exchange_code, "l": letter}
        soup = self._fetch_page(BASE_URL, params)

        if not soup:
            return []

        symbols = []
        yahoo_suffix = YAHOO_SUFFIX_MAP.get(exchange_code.upper(), "")

        # 테이블 찾기 - 여러 패턴 시도
        table = None

        # 패턴 1: ID로 찾기
        for table_id in ['ctl00_cph1_divSymbols', 'cph1_divSymbols', 'symbolsTable']:
            table = soup.find('table', {'id': re.compile(table_id, re.I)})
            if table:
                break

        # 패턴 2: 클래스로 찾기
        if not table:
            table = soup.find('table', class_=re.compile(r'quotes|symbols|data', re.I))

        # 패턴 3: 구조로 찾기 (Code, Name 헤더가 있는 테이블)
        if not table:
            for t in soup.find_all('table'):
                headers = [th.text.strip().lower() for th in t.find_all('th')]
                if 'code' in headers or 'symbol' in headers:
                    table = t
                    break

        if not table:
            return []

        # 행 파싱
        rows = table.find_all('tr')
        for row in rows[1:]:  # 헤더 건너뛰기
            cells = row.find_all(['td', 'th'])
            if len(cells) >= 2:
                ticker = self._clean_text(cells[0].text)
                name = self._clean_text(cells[1].text)

                if ticker and name and not ticker.lower() in ['code', 'symbol']:
                    yahoo_symbol = f"{ticker}{yahoo_suffix}" if yahoo_suffix else ticker
                    symbols.append(Symbol(
                        ticker=ticker,
                        name=name,
                        exchange=exchange_code,
                        yahoo_symbol=yahoo_symbol
                    ))

        return symbols

    def _clean_text(self, text: str) -> str:
        """텍스트 정제."""
        if not text:
            return ""
        # 공백 정규화, 특수문자 제거
        text = re.sub(r'\s+', ' ', text.strip())
        # CSV 호환성을 위해 쌍따옴표 이스케이프
        text = text.replace('"', '""')
        return text


def save_to_csv(symbols: list[Symbol], filepath: Path) -> int:
    """
    심볼 리스트를 CSV로 저장.

    Args:
        symbols: Symbol 리스트
        filepath: 저장 경로

    Returns:
        저장된 심볼 수
    """
    filepath.parent.mkdir(parents=True, exist_ok=True)

    with open(filepath, 'w', newline='', encoding='utf-8') as f:
        writer = csv.DictWriter(f, fieldnames=['ticker', 'name', 'exchange', 'yahoo_symbol'])
        writer.writeheader()
        for symbol in symbols:
            writer.writerow(symbol.to_dict())

    logger.info(f"저장 완료: {filepath} ({len(symbols)}개 심볼)")
    return len(symbols)


def merge_csv_files(output_dir: Path, output_file: Path) -> int:
    """
    개별 거래소 CSV 파일들을 하나로 병합.

    Args:
        output_dir: CSV 파일들이 있는 디렉토리
        output_file: 출력 파일 경로

    Returns:
        총 심볼 수
    """
    all_symbols = []

    for csv_file in output_dir.glob("eod_*.csv"):
        if csv_file.name == output_file.name:
            continue

        with open(csv_file, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                all_symbols.append(row)

    if all_symbols:
        with open(output_file, 'w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=['ticker', 'name', 'exchange', 'yahoo_symbol'])
            writer.writeheader()
            writer.writerows(all_symbols)

    logger.info(f"병합 완료: {output_file} ({len(all_symbols)}개 심볼)")
    return len(all_symbols)


def main():
    parser = argparse.ArgumentParser(
        description="EODData에서 거래소 심볼 정보를 스크래핑합니다.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
예제:
    # 주요 거래소만 스크래핑
    python scrape_eoddata_symbols.py

    # 특정 거래소만
    python scrape_eoddata_symbols.py --exchanges NYSE NASDAQ

    # 모든 거래소 조회
    python scrape_eoddata_symbols.py --list-exchanges
        """
    )

    parser.add_argument(
        '--exchanges', '-e',
        nargs='+',
        default=None,
        help='스크래핑할 거래소 코드 목록 (기본: 주요 거래소)'
    )
    parser.add_argument(
        '--output-dir', '-o',
        type=Path,
        default=Path('data'),
        help='출력 디렉토리 (기본: data/)'
    )
    parser.add_argument(
        '--delay', '-d',
        type=float,
        default=1.5,
        help='요청 간 지연 시간(초) (기본: 1.5)'
    )
    parser.add_argument(
        '--list-exchanges',
        action='store_true',
        help='사용 가능한 거래소 목록만 출력'
    )
    parser.add_argument(
        '--no-merge',
        action='store_true',
        help='통합 파일 생성 안 함'
    )
    parser.add_argument(
        '--verbose', '-v',
        action='store_true',
        help='상세 로그 출력'
    )

    args = parser.parse_args()

    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)

    scraper = EODDataScraper(delay=args.delay)

    # 거래소 목록 조회 모드
    if args.list_exchanges:
        print("\n사용 가능한 거래소 목록:")
        print("-" * 40)
        exchanges = scraper.get_available_exchanges()
        for code, name in exchanges:
            print(f"  {code:10s} - {name}")
        print(f"\n총 {len(exchanges)}개 거래소")
        return

    # 스크래핑할 거래소 결정
    if args.exchanges:
        exchange_codes = args.exchanges
    else:
        exchange_codes = list(MAJOR_EXCHANGES.keys())

    logger.info(f"스크래핑 대상 거래소: {', '.join(exchange_codes)}")

    # 스크래핑 실행
    total_symbols = 0
    start_time = datetime.now()

    for exchange_code in exchange_codes:
        try:
            symbols = scraper.scrape_exchange(exchange_code)
            if symbols:
                csv_path = args.output_dir / f"eod_{exchange_code.lower()}.csv"
                save_to_csv(symbols, csv_path)
                total_symbols += len(symbols)
        except Exception as e:
            logger.error(f"[{exchange_code}] 스크래핑 실패: {e}")

    # 통합 파일 생성
    if not args.no_merge and total_symbols > 0:
        merge_path = args.output_dir / "eod_all_symbols.csv"
        merge_csv_files(args.output_dir, merge_path)

    elapsed = datetime.now() - start_time
    logger.info(f"\n{'='*50}")
    logger.info(f"스크래핑 완료!")
    logger.info(f"  - 거래소: {len(exchange_codes)}개")
    logger.info(f"  - 총 심볼: {total_symbols}개")
    logger.info(f"  - 소요 시간: {elapsed}")
    logger.info(f"  - 출력 디렉토리: {args.output_dir.absolute()}")


if __name__ == "__main__":
    main()
