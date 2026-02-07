# 백테스트 테스트 시나리오

이 문서는 각 전략별 테스트 파라미터와 기대 결과를 정의합니다.

## 전략 분류

### 단일 자산 전략 (Single Asset Strategies)
- RSI 평균회귀
- 그리드 트레이딩
- 볼린저 밴드
- 변동성 돌파
- Magic Split
- 이동평균 크로스오버

### 다중 자산 전략 (Multi-Asset Strategies)
- CompoundMomentum (TQQQ/SCHD/PFIX/TMF)
- HAA (SPY/TLT/VEA/VWO 등)
- XAA (SPY/QQQ/TLT 등)
- 종목 갈아타기 (여러 종목 비교)

---

## 단일 자산 전략 테스트 시나리오

### 1. RSI 평균회귀 (rsi_mean_reversion)

```json
{
  "strategy_id": "rsi_mean_reversion",
  "symbol": "005930",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "period": 14,
    "oversold_threshold": 30,
    "overbought_threshold": 70,
    "amount": "100000"
  }
}
```

**기대 결과**: RSI가 30 이하로 떨어지면 매수, 70 이상으로 올라가면 매도

---

### 2. 그리드 트레이딩 (grid_trading)

```json
{
  "strategy_id": "grid_trading",
  "symbol": "005930",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "grid_spacing_pct": 1.0,
    "grid_levels": 10,
    "amount_per_level": "100000"
  }
}
```

**기대 결과**: 1% 간격으로 매수/매도 그리드 배치, 횡보장에서 높은 거래 빈도

---

### 3. 볼린저 밴드 (bollinger)

```json
{
  "strategy_id": "bollinger",
  "symbol": "005930",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "period": 20,
    "std_multiplier": 1.5,
    "use_rsi_confirmation": false,
    "min_bandwidth_pct": 0.0,
    "amount": "100000"
  }
}
```

**테스트 결과**: 3회 거래, -0.58% 수익률
**참고**:
- `std_multiplier` (std_dev 아님): 1.5로 낮추면 밴드 터치 확률 증가
- `use_rsi_confirmation: false`: RSI 확인 비활성화 (일봉에서 필수)
- `min_bandwidth_pct: 0.0`: 최소 밴드폭 제한 제거

---

### 4. 변동성 돌파 (volatility_breakout)

```json
{
  "strategy_id": "volatility_breakout",
  "symbol": "005930",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "k_factor": 0.3,
    "lookback_period": 1,
    "use_atr": true,
    "atr_period": 5,
    "min_range_pct": 0.1,
    "amount": "100000"
  }
}
```

**테스트 결과**: 28회 거래, -2.60% 수익률
**참고**:
- `k_factor: 0.3`: 낮을수록 돌파 신호 발생 확률 증가
- `use_atr: true`: ATR 기반 레인지 계산 (일봉에 적합)
- `atr_period: 5`: 짧은 ATR 기간
- `min_range_pct: 0.1`: 최소 레인지 요구사항 완화

---

### 5. Magic Split (magic_split)

```json
{
  "strategy_id": "magic_split",
  "symbol": "305540",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "levels": [
      {"number": 1, "target_rate": "10.0", "trigger_rate": null, "invest_money": "200000"},
      {"number": 2, "target_rate": "2.0", "trigger_rate": "-3.0", "invest_money": "100000"},
      {"number": 3, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
      {"number": 4, "target_rate": "3.0", "trigger_rate": "-5.0", "invest_money": "100000"},
      {"number": 5, "target_rate": "4.0", "trigger_rate": "-6.0", "invest_money": "100000"}
    ],
    "allow_same_day_reentry": false,
    "slippage_tolerance": "1.0"
  }
}
```

**기대 결과**: 단계적 물타기 및 익절 실현

---

### 6. 이동평균 크로스오버 (sma_crossover)

```json
{
  "strategy_id": "sma_crossover",
  "symbol": "005930",
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "short_period": 10,
    "long_period": 20,
    "amount": "100000"
  }
}
```

**기대 결과**: 골든크로스 매수, 데드크로스 매도

