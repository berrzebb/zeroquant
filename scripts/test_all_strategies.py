#!/usr/bin/env python3
"""
전체 전략 백테스트 API 테스트 스크립트
27개 내장 전략에 대해 백테스트 API를 호출하여 정상 동작 확인
"""

import requests
import json
from datetime import datetime, timedelta

API_BASE = "http://localhost:3000/api/v1"

# 전략별 테스트 설정 (전략ID -> 테스트 심볼)
STRATEGY_TEST_CONFIGS = {
    # 단일 자산 전략 - 크립토
    "rsi_mean_reversion": {"symbol": "BTC/USDT", "market": "crypto"},
    "grid_trading": {"symbol": "ETH/USDT", "market": "crypto"},
    "bollinger": {"symbol": "BTC/USDT", "market": "crypto"},
    "volatility_breakout": {"symbol": "BTC/USDT", "market": "crypto"},
    "magic_split": {"symbol": "BTC/USDT", "market": "crypto"},
    "sma_crossover": {"symbol": "BTC/USDT", "market": "crypto"},
    "trailing_stop": {"symbol": "BTC/USDT", "market": "crypto"},
    "candle_pattern": {"symbol": "BTC/USDT", "market": "crypto"},
    "infinity_bot": {"symbol": "BTC/USDT", "market": "crypto"},

    # 자산배분 전략 - US ETF
    "simple_power": {"symbols": ["TQQQ", "SCHD", "PFIX", "TMF"], "market": "us"},
    "haa": {"symbols": ["TIP", "SPY", "IWM", "VEA", "VWO", "TLT", "IEF", "PDBC", "VNQ", "BIL"], "market": "us"},
    "xaa": {"symbols": ["VWO", "BND", "SPY", "EFA", "EEM", "TLT", "IEF", "LQD", "BIL"], "market": "us"},
    "all_weather": {"symbols": ["SPY", "TLT", "IEF", "GLD", "PDBC", "IYK"], "market": "us"},
    "snow": {"symbols": ["TIP", "UPRO", "TLT", "BIL"], "market": "us"},
    "baa": {"symbols": ["SPY", "VEA", "VWO", "BND", "TLT", "LQD", "BIL"], "market": "us"},
    "sector_momentum": {"symbols": ["XLK", "XLF", "XLE", "XLV", "XLY"], "market": "us"},
    "dual_momentum": {"symbols": ["SPY", "EFA", "AGG", "BIL"], "market": "us"},

    # 한국 주식 전략
    "stock_rotation": {"symbols": ["005930", "000660", "035420", "051910", "006400"], "market": "kr"},
    "market_cap_top": {"symbols": ["005930", "000660", "035420"], "market": "kr"},
    "market_interest_day": {"symbol": "005930", "market": "kr"},
    "small_cap_quant": {"symbols": ["005930", "000660", "035420"], "market": "kr"},
    "pension_bot": {"symbols": ["069500", "114800", "148020", "152380"], "market": "kr"},

    # 2차 전략들 (새로 등록)
    "sector_vb": {"symbols": ["XLK", "XLF", "XLE", "XLV"], "market": "us"},
    "kospi_bothside": {"symbols": ["005930", "000660"], "market": "kr"},
    "kosdaq_fire_rain": {"symbols": ["035420", "035720"], "market": "kr"},
    "us_3x_leverage": {"symbols": ["TQQQ", "UPRO", "TMF"], "market": "us"},
    "stock_gugan": {"symbol": "005930", "market": "kr"},
}

def run_backtest(strategy_id: str, config: dict) -> dict:
    """전략에 대해 백테스트 API 호출"""
    end_date = datetime.now()
    start_date = end_date - timedelta(days=30)  # 30일 테스트

    # 심볼 결정
    if "symbols" in config:
        symbols = config["symbols"]
    else:
        symbols = [config.get("symbol", "BTC/USDT")]

    payload = {
        "strategy": strategy_id,
        "symbols": symbols,
        "start_date": start_date.strftime("%Y-%m-%d"),
        "end_date": end_date.strftime("%Y-%m-%d"),
        "initial_capital": 1000000.0,
        "timeframe": "1d"
    }

    try:
        response = requests.post(
            f"{API_BASE}/backtest/run",
            json=payload,
            timeout=60
        )
        return {
            "status_code": response.status_code,
            "success": response.status_code == 200,
            "response": response.json() if response.status_code == 200 else response.text[:500]
        }
    except requests.exceptions.Timeout:
        return {"status_code": 0, "success": False, "response": "Timeout"}
    except Exception as e:
        return {"status_code": 0, "success": False, "response": str(e)}

def main():
    print("=" * 70)
    print("Full Strategy Backtest API Test (27 strategies)")
    print("=" * 70)

    results = {}
    success_count = 0
    fail_count = 0

    for strategy_id, config in STRATEGY_TEST_CONFIGS.items():
        print(f"\nTesting: {strategy_id}...", end=" ", flush=True)
        result = run_backtest(strategy_id, config)
        results[strategy_id] = result

        if result["success"]:
            success_count += 1
            print("[OK]")
        else:
            fail_count += 1
            print(f"[FAIL] (code: {result['status_code']})")
            print(f"   응답: {result['response'][:200]}...")

    print("\n" + "=" * 70)
    print(f"Result: {success_count}/{len(STRATEGY_TEST_CONFIGS)} passed, {fail_count} failed")
    print("=" * 70)

    if fail_count > 0:
        print("\nFailed strategies:")
        for strategy_id, result in results.items():
            if not result["success"]:
                print(f"  - {strategy_id}: {result['response'][:100]}")

    return fail_count == 0

if __name__ == "__main__":
    import sys
    success = main()
    sys.exit(0 if success else 1)
