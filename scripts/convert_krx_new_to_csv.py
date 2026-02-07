#!/usr/bin/env python3
"""
KRX 정보시스템 CSV 변환기.

data/new 폴더의 KRX 원본 CSV 파일들을 표준 형식으로 변환합니다.

입력:
    - data/new/data_3729_*.csv (ETF)
    - data/new/data_3749_*.csv
    - data/new/data_3801_*.csv (주식)
    - data/new/data_3817_*.csv (개별 주식)
    - data/new/data_3831_*.csv (선물/파생)

출력:
    - data/krx_codes.csv (종목코드,종목명 형식)
    - data/krx_codes_detailed.csv (상세 정보 포함)

사용법:
    python scripts/convert_krx_new_to_csv.py
    python scripts/convert_krx_new_to_csv.py --input-dir data/new --output-dir data
"""

import argparse
import csv
import logging
import sys
from pathlib import Path
from typing import Dict, List, Set

# 로깅 설정
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def read_krx_csv(file_path: Path, encoding: str = 'cp949') -> List[Dict[str, str]]:
    """
    KRX CSV 파일 읽기.
    
    Args:
        file_path: CSV 파일 경로
        encoding: 파일 인코딩 (기본: cp949)
    
    Returns:
        레코드 리스트
    """
    records = []
    
    try:
        with open(file_path, 'r', encoding=encoding) as f:
            reader = csv.DictReader(f)
            for row in reader:
                records.append(row)
        
        logger.info(f"✓ {file_path.name}: {len(records)}개 레코드 읽음")
        return records
    
    except UnicodeDecodeError:
        # CP949로 실패하면 EUC-KR 시도
        try:
            with open(file_path, 'r', encoding='euc-kr') as f:
                reader = csv.DictReader(f)
                for row in reader:
                    records.append(row)
            
            logger.info(f"✓ {file_path.name}: {len(records)}개 레코드 읽음 (EUC-KR)")
            return records
        
        except Exception as e:
            logger.error(f"✗ {file_path.name}: 인코딩 오류 - {e}")
            return []
    
    except Exception as e:
        logger.error(f"✗ {file_path.name}: 읽기 실패 - {e}")
        return []


def extract_symbol_info(record: Dict[str, str], file_type: str) -> tuple[str, str]:
    """
    레코드에서 종목코드와 종목명 추출.
    
    Args:
        record: CSV 레코드
        file_type: 파일 유형 (3729, 3801, 3817 등)
    
    Returns:
        (종목코드, 종목명) 튜플
    """
    # 단축코드 추출 (6자리 숫자)
    ticker = record.get('단축코드', '').strip().strip('"')
    
    # 종목명 추출 (축약명 우선, 없으면 전체명)
    name = record.get('한글종목약명', '').strip().strip('"')
    if not name:
        name = record.get('한글종목명', '').strip().strip('"')
    
    return ticker, name


def is_valid_ticker(ticker: str) -> bool:
    """
    유효한 티커인지 확인.
    
    Args:
        ticker: 종목코드
    
    Returns:
        유효 여부
    """
    # 최소 6자리 숫자 또는 영숫자
    if len(ticker) < 6:
        return False
    
    # 영숫자만 허용
    if not ticker.isalnum():
        return False
    
    return True