---

## 다중 자산 전략 테스트 시나리오

### 7. Compound Momentum (compound_momentum)

```json
{
  "strategy_id": "compound_momentum",
  "symbols": ["TQQQ", "SCHD", "PFIX", "TMF"],
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "market": "US",
    "aggressive_asset": "TQQQ",
    "aggressive_weight": "0.5",
    "dividend_asset": "SCHD",
    "dividend_weight": "0.2",
    "rate_hedge_asset": "PFIX",
    "rate_hedge_weight": "0.15",
    "bond_leverage_asset": "TMF",
    "bond_leverage_weight": "0.15",
    "ma_period": 130,
    "rebalance_interval_months": 1,
    "invest_rate": "1.0",
    "rebalance_threshold": "0.03"
  }
}
```

**요구사항**: 다중 심볼 데이터 로딩 지원 필요

---

### 8. HAA (haa)

```json
{
  "strategy_id": "haa",
  "symbols": ["TIP", "SPY", "IWM", "VEA", "VWO", "TLT", "IEF", "PDBC", "VNQ", "BIL"],
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "market": "US",
    "offensive_top_n": 4,
    "defensive_top_n": 1,
    "cash_symbol": "BIL",
    "invest_rate": "1.0",
    "rebalance_threshold": "0.03"
  }
}
```

**요구사항**: 카나리아 자산(TIP) 모멘텀 기반 위험 감지

---

### 9. XAA (xaa)

```json
{
  "strategy_id": "xaa",
  "symbols": ["VWO", "BND", "SPY", "EFA", "EEM", "TLT", "IEF", "LQD", "BIL"],
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "market": "US",
    "offensive_top_n": 4,
    "bond_top_n": 2,
    "safe_top_n": 1,
    "invest_rate": "1.0",
    "rebalance_threshold": "0.03"
  }
}
```

---

### 10. 종목 갈아타기 (stock_rotation)

```json
{
  "strategy_id": "stock_rotation",
  "symbols": ["005930", "000660", "035420", "051910", "006400"],
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": {
    "market": "KR",
    "top_n": 3,
    "invest_rate": "1.0",
    "rebalance_threshold": "0.05",
    "min_momentum": "0",
    "cash_reserve_rate": "0.1"
  }
}
```

---

## 테스트 심볼 목록

### 한국 주식 (KRX)
| 코드 | 종목명 | 용도 |
|-----|-------|-----|
| 005930 | 삼성전자 | 일반 테스트 |
| 000660 | SK하이닉스 | 종목 갈아타기 |
| 035420 | NAVER | 종목 갈아타기 |
| 051910 | LG화학 | 종목 갈아타기 |
| 305540 | TIGER 2차전지 | Magic Split |

### 미국 주식 (NYSE/NASDAQ)
| 티커 | 종목명 | 용도 |
|-----|-------|-----|
| SPY | S&P 500 ETF | HAA/XAA |
| QQQ | Nasdaq 100 ETF | XAA |
| TQQQ | 3x Nasdaq ETF | CompoundMomentum |
| SCHD | 배당 ETF | CompoundMomentum |
| TLT | 20년 국채 ETF | HAA/XAA |
| IEF | 7-10년 국채 ETF | HAA/XAA |
| BIL | 단기 국채 ETF | 현금 대용 |

---

## 다중 자산 전략 백테스트 API 설계

현재 백테스트 엔진은 단일 심볼만 지원합니다. 다중 자산 전략 지원을 위해 다음 수정이 필요합니다:

### 요청 형식 (제안)

```json
POST /api/v1/backtest/run-multi
{
  "strategy_id": "haa",
  "symbols": ["TIP", "SPY", "TLT", "IEF", "BIL"],
  "initial_capital": 10000000,
  "start_date": "2024-01-01",
  "end_date": "2025-01-01",
  "params": { ... }
}
```

### 구현 우선순위
1. 단일 자산 전략 파라미터 튜닝 (볼린저, 변동성 돌파)
2. 다중 자산 데이터 로딩 함수 구현
3. 다중 자산 백테스트 API 엔드포인트 추가