def convert_krx_new_to_csv(
    input_dir: Path,
    output_dir: Path,
    include_detailed: bool = True
) -> tuple[int, int]:
    """
    data/new 폴더의 CSV 파일들을 표준 형식으로 변환.
    
    Args:
        input_dir: 입력 디렉토리 (data/new)
        output_dir: 출력 디렉토리 (data)
        include_detailed: 상세 정보 CSV 생성 여부
    
    Returns:
        (총 레코드 수, 유효 레코드 수) 튜플
    """
    logger.info(f"입력 디렉토리: {input_dir}")
    logger.info(f"출력 디렉토리: {output_dir}")
    
    # 입력 디렉토리 확인
    if not input_dir.exists():
        logger.error(f"입력 디렉토리가 없습니다: {input_dir}")
        return 0, 0
    
    # 출력 디렉토리 생성
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # CSV 파일 찾기 (data_*.csv 패턴)
    csv_files = sorted(input_dir.glob("data_*.csv"))
    
    if not csv_files:
        logger.warning(f"CSV 파일을 찾지 못했습니다: {input_dir}")
        return 0, 0
    
    logger.info(f"발견된 CSV 파일: {len(csv_files)}개")
    
    # 전체 심볼 수집 (중복 제거)
    symbols: Dict[str, str] = {}  # ticker → name
    detailed_records: List[Dict[str, str]] = []
    total_records = 0
    
    for csv_file in csv_files:
        # 파일 유형 추출 (data_3729_*.csv → 3729)
        file_type = csv_file.stem.split('_')[1] if '_' in csv_file.stem else 'unknown'
        
        logger.info(f"\n처리 중: {csv_file.name} (유형: {file_type})")
        
        # CSV 파일 읽기
        records = read_krx_csv(csv_file)
        total_records += len(records)
        
        for record in records:
            # 종목코드와 종목명 추출
            ticker, name = extract_symbol_info(record, file_type)
            
            # 유효성 검사
            if not is_valid_ticker(ticker):
                continue
            
            if not name:
                continue
            
            # 중복 제거 (같은 티커면 마지막 값 사용)
            symbols[ticker] = name
            
            # 상세 정보 저장 (선택적)
            if include_detailed:
                detailed_records.append({
                    'ticker': ticker,
                    'name': name,
                    'file_type': file_type,
                    'exchange': record.get('기초시장분류', '').strip(),
                    'asset_type': record.get('기초자산분류', '').strip(),
                    'listing_date': record.get('상장일', '').strip(),
                })
    
    logger.info(f"\n총 레코드: {total_records}개")
    logger.info(f"유효 종목: {len(symbols)}개 (중복 제거 후)")
    
    # 1. 표준 CSV 저장 (종목코드,종목명)
    output_file = output_dir / "krx_codes.csv"
    
    with open(output_file, 'w', encoding='utf-8', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['종목코드', '종목명'])
        
        for ticker in sorted(symbols.keys()):
            writer.writerow([ticker, symbols[ticker]])
    
    logger.info(f"✓ 표준 CSV 저장: {output_file} ({len(symbols)}개 종목)")
    
    # 2. 상세 CSV 저장 (선택적)
    if include_detailed and detailed_records:
        detailed_output = output_dir / "krx_codes_detailed.csv"
        
        with open(detailed_output, 'w', encoding='utf-8', newline='') as f:
            fieldnames = ['ticker', 'name', 'file_type', 'exchange', 'asset_type', 'listing_date']
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(detailed_records)
        
        logger.info(f"✓ 상세 CSV 저장: {detailed_output} ({len(detailed_records)}개 레코드)")
    
    return total_records, len(symbols)


def main():
    """메인 함수."""
    parser = argparse.ArgumentParser(
        description='KRX 정보시스템 CSV 변환기',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
예시:
  # 기본 사용
  python scripts/convert_krx_new_to_csv.py
  
  # 디렉토리 지정
  python scripts/convert_krx_new_to_csv.py --input-dir data/new --output-dir data
  
  # 상세 정보 CSV 제외
  python scripts/convert_krx_new_to_csv.py --no-detailed
        """
    )
    
    parser.add_argument(
        '--input-dir',
        type=Path,
        default=Path('data/new'),
        help='입력 디렉토리 (기본: data/new)'
    )
    
    parser.add_argument(
        '--output-dir',
        type=Path,
        default=Path('data'),
        help='출력 디렉토리 (기본: data)'
    )
    
    parser.add_argument(
        '--no-detailed',
        action='store_true',
        help='상세 정보 CSV 생성 안 함'
    )
    
    args = parser.parse_args()
    
    # 변환 실행
    total, valid = convert_krx_new_to_csv(
        input_dir=args.input_dir,
        output_dir=args.output_dir,
        include_detailed=not args.no_detailed
    )
    
    if valid == 0:
        logger.error("변환된 종목이 없습니다.")
        sys.exit(1)
    
    logger.info(f"\n{'='*60}")
    logger.info("변환 완료!")
    logger.info(f"  총 레코드: {total:,}개")
    logger.info(f"  유효 종목: {valid:,}개")
    logger.info(f"  출력 파일: {args.output_dir / 'krx_codes.csv'}")
    logger.info(f"{'='*60}\n")
    
    return 0


if __name__ == '__main__':
    sys.exit(main())
