> ⚠️ **투자 관련 안내사항 (Disclaimer)**
>
> 본 소프트웨어는 **학습 및 연구 목적**으로 개발되었습니다.
>
> - **투자 권유가 아닙니다**: 본 시스템의 신호, 분석, 전략은 투자 조언이나 권유가 아닙니다.
> - **손실 위험**: 모든 투자에는 원금 손실의 위험이 있습니다. 과거 성과가 미래 수익을 보장하지 않습니다.
> - **자기 책임**: 본 소프트웨어를 사용한 모든 투자 결정과 그 결과는 사용자 본인의 책임입니다.
> - **전문가 상담 권장**: 실제 투자 전 공인된 금융 전문가와 상담하시기 바랍니다.
> - **테스트 권장**: 실제 자금 투입 전 모의투자 환경에서 충분히 테스트하세요.
>
> 개발자는 본 소프트웨어 사용으로 인한 직접적, 간접적 손실에 대해 어떠한 책임도 지지 않습니다.

---

<p align="center">
  <h1 align="center">ZeroQuant</h1>
  <p align="center">
    <strong>Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템</strong>
  </p>
  <p align="center">
    <a href="#주요-기능">주요 기능</a> •
    <a href="#지원-전략">지원 전략</a> •
    <a href="#설치-가이드">설치 가이드</a> •
    <a href="#데이터-수집기-collector">수집기</a> •
    <a href="#마이그레이션-가이드">마이그레이션</a> •
    <a href="#전략-개발-가이드">전략 개발</a> •
    <a href="#문서">문서</a>
  </p>
</p>

---

## 소개

ZeroQuant는 암호화폐와 주식 시장에서 **24/7 자동화된 거래**를 수행하는 트레이딩 시스템입니다.

검증된 **16가지 통합 전략**과 **50개 ML 패턴 인식** (캔들스틱 26개 + 차트 패턴 24개)을 통해 **그리드 트레이딩**, **자산배분**, **모멘텀** 등 다양한 투자 방법론을 지원합니다. 웹 대시보드에서 실시간 모니터링과 전략 제어가 가능하며, 리스크 관리 시스템이 자동으로 자산을 보호합니다.

> ⚠️ **v0.8.3 쿼리 최적화, 백테스트 타임프레임 폴백, UI 성능**: OHLCV 배치 쿼리를 LATERAL JOIN + TimescaleDB 청크 프루닝으로 ~70% 최적화하고, 백테스트/CLI에 전략 기본 타임프레임 자동 폴백을 도입했습니다. 스크리닝 가상 스크롤(11,000+ 행 60fps), 상태/등급/점수 정렬, 매매일지 상세 통계가 추가되었습니다.

## 주요 기능

### 🏦 다중 시장 지원
| 시장 | 거래소 | 기능 |
|------|--------|------|
| 암호화폐 | Binance | 현물 거래, WebSocket 실시간 시세 |
| 한국/미국 주식 | 한국투자증권 (KIS) | 국내/해외 주식, WebSocket 실시간 시세, 모의투자 지원 |

### 📝 Paper Trading (v0.8.0)
- **가상 계좌 시뮬레이션**: 실시간 시세 기반 가상 거래 실행
- **전략 검증**: 실제 자금 없이 전략 성과 검증
- **실시간 포지션 추적**: 가상 포지션 및 체결 내역 실시간 모니터링
- **계좌 초기화**: 가상 계좌 리셋 후 재시작 가능

### 📊 데이터 & 분석
- **다중 데이터 소스**: KRX OPEN API, 네이버 금융 (국내), Yahoo Finance (해외/암호화폐)
  - 데이터 프로바이더 토글 지원 (`PROVIDER_KRX_API_ENABLED`, `PROVIDER_YAHOO_ENABLED`, `NAVER_FUNDAMENTAL_ENABLED`)
  - 네이버 금융 크롤링으로 국내 펀더멘털 데이터 수집 속도 개선
- **다중 타임프레임 분석**: 여러 시간대 데이터 동시 분석 (1분~월봉)
  - Look-Ahead Bias 방지 자동 정렬
  - 크로스 타임프레임 시그널 결합
- **실시간 시세**: WebSocket 기반 실시간 가격/호가/체결 (KIS 국내/해외 + Binance)
  - Singleton 스트림 관리 (credential별 하나의 WebSocket)
  - 동적 구독/해제 (전략 추가/삭제 시 런타임 반영)
  - 프론트엔드 구독 → 거래소 스트림 자동 브릿지
- **OHLCV 배치 쿼리**: LATERAL JOIN + TimescaleDB 청크 프루닝으로 2,500+ 심볼 동시 조회 300ms 이내
- **과거 데이터**: TimescaleDB 시계열 저장, 백테스팅 지원
- **데이터셋 관리**: Yahoo Finance 데이터 다운로드, 캔들 데이터 CRUD
- **백그라운드 수집**: 펀더멘털 데이터 자동 수집, 심볼 자동 동기화 (KRX/Binance/Yahoo)
- **ML 패턴 인식**: 캔들스틱 26개 + 차트 패턴 24개 (ONNX 추론)
- **ML 모델 훈련**: XGBoost, LightGBM, RandomForest, 앙상블 지원
- **성과 지표**: Sharpe Ratio, MDD, Win Rate, CAGR 등

### 🎯 고급 스코어링 시스템
- **Global Score Ranking**: 7개 팩터 기반 종합 종목 평가
  - VolumeQuality, Momentum, ValueFactor, RouteState 등
  - 페널티 시스템 (LiquidityGate, MarketRegime 필터)
- **7Factor Scoring**: 정규화된 다요인 분석 (0-100 점수)
  - Momentum, Value, Quality, Volatility, Liquidity, Growth, Sentiment
- **RouteState Calculator**: 진입 타이밍 자동 판단
  - ATTACK (진입 적기), ARMED (대기), WAIT (관찰), OVERHEAT (과열)
  - TTM Squeeze, 모멘텀, RSI, Range 종합 분석
- **Market Regime**: 5단계 추세 분류 (STRONG_UPTREND → DOWNTREND)
- **Reality Check**: 추천 종목 실제 성과 자동 검증
  - 전일 추천 → 익일 성과 자동 계산
  - 일별/소스별/랭크별 승률 통계

### 🤖 알림 & 모니터링
- **다채널 알림**: Telegram, Discord, Slack, Email, SMS (Twilio) 지원
  - 포지션 현황 및 손익 업데이트
  - 거래 체결 알림
  - 전략 신호 알림
- **Signal System**: 백테스트/실거래 신호 저장
  - 신호 마커 (차트 표시용)
  - 알림 규칙 관리 (JSONB 필터)

### 🛡️ 리스크 관리
- **ExitConfig 통합 리스크 관리** (v0.8.2): 6가지 리스크 옵션을 전략별로 조합
  - **StopLoss**: 고정 % 손절 또는 ATR(평균 진폭 범위) 기반 동적 손절
  - **TakeProfit**: 고정 % 익절
  - **TrailingStop**: 4가지 모드 (고정 %, ATR 기반, 단계별 Step, Parabolic SAR)
  - **ProfitLock**: 수익 보호 — 목표 수익 달성 후 일정 비율 이하 하락 시 자동 청산
  - **DailyLossLimit**: 일일 최대 손실 비율 초과 시 거래 중단
  - **반대 신호 청산**: 보유 중 반대 방향 신호 발생 시 즉시 청산
- **6가지 프리셋** 제공 (day_trading, mean_reversion, grid, rebalancing, leverage, momentum)
- 전략마다 `exit_config()` trait 메서드로 리스크 설정을 엔진에 전달
- `enrich_signal()`: Entry 신호에 SL/TP 가격 자동 계산, 트레일링/수익잠금은 metadata로 executor에 전달
- Circuit Breaker 패턴 (에러 카테고리별 차등 임계치)
- API 재시도 시스템 (지수 백오프, Rate Limit 대응)

### 🖥️ 웹 대시보드
- 실시간 포트폴리오 모니터링
- 전략 등록/시작/중지/설정 (SDUI 동적 폼)
- 데이터셋 관리 (심볼 데이터 다운로드/조회/삭제)
- 백테스트 실행 및 결과 저장/비교
- ML 모델 훈련 및 관리
- 동기화된 멀티 차트 패널
- 거래소 API 키 관리 (AES-256-GCM 암호화)

### 📒 매매일지 (Trading Journal)
거래 내역을 체계적으로 관리하고 투자 성과를 분석합니다:
- **체결 내역 동기화**: 거래소 API에서 자동 수집
- **종목별 보유 현황**: 보유 수량, 평균 매입가, 투자 금액
- **물타기 자동 계산**: 추가 매수 시 가중평균 매입가 자동 갱신
- **FIFO 실현손익**: 선입선출 방식 실현손익 계산 (로트별 추적)
- **손익 분석**: 실현/미실현 손익, 기간별 수익률
- **매매 패턴 분석**: 빈도, 성공률, 평균 보유 기간
- **포트폴리오 비중**: 종목별 비중 시각화 및 리밸런싱 추천
- **백테스트 인사이트**: 매수/매도 건수, 총 거래량, 활성 거래일, 최대 수익/손실 등 상세 통계

## 지원 전략

### 통합 전략 (v0.7.0)

#### 📈 단기/분할 전략
| 전략 | 설명 |
|------|------|
| **DCA** | 분할매수 전략 (그리드, 분할, 무한 매수) |
| **Day Trading** | 단기 매매 전략 |
| **Mean Reversion** | 평균회귀 전략 |
| **Candle Pattern** | 캔들스틱 패턴 인식 |

#### 📊 모멘텀/로테이션 전략
| 전략 | 설명 |
|------|------|
| **Rotation** | 모멘텀 로테이션 전략 |
| **Compound Momentum** | 공격/안전 자산 전환 |
| **Momentum Power** | ETF 모멘텀 전략 |

#### 🏦 자산배분 전략
| 전략 | 설명 |
|------|------|
| **Asset Allocation** | 자산배분 전략 |
| **Pension Portfolio** | 연금 포트폴리오 전략 |

#### 🎯 기타 전략
| 전략 | 설명 |
|------|------|
| **Sector VB** | 변동성 돌파 전략 |
| **US 3X Leverage** | 레버리지 ETF 전략 |
| **Momentum Surge** | 급등 모멘텀 전략 |
| **Market BothSide** | 양방향 매매 전략 |
| **Small Cap Quant** | 소형주 퀀트 전략 |
| **Range Trading** | 구간별 분할 매매 |
| **RSI Multi Timeframe** | 다중 타임프레임 전략 |

## 전략 실행 아키텍처 (v0.8.0)

전략은 데이터 소스와 주문 실행 방식을 알 필요 없이, **Signal만 발행**하면 됩니다. 데이터와 실행은 의존성 주입으로 교체됩니다.

### 실행 흐름

```
┌─────────────────────────────────────────┐
│  DataProvider (교체 가능)                │
│  • ExchangeProvider (실시간 데이터)      │
│  • BacktestEngine (과거 데이터)          │
└─────────────────┬───────────────────────┘
                  ▼
           ┌─────────────┐
           │   Strategy   │ → Signal 발행
           └──────┬──────┘
                  ▼
┌─────────────────────────────────────────┐
│  SignalProcessor (교체 가능)             │
│  • OrderExecutor (실제 주문)             │
│  • SimulatedExecutor (가상 체결)         │
└─────────────────────────────────────────┘
```

### 실행 모드 조합

| 데이터 소스 | Signal 처리 | 결과 |
|------------|-------------|------|
| ExchangeProvider | OrderExecutor | **실거래** |
| ExchangeProvider | SimulatedExecutor | **페이퍼 트레이딩** |
| BacktestEngine | SimulatedExecutor | **백테스트** |

### SignalProcessor trait

`SignalProcessor`는 전략이 발행한 Signal을 받아 주문을 실행하는 핵심 trait입니다.

```rust
#[async_trait]
pub trait SignalProcessor: Send + Sync {
    /// Signal 수신 → 주문 실행 → TradeResult 반환
    async fn process_signal(&mut self, signal: &Signal, price: Decimal) -> Result<Option<TradeResult>>;

    /// 잔고 / 포지션 / 체결 내역 조회
    fn balance(&self) -> Decimal;
    fn positions(&self) -> &HashMap<String, ProcessorPosition>;
    fn trades(&self) -> &[TradeResult];

    /// 그룹별 포지션 관리 (그리드/분할매수 전략용)
    fn positions_by_group(&self, group_id: &str) -> Vec<&ProcessorPosition>;
    fn position_keys_by_group(&self, group_id: &str) -> Vec<String>;
    fn group_unrealized_pnl(&self, group_id: &str, current_prices: &HashMap<String, Decimal>) -> Decimal;
}
```

| 구현체 | 위치 | 용도 |
|--------|------|------|
| `SimulatedExecutor` | `trader-execution/src/simulated_executor.rs` | 백테스트, 페이퍼 트레이딩 |
| `LiveExecutor` | `trader-execution/src/live_executor.rs` | 실거래 (OrderExecutionProvider 주입) [v0.8.1] |
| `OrderExecutor` (레거시) | `trader-execution/src/executor.rs` | 레거시 실행기 |

### Position ID / Group ID 시스템

스프레드 기반 전략(그리드, 분할매수 등)에서 **레벨별 독립 포지션 관리**를 지원합니다.

```rust
Signal {
    ticker: "005930",              // 실제 거래 심볼
    position_id: "005930_grid_L1", // 개별 포지션 식별
    group_id: "grid_55000_1707..", // 관련 포지션 그룹
}
```

| 전략 | position_id 형식 | group_id 형식 |
|------|-----------------|---------------|
| Grid Trading | `{ticker}_grid_L{level}` | `grid_{base_price}_{timestamp}` |
| Magic Split | `{ticker}_split_L{level}` | `split_{ticker}_{timestamp}` |
| Infinity Bot | `{ticker}_inf_R{round}` | `inf_{ticker}_{timestamp}` [v0.8.1] |

### StrategyContext (전략 컨텍스트)

전략에 주입되는 `Arc<RwLock<StrategyContext>>`는 거래소 데이터와 분석 데이터를 통합 제공합니다.

```
StrategyContext
├── 거래소 데이터 (1~5초 갱신)
│   ├── account          # 계좌 정보 (잔고, 증거금)
│   ├── positions         # 보유 포지션
│   ├── pending_orders    # 미체결 주문
│   └── exchange_constraints # 거래소 제약 (최소 주문량, 수수료율)
│
├── 분석 데이터 (1~10분 갱신)
│   ├── global_scores     # 종목별 종합 점수 (7팩터)
│   ├── route_states      # 종목별 진입 상태 (ATTACK/ARMED/WAIT/OVERHEAT)
│   ├── screening_results # 스크리닝 결과
│   ├── structural_features # 구조적 피처 (추세, 변동성)
│   ├── trigger_results   # 진입 트리거 결과 (급등시동, 박스돌파 등)
│   ├── market_regime     # 시장 국면 (5단계)
│   ├── market_breadth    # 시장 폭 지표
│   └── macro_environment # 매크로 환경
│
└── 시장 데이터
    └── klines_by_timeframe # 멀티 타임프레임 캔들 데이터
```

**전략별 권장 활용 (v0.8.1 재설계):**

| 전략 유형 | RouteState | GlobalScore | 스크리닝 |
|----------|------------|-------------|----------|
| 단일 티커 | Overheat만 차단 | 미사용 또는 강도 조정 | - |
| US ETF (자산배분/모멘텀) | Overheat만 차단 | 미사용 (자체 모멘텀) | - |
| 로테이션 (KR) | Overheat만 차단 | 필터 유지 | `get_tickers_by_global_score` |
| 로테이션 (US) | Overheat만 차단 | 미사용 | - |
| DCA (Grid/분할매수) | 미사용 (순수 가격 기반) | 미사용 | - |
| 캔들 패턴 | Overheat만 차단 | 강도 조정 (0.75x~1.25x) | - |

### Signal Filter 시스템 (v0.8.0)

전략이 발행한 Signal을 실행 전 다단계로 필터링합니다.

```rust
// CompositeFilter: 여러 필터를 체인으로 연결
let mut filter = CompositeFilter::new();
filter.add_filter(Box::new(VolumeFilter::new(1.5)));  // 거래량 150% 이상
filter.add_filter(Box::new(TrendFilter::new()));       // 추세 방향 일치

let result = filter.filter(&signal, &context);
if result.is_valid {
    // Signal 실행
}
```

| 필터 | 역할 | 조건 |
|------|------|------|
| `VolumeFilter` | 거래량 검증 | 평균 거래량 대비 최소 비율 |
| `TrendFilter` | 추세 방향 일치 | 매수 신호 + 상승 추세 |
| `CompositeFilter` | 다중 필터 체인 | 모든 필터 통과 시 유효 |
| `ConfirmationPattern` | 연속 확인 | N회 연속 신호 발생 시 유효 |

### Exchange Provider 추상화

거래소 연동은 두 가지 trait로 추상화되어, 전략과 거래소가 완전히 분리됩니다.

```rust
/// 계좌/포지션/주문 관리
#[async_trait]
pub trait ExchangeProvider: Send + Sync {
    async fn fetch_account(&self) -> Result<AccountInfo>;
    async fn fetch_positions(&self) -> Result<Vec<PositionInfo>>;
    async fn fetch_pending_orders(&self) -> Result<Vec<PendingOrder>>;
    async fn fetch_execution_history(&self, req: ExecutionHistoryRequest) -> Result<ExecutionHistoryResponse>;
}

/// 시세 조회
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn get_quote(&self, symbol: &str) -> Result<QuoteData>;
    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<QuoteData>>;
}
```

| Provider | 구현 | 용도 |
|----------|------|------|
| `KisExchangeProvider` | `provider/kis.rs` | KIS 한국/미국 주식 (통합) |
| `BinanceExchangeProvider` | `provider/binance.rs` | Binance 암호화폐 |
| `MockExchangeProvider` | `provider/mock.rs` | 개발/테스트, Paper Trading |

### MarketStream 실시간 스트림 (v0.8.0)

```
[KIS WebSocket KR/US] ─── Bridge Task (mpsc) ──→ [UnifiedMarketStream]
                                                        │
                                            ┌───────────┤
                                            ▼           ▼
                                  [MarketDataAggregator] [동적 구독 command channel]
                                            │
                                            ▼
                                  [SubscriptionManager] → Frontend WebSocket
```

| 패턴 | 설명 |
|------|------|
| **Singleton** | credential_id별 단일 스트림 (여러 전략이 공유) |
| **Bridge Task** | KR/US 개별 태스크 → mpsc 채널로 이벤트 통합 |
| **참조 카운트** | 구독 심볼의 사용자 수 추적, 0이면 구독 해제 |
| **Lazy 초기화** | 전략 시작 시 DB 자격증명 기반으로 스트림 자동 생성 |
| **동적 구독** | 연결 중에도 심볼 추가/해제 가능 |

---

## 전략 개발 가이드

ZeroQuant는 플러그인 기반 전략 시스템을 지원합니다. `Strategy` trait를 구현하여 새로운 전략을 추가할 수 있습니다.

### Strategy Trait

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    // ── 필수 구현 ──────────────────────────────────────────
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;

    /// 설정으로 전략 초기화
    async fn initialize(&mut self, config: Value)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 새 시장 데이터 수신 시 호출 → 트레이딩 신호 반환
    async fn on_market_data(&mut self, data: &MarketData)
        -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>>;

    /// 주문 체결 시 호출
    async fn on_order_filled(&mut self, order: &Order)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 포지션 업데이트 시 호출
    async fn on_position_update(&mut self, position: &Position)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 전략 종료 및 리소스 정리
    async fn shutdown(&mut self)
        -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 현재 전략 상태 (모니터링용)
    fn get_state(&self) -> Value;

    // ── 기본 구현 제공 (선택적 오버라이드) ─────────────────
    /// StrategyContext 주입 (거래소 데이터, GlobalScore, RouteState 접근)
    fn set_context(&mut self, _context: Arc<RwLock<StrategyContext>>) {}

    /// 다중 타임프레임 설정 (None이면 단일 타임프레임)
    fn multi_timeframe_config(&self) -> Option<MultiTimeframeConfig> { None }

    /// 다중 타임프레임 데이터로 신호 생성 (기본: on_market_data 호출)
    async fn on_multi_timeframe_data(
        &mut self, primary_data: &MarketData,
        _secondary_data: &HashMap<Timeframe, Vec<Kline>>,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>>;

    /// 영속성을 위한 상태 저장/로드
    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> { Ok(vec![]) }
    fn load_state(&mut self, _data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}
```

### 새 전략 추가 방법

#### 1. 전략 Config 정의 (SDUI 스키마 자동 생성)

`crates/trader-strategy/src/strategies/my_strategy.rs`

```rust
use crate::Strategy;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_strategy_macro::StrategyConfig;  // SDUI 스키마 자동 생성 매크로

// 공통 모듈 임포트 (v0.7.0+)
use super::common::{
    ExitConfig, calculate_rsi, PositionSizer, RiskChecker,
};

/// 전략 설정 - SDUI 스키마 자동 생성
#[derive(Debug, Clone, Serialize, Deserialize, StrategyConfig)]
#[strategy(
    id = "my_strategy",                    // 전략 고유 ID (URL/API에서 사용)
    name = "나만의 RSI 전략",               // UI 표시명
    description = "RSI 과매수/과매도 평균회귀", // 전략 설명
    category = "Intraday"                  // 카테고리: Intraday, Swing, Position
)]
pub struct MyStrategyConfig {
    /// 거래 종목
    #[serde(default = "default_ticker")]
    #[schema(
        label = "거래 종목",      // UI 라벨
        field_type = "symbol",   // 필드 타입: symbol, number, integer, boolean, select
        default = "005930",      // 기본값
        section = "asset"        // UI 섹션 그룹
    )]
    pub ticker: String,

    /// 거래 금액
    #[serde(default = "default_amount")]
    #[schema(label = "거래 금액", field_type = "number", min = 10000, max = 100000000, default = 1000000, section = "asset")]
    pub amount: Decimal,

    /// RSI 기간
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", field_type = "integer", min = 2, max = 100, default = 14, section = "indicator")]
    pub rsi_period: usize,

    /// 과매도 임계값
    #[serde(default = "default_oversold")]
    #[schema(label = "과매도 임계값", field_type = "number", min = 0, max = 50, default = 30, section = "indicator")]
    pub oversold: Decimal,

    /// 과매수 임계값
    #[serde(default = "default_overbought")]
    #[schema(label = "과매수 임계값", field_type = "number", min = 50, max = 100, default = 70, section = "indicator")]
    pub overbought: Decimal,

    /// 청산 설정 (공통 스키마 프래그먼트 참조)
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,

}

// 기본값 함수들
fn default_ticker() -> String { "005930".to_string() }
fn default_amount() -> Decimal { dec!(1000000) }
fn default_rsi_period() -> usize { 14 }
fn default_oversold() -> Decimal { dec!(30) }
fn default_overbought() -> Decimal { dec!(70) }

impl Default for MyStrategyConfig {
    fn default() -> Self {
        Self {
            ticker: default_ticker(),
            amount: default_amount(),
            rsi_period: default_rsi_period(),
            oversold: default_oversold(),
            overbought: default_overbought(),
            exit_config: ExitConfig::default(),
        }
    }
}
```

#### 2. Strategy Trait 구현

```rust
pub struct MyStrategy {
    config: MyStrategyConfig,
    context: Option<Arc<RwLock<StrategyContext>>>,  // RouteState 접근용
}

#[async_trait]
impl Strategy for MyStrategy {
    fn name(&self) -> &str { "my_strategy" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "RSI 기반 평균회귀 전략" }

    // StrategyContext 주입 (GlobalScore, RouteState 접근용)
    fn set_context(&mut self, ctx: Arc<RwLock<StrategyContext>>) {
        self.context = Some(ctx);
    }

    async fn on_market_data(&mut self, data: &MarketData)
        -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        let mut signals = vec![];

        // RouteState 과열 차단 (v0.8.1 공통 패턴)
        if let Some(ctx) = &self.context {
            let ctx = ctx.read().await;
            if let Some(state) = ctx.get_route_state(&self.config.ticker) {
                if state == RouteState::Overheat {
                    return Ok(vec![]); // 과열 종목 진입 차단
                }
            }
        }

        // RSI 계산 및 신호 생성
        let rsi = calculate_rsi(&data.closes, self.config.rsi_period)?;

        if rsi < self.config.oversold {
            signals.push(Signal::buy(&self.config.ticker, data.close));
        } else if rsi > self.config.overbought {
            signals.push(Signal::sell(&self.config.ticker, data.close));
        }

        Ok(signals)
    }
    // ... 나머지 필수 구현 (initialize, on_order_filled, on_position_update, shutdown, get_state)
}
```

#### 3. 모듈 및 레지스트리 등록

**모듈 등록**: `crates/trader-strategy/src/strategies/mod.rs`
```rust
pub mod my_strategy;
pub use my_strategy::*;
```

**전략 레지스트리 등록**: 파일 하단에 `register_strategy!` 매크로 호출
```rust
// inventory 기반 자동 등록 (컴파일 타임에 StrategyMeta 생성)
register_strategy! {
    id: "my_strategy",
    aliases: ["my_rsi"],
    name: "나만의 RSI 전략",
    description: "RSI 과매수/과매도 평균회귀",
    timeframe: "15m",
    tickers: [],                              // 빈 배열 = 사용자 지정
    category: Intraday,                       // Realtime, Intraday, Daily, Monthly
    markets: [Crypto, Stock],                 // 지원 시장
    type: MyStrategy,                         // ::new()로 생성 (또는 factory: MyStrategy::variant)
    config: MyStrategyConfig                  // SDUI 스키마 자동 생성
}
```

#### 4. ExitConfig 통합 리스크 관리 (v0.8.2)

전략별로 6가지 리스크 옵션을 `enabled: bool`로 독립 활성화/비활성화하여 조합합니다.
`Strategy` trait의 `exit_config()` 메서드로 엔진에 전달하면, `enrich_signal()`이 Entry 신호에 SL/TP 가격과 metadata를 자동 적용합니다.

**리스크 옵션 상세:**

| 옵션 | 설명 | 모드/파라미터 | 동작 방식 |
|------|------|-------------|----------|
| **StopLoss** | 손절 | `Fixed` (고정 %), `AtrBased` (ATR 배수) | Fixed: entry_price ± pct% / ATR: executor에서 ATR 값으로 동적 계산 |
| **TakeProfit** | 익절 | `pct` (고정 %) | entry_price ± pct% |
| **TrailingStop** | 트레일링 스톱 | `FixedPercentage`, `AtrBased`, `Step`, `ParabolicSar` | trigger_pct 도달 후 고점 대비 stop_pct 하락 시 청산 |
| **ProfitLock** | 수익 잠금 | `threshold_pct`, `lock_pct` | 수익이 threshold 달성 → 수익의 lock% 이하 하락 시 청산 (예: 5%수익, 80%잠금 → 4% 이하면 청산) |
| **DailyLossLimit** | 일일 손실 한도 | `max_loss_pct` | 계좌 대비 일일 손실이 max_loss_pct 초과 시 거래 중단 |
| **반대 신호 청산** | exit_on_opposite_signal | `bool` | 보유 중 반대 방향 Entry 발생 시 기존 포지션 즉시 청산 |

**TrailingStop 4가지 모드:**

| 모드 | 설명 | 파라미터 |
|------|------|---------|
| `FixedPercentage` | 고정 % 트레일링 | trigger_pct (시작 수익률), stop_pct (고점 대비 하락폭) |
| `AtrBased` | ATR 기반 동적 트레일링 | trigger_pct, atr_multiplier (ATR × 배수만큼 트레일) |
| `Step` | 단계별 트레일링 | step_levels: [{profit_pct, trail_pct}, ...] 수익 구간별 차등 트레일 |
| `ParabolicSar` | Parabolic SAR 지표 기반 | 지표가 가격과 교차 시 청산 |

**6가지 프리셋:**

| 프리셋 | 손절 | 익절 | 트레일링 | 반대 청산 | 적용 대상 |
|--------|------|------|----------|----------|----------|
| `for_day_trading()` | 2% Fixed | 4% | ❌ | ✅ | day_trading, sector_vb, momentum_surge |
| `for_mean_reversion()` | 3% Fixed | 6% | ❌ | ✅ | mean_reversion, range_trading, candle_pattern |
| `for_grid_trading()` | ❌ | 3% | ❌ | ❌ | DCA (grid, magic_split, infinity_bot) |
| `for_rebalancing()` | ❌ | ❌ | ❌ | ❌ | asset_allocation, rotation, pension_bot |
| `for_leverage()` | 5% Fixed | 10% | ✅ (5%→2%) | ✅ | us_3x_leverage, market_bothside |
| `for_momentum()` | 5% Fixed | 15% | ✅ (8%→3%) | ✅ | compound_momentum, momentum_power, rsi_multi_tf |

```rust
// Strategy trait에서 exit_config() 반환 → 엔진이 enrich_signal() 자동 적용
fn exit_config(&self) -> Option<&ExitConfig> {
    Some(&self.config.exit_config)
}

// 커스텀 빌더 패턴
let config = ExitConfig::for_momentum()
    .stop_loss(StopLossConfig { mode: StopLossMode::AtrBased, atr_multiplier: dec!(2.0), ..Default::default() })
    .trailing_stop(TrailingStopConfig { mode: TrailingMode::Step, step_levels: vec![
        StepLevel { profit_pct: dec!(5), trail_pct: dec!(3) },
        StepLevel { profit_pct: dec!(10), trail_pct: dec!(2) },
    ], ..Default::default() });
```

### CLI로 전략 테스트 (v0.7.0+)

```bash
# 단일 심볼 테스트
trader strategy-test --strategy my_strategy --symbol 005930 --market KR

# 다중 심볼 테스트 (로테이션 전략)
trader strategy-test --strategy rotation --symbols "SPY,QQQ,IWM" --market US

# JSON config로 상세 테스트
trader strategy-test --strategy my_strategy --config '{"ticker":"005930","rsi_period":14}'

# 디버그 모드 (상세 진단 정보 출력)
trader strategy-test --strategy my_strategy --symbol 005930 --debug
```

### 백테스트 CLI (v0.8.1+)

```bash
# TOML 설정 파일로 백테스트 실행
trader backtest --config config/backtest/rsi.toml

# 자동 차트 생성 (equity curve, drawdown, trade markers)
trader backtest --config config/backtest/grid.toml --chart

# 멀티에셋 전략 (HAA, XAA, StockRotation 등 자동 감지)
trader backtest --config config/backtest/haa.toml

# 사전 정의 설정 파일 (config/backtest/)
# rsi.toml, bollinger.toml, grid.toml, magic_split.toml, infinity_bot.toml,
# haa.toml, xaa.toml, compound_momentum.toml, stock_rotation.toml, volatility.toml
```

**기능:**
- **멀티에셋 자동 감지**: 다중 심볼 전략은 StrategyContext와 함께 자동 실행
- **스마트 심볼 해석**: KR 종목코드를 `.KS`/`.KQ` 접미사로 자동 변환
- **Signal Analysis Report**: Entry/Exit 균형, 레벨별 분포, 검증 체크리스트 자동 출력
- **회귀 차트**: `--chart` 옵션으로 equity curve, drawdown, 거래 마커 포함 PNG 생성
- **타임프레임 자동 폴백**: 전략 기본 타임프레임 → secondary → 일반(1m~1d) 순서로 가용 데이터 자동 탐색

### 전략 구조

```
crates/trader-strategy/
├── src/
│   ├── lib.rs              # 모듈 진입점
│   ├── traits.rs           # Strategy trait 정의
│   ├── engine.rs           # 전략 엔진 (로딩/실행)
│   ├── plugin/             # 동적 플러그인 로더
│   └── strategies/
│       ├── mod.rs          # 전략 모듈 목록
│       ├── common/         # 공통 유틸리티 (v0.7.0 대폭 확장)
│       │   ├── exit_config.rs      # 통합 리스크 관리 설정 (5가지 타입) [v0.8.2]
│       │   ├── global_score_utils.rs # GlobalScore 유틸리티
│       │   ├── indicators.rs       # 기술 지표 (RSI, SMA, BB 등)
│       │   ├── position_sizing.rs  # 포지션 사이징
│       │   ├── risk_checks.rs      # 리스크 검증
│       │   └── signal_filters.rs   # 신호 필터링
│       ├── day_trading.rs      # 단타/그리드 전략
│       ├── mean_reversion.rs   # RSI/볼린저 평균회귀
│       ├── rotation.rs         # 모멘텀 로테이션
│       ├── asset_allocation.rs # 자산배분 (HAA/XAA/BAA)
│       └── ...                 # 기타 통합 전략들
```

## 아키텍처

```
zeroquant/
├── crates/
│   ├── trader-core/         # 도메인 모델, 공통 유틸리티
│   │   └── domain/          # Account, ExchangeTypes, Signal, Context [v0.8.0]
│   ├── trader-exchange/     # 거래소 연동 (Binance, KIS) [v0.8.0 통합]
│   │   ├── provider/        # ExchangeProvider (KIS 통합, Mock)
│   │   ├── connector/kis/   # KIS 커넥터 (WebSocket 동적 구독)
│   │   ├── stream.rs        # UnifiedMarketStream (Bridge Task 패턴)
│   │   └── simulated/       # 시뮬레이션 거래소
│   ├── trader-strategy/     # 전략 엔진, 16개 통합 전략
│   │   └── strategies/common/ # 공통 모듈 (exit_config, global_score_utils 등)
│   ├── trader-risk/         # 리스크 관리
│   ├── trader-execution/    # 주문 실행 엔진 (LiveExecutor, SimulatedExecutor) [v0.8.1]
│   ├── trader-data/         # 데이터 수집/저장 (OHLCV)
│   │   └── provider/        # 데이터 프로바이더
│   │       ├── naver.rs         # 네이버 금융 (국내)
│   │       ├── yahoo_fundamental.rs # Yahoo 펀더멘털 (해외)
│   │       └── krx_api.rs       # KRX OPEN API
│   ├── trader-analytics/    # ML 추론, 성과 분석, 패턴 인식, CandleProcessor [v0.8.2]
│   ├── trader-api/          # REST/WebSocket API (OpenAPI 3.0 문서화)
│   │   ├── repository/      # 데이터 접근 계층 (12개 Repository)
│   │   ├── routes/          # 모듈화된 라우트 (paper_trading 추가) [v0.8.0]
│   │   ├── services/        # MarketStreamHandle, SignalProcessor [v0.8.0]
│   │   └── websocket/       # 실시간 연동 (Aggregator, Handler) [v0.8.0]
│   ├── trader-cli/          # CLI 도구
│   │   └── commands/
│   │       ├── strategy_test.rs # 전략 통합 테스트 (v0.7.0 신규)
│   │       └── download.rs      # 데이터 다운로드
│   ├── trader-collector/    # Standalone 데이터 수집기
│   │   └── modules/         # 수집 모듈 (ohlcv, indicator, global_score, market_breadth) [v0.8.0]
│   └── trader-notification/ # 알림 (Telegram, Discord, Slack, Email, SMS)
├── frontend/                # SolidJS + TypeScript + Vite
│   ├── src/pages/
│   │   ├── Dashboard.tsx    # 포트폴리오 모니터링
│   │   ├── Strategies.tsx   # 전략 등록/관리 (SDUI)
│   │   ├── Dataset.tsx      # 데이터셋 관리
│   │   ├── Backtest.tsx     # 백테스트 실행
│   │   ├── Simulation.tsx   # 시뮬레이션 + Paper Trading [v0.8.0]
│   │   ├── MLTraining.tsx   # ML 모델 훈련
│   │   ├── Screening.tsx   # 종목 스크리닝 (가상 스크롤, 정렬) [v0.8.3]
│   │   ├── TradingJournal.tsx # 매매일지 (상세 인사이트) [v0.8.3]
│   │   ├── GlobalRanking.tsx  # 글로벌 랭킹
│   │   └── Settings.tsx     # 설정 (API 키, 알림, 자격증명)
│   ├── src/components/
│   │   └── simulation/      # Paper Trading 컴포넌트 [v0.8.0]
│   └── src/types/generated/ # ts-rs 자동 생성 타입 (paper_trading 추가)
├── migrations_v2/           # 통합 마이그레이션 (9개)
├── scripts/                 # ML 훈련 파이프라인
└── docs/                    # 프로젝트 문서
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Backend | Rust, Tokio, Axum, WebSocket (실시간 시세) |
| Database | PostgreSQL (TimescaleDB), Redis |
| Frontend | SolidJS, TypeScript, Vite |
| ML | ONNX Runtime, XGBoost, LightGBM, RandomForest |
| Exchange | KIS (KR/US 통합), Binance, Mock Provider |
| Testing | Playwright (E2E), pytest (ML) |
| Infrastructure | Podman/Docker, TimescaleDB, Redis |

## 빠른 시작

### 요구사항
- Rust 1.83+ (ONNX Runtime 호환)
- Node.js 18+
- **Podman** (권장) 또는 Docker & Docker Compose
- PostgreSQL 15+ (TimescaleDB) / Redis 7+

### 설치

```bash
# 저장소 클론
git clone https://github.com/berrzebb/zeroquant.git
cd zeroquant

# 환경 설정
cp .env.example .env
```

### Podman 설정 (Windows)

```bash
# Podman 설치
winget install RedHat.Podman

# Podman Machine 초기화 및 시작
podman machine init --cpus=2 --memory=2048 --disk-size=20
podman machine start
```

### 실행 (인프라 + 로컬 개발)

```bash
# 1. 인프라 시작 (DB, Redis) - Podman 또는 Docker 모두 지원
podman compose up -d    # Podman 사용 시
# docker-compose up -d  # Docker 사용 시

# 2. 백엔드 실행 (로컬)
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release  # ML 기능 포함

# 3. 프론트엔드 실행 (로컬)
cd frontend && npm install && npm run dev
```

### 명령어 매핑 (Docker ↔ Podman)

| Docker | Podman |
|--------|--------|
| `docker-compose up -d` | `podman compose up -d` |
| `docker-compose down` | `podman compose down` |
| `docker-compose logs -f` | `podman compose logs -f` |
| `docker exec -it` | `podman exec -it` |
| `docker ps` | `podman ps` |

### ML 모델 훈련

```bash
# Podman으로 ML 훈련 실행
podman compose --profile ml run --rm trader-ml \
  python scripts/train_ml_model.py --symbol SPY --model xgboost

# 사용 가능한 심볼 목록
podman compose --profile ml run --rm trader-ml \
  python scripts/train_ml_model.py --list-symbols
```

### E2E 테스트

```bash
# Playwright 설치
cd frontend && npx playwright install

# E2E 테스트 실행
npm run test:e2e

# 특정 테스트 실행
npx playwright test risk-management-ui.spec.ts
```

---

## 데이터 수집기 (Collector)

`trader-collector`는 시장 데이터를 자동으로 수집하고 분석 지표를 계산하는 독립 실행형 CLI 도구입니다.

### 워크플로우 구조

```
┌─────────────────────────────────────────────────────────────────┐
│                    Group A: 외부 API 워크플로우                   │
│                    (Rate Limited - 60분 주기)                    │
├─────────────────────────────────────────────────────────────────┤
│  1. 심볼 동기화     → KRX, Binance, Yahoo에서 종목 목록 수집      │
│  2. Fundamental    → PER, PBR, ROE, 섹터, 시가총액 등            │
│  3. OHLCV 수집     → 일봉/시간봉/분봉 데이터 수집                 │
│     └── KRX API 직접 수집 (v0.8.0): 누락 범위 자동 감지 + 배치 저장│
│  4. 매크로 데이터   → 환율, 금리, 유가 등 매크로 지표 (Redis 캐시)   │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Group B: 내부 계산 워크플로우                   │
│                    (No Rate Limit - 15분 주기)                   │
├─────────────────────────────────────────────────────────────────┤
│  1. 지표 동기화     → RouteState, MarketRegime, TTM Squeeze      │
│  2. GlobalScore    → 7개 팩터 기반 종합 점수 계산                 │
│  3. 스크리닝 뷰     → Materialized View 갱신                     │
│  4. 신호 성과       → 과거 신호의 N일 후 수익률 계산               │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Group C: 실시간 데이터 워크플로우                 │
│                    (5분 주기)                                    │
├─────────────────────────────────────────────────────────────────┤
│  1. 시장 폭 지표   → 전종목 등락 비율 (KOSPI/KOSDAQ/전체)          │
│  2. 매크로 데이터   → 환율, 금리, 유가 → Redis 캐싱                │
└─────────────────────────────────────────────────────────────────┘
```

### CLI 명령어

```bash
# 빌드
cargo build -p trader-collector --release

# 실행 (환경변수 설정 후)
./target/release/trader-collector <command>
```

#### 데이터 수집 명령어

| 명령어 | 설명 | 예시 |
|--------|------|------|
| `sync-symbols` | 심볼 정보 동기화 | `collector sync-symbols` |
| `collect-ohlcv` | OHLCV 데이터 수집 | `collector collect-ohlcv --stale-hours 24` |
| `sync-naver-fundamentals` | 네이버 금융 데이터 수집 (KR) | `collector sync-naver-fundamentals` |
| `sync-yahoo-fundamentals` | Yahoo 펀더멘털 수집 (US/글로벌) | `collector sync-yahoo-fundamentals --market US` |
| `sync-krx-fundamentals` | KRX API 데이터 수집 | `collector sync-krx-fundamentals` |

#### 분석 지표 명령어

| 명령어 | 설명 | 예시 |
|--------|------|------|
| `sync-indicators` | 기술 지표 동기화 | `collector sync-indicators` |
| `sync-global-scores` | GlobalScore 계산 | `collector sync-global-scores` |
| `refresh-screening` | 스크리닝 뷰 갱신 | `collector refresh-screening` |
| `sync-signal-performance` | 신호 성과 계산 | `collector sync-signal-performance --max-days 20` |

#### 운영 명령어

| 명령어 | 설명 | 예시 |
|--------|------|------|
| `run-all` | 전체 워크플로우 1회 실행 | `collector run-all` |
| `daemon` | 데몬 모드 (주기적 실행) | `collector daemon` |
| `checkpoint list` | 체크포인트 상태 조회 | `collector checkpoint list` |
| `checkpoint clear` | 체크포인트 삭제 | `collector checkpoint clear naver_fundamental` |
| `scheduler-status` | 스케줄러 상태 확인 | `collector scheduler-status --market KR` |

### 모듈 구조 (v0.8.0)

```
trader-collector/src/modules/
├── symbol_sync.rs           # 심볼 정보 동기화 (KRX, Binance, Yahoo)
├── fundamental_sync.rs      # 펀더멘털 데이터 (Naver, Yahoo, KRX)
├── ohlcv_collect.rs         # OHLCV 수집 (KRX API 직접 수집, 누락 범위 감지)
├── indicator_sync.rs        # 기술 지표 계산 (RouteState, MarketRegime)
├── global_score_sync.rs     # GlobalScore 7팩터 종합 점수
├── market_breadth_sync.rs   # 시장 폭 지표 (전종목 등락 비율) [v0.8.0 신규]
├── macro_data_sync.rs       # 매크로 경제 데이터 (환율, 금리, 유가)
├── screening_refresh.rs     # Materialized View 갱신
├── signal_performance_sync.rs # 신호 성과 추적
├── scheduler.rs             # 시장 운영 시간 기반 스케줄링
├── checkpoint.rs            # 수집 중단/재개 체크포인트
├── watchlist_helper.rs      # 관심종목 데이터 우선 수집 (Strategy Watched Tickers 연동) [v0.8.1]
└── utils.rs                 # 공통 유틸리티
```

**v0.8.0 OHLCV 수집 개선:**
- KRX API 직접 수집으로 국내 주식 데이터 정확도 향상
- 기존 데이터와 비교하여 **누락 범위 자동 감지** (`calculate_missing_ranges`)
- 배치 저장으로 DB I/O 최적화 (`save_krx_ohlcv_batch`)
- 시가총액/발행주식수 동시 업데이트 (`update_market_info_batch`)
- 수집 진행률 실시간 표시 (`ProgressTracker`)

### 데몬 모드

```bash
# 백그라운드 실행 (systemd 또는 nohup)
nohup ./target/release/trader-collector daemon > collector.log 2>&1 &

# 또는 systemd 서비스로 등록
sudo systemctl enable trader-collector
sudo systemctl start trader-collector
```

**데몬 모드 동작:**
- **Group A** (외부 API): 60분마다 실행 (`DAEMON_INTERVAL_MINUTES`)
- **Group B** (내부 계산): 15분마다 실행 (`RANKING_INTERVAL_MINUTES`)
- **Group C** (실시간 데이터): 5분마다 실행 (`REALTIME_INTERVAL_MINUTES`)
- 시장 운영 시간 기반 스케줄링 지원 (`SCHEDULING_ENABLED=true`)

### 증분 수집

```bash
# 24시간 이상 지난 심볼만 OHLCV 수집
collector collect-ohlcv --stale-hours 24

# 특정 심볼만 수집
collector collect-ohlcv --symbols "005930,000660,035720"

# 중단점부터 재개 (네이버 펀더멘털)
collector sync-naver-fundamentals --resume
```

### 체크포인트 시스템

대용량 수집 작업의 중단/재개를 지원합니다.

```bash
# 현재 체크포인트 상태 확인
collector checkpoint list

# 출력 예시:
# 📋 체크포인트 상태:
# --------------------------------------------------------------------------------
#   naver_fundamental         | 상태: running      | 처리:  1234개 | 마지막: 005930
#   indicator_sync            | 상태: completed    | 처리:  2500개 | 마지막: 999999
# --------------------------------------------------------------------------------

# 특정 워크플로우 체크포인트 삭제 (처음부터 다시 시작)
collector checkpoint clear naver_fundamental

# 실행 중인 워크플로우를 interrupted로 마킹
collector checkpoint interrupt naver_fundamental
```

### 스케줄링 (시장 운영 시간 기반)

```bash
# 환경변수 설정
SCHEDULING_ENABLED=true
SCHEDULING_KRX_DELAY_MINUTES=60    # 장마감 후 60분 대기
SCHEDULING_SKIP_WEEKENDS=true
SCHEDULING_SKIP_HOLIDAYS=true

# 스케줄러 상태 확인
collector scheduler-status --market KR

# 출력 예시:
# 📅 KR 시장 스케줄러 상태:
#   현재 시간: 2026-02-06 16:30:00 KST
#   장 상태: 마감
#   다음 수집: 2026-02-07 09:00:00 KST
```

---

## 설정

### 환경 변수
```env
DATABASE_URL=postgresql://trader:password@localhost:5432/trader
REDIS_URL=redis://localhost:6379
JWT_SECRET=your-secret-key
ENCRYPTION_MASTER_KEY=your-32-byte-key-base64
```

### 거래소 API 키
웹 대시보드 **설정 > API 키 관리**에서 등록합니다. 모든 키는 AES-256-GCM으로 암호화 저장됩니다.

---

## 설치 가이드

### 시스템 요구사항

| 항목 | 최소 | 권장 |
|------|------|------|
| **OS** | Windows 10, Ubuntu 20.04, macOS 12 | Windows 11, Ubuntu 22.04, macOS 14 |
| **CPU** | 4 코어 | 8 코어 이상 |
| **메모리** | 8GB | 16GB 이상 |
| **저장공간** | 20GB | 50GB+ (시계열 데이터) |
| **Rust** | 1.83+ | 최신 stable |
| **Node.js** | 18+ | 20+ |
| **PostgreSQL** | 15+ (TimescaleDB) | 16+ |

### 1단계: 필수 도구 설치

#### Windows
```powershell
# Rust 설치
winget install Rustlang.Rust.MSVC

# Node.js 설치
winget install OpenJS.NodeJS.LTS

# Podman 설치 (권장)
winget install RedHat.Podman

# Visual Studio Build Tools (C++ 빌드 필수)
winget install Microsoft.VisualStudio.2022.BuildTools
```

#### Linux (Ubuntu/Debian)
```bash
# Rust 설치
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Node.js 설치
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs

# Podman 설치
sudo apt install -y podman podman-compose

# 빌드 도구
sudo apt install -y build-essential pkg-config libssl-dev
```

#### macOS
```bash
# Homebrew로 설치
brew install rust node podman

# Podman 초기화
podman machine init
podman machine start
```

### 2단계: 프로젝트 설정

```bash
# 저장소 클론
git clone https://github.com/berrzebb/zeroquant.git
cd zeroquant

# 환경 설정
cp .env.example .env

# .env 파일 편집 (필수 값 설정)
# DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
# REDIS_URL=redis://localhost:6379
# JWT_SECRET=your-secret-key-min-32-chars
# ENCRYPTION_MASTER_KEY=your-32-byte-encryption-key-base64
```

### 3단계: 인프라 실행

```bash
# Podman 컨테이너 시작 (TimescaleDB + Redis)
podman compose up -d

# 컨테이너 상태 확인
podman ps

# 로그 확인
podman compose logs -f
```

### 4단계: 데이터베이스 마이그레이션

```bash
# 마이그레이션 검증
cargo run -p trader-cli -- migrate verify

# 마이그레이션 적용
cargo run -p trader-cli -- migrate apply --dir migrations_v2
```

### 5단계: 애플리케이션 빌드 및 실행

```bash
# 백엔드 빌드 (Release + ML 기능)
cargo build --bin trader-api --features ml --release

# 백엔드 실행
./target/release/trader-api

# 또는 개발 모드
cargo run --bin trader-api --features ml

# 프론트엔드 설치 및 실행 (새 터미널)
cd frontend
npm install
npm run dev
```

### 6단계: 접속 확인

| 서비스 | URL |
|--------|-----|
| 웹 대시보드 | http://localhost:5173 |
| API 서버 | http://localhost:3000 |
| API 문서 (Swagger) | http://localhost:3000/swagger-ui |
| TimescaleDB | localhost:5432 |
| Redis | localhost:6379 |

### 문제 해결

#### Podman Machine 시작 실패 (Windows)
```powershell
# WSL2 설치 확인
wsl --install

# Podman Machine 재초기화
podman machine stop
podman machine rm
podman machine init --cpus=2 --memory=4096
podman machine start
```

#### 빌드 에러: ONNX Runtime
```bash
# Windows: Visual C++ Redistributable 설치
winget install Microsoft.VCRedist.2015+.x64

# Linux: 라이브러리 설치
sudo apt install -y libonnxruntime-dev
```

#### 데이터베이스 연결 실패
```bash
# 컨테이너 상태 확인
podman ps -a

# TimescaleDB 로그 확인
podman logs trader-timescaledb

# 직접 연결 테스트
podman exec -it trader-timescaledb psql -U trader -d trader
```

---

## 마이그레이션 가이드

ZeroQuant는 데이터 안전성을 보장하는 마이그레이션 관리 도구를 제공합니다.

### 핵심 기능

| 기능 | 설명 |
|------|------|
| **자동 검증** | 중복 정의, CASCADE 사용, 순환 의존성 자동 검출 |
| **멱등성 보장** | IF NOT EXISTS 자동 주입으로 재실행 안전 |
| **통합 계획** | 19개 → 9개 논리적 그룹 통합 |
| **의존성 시각화** | Mermaid/DOT/Text 형식 그래프 생성 |

### CLI 명령어

```bash
# 마이그레이션 검증
trader migrate verify [--verbose] [--dir migrations_v2]

# 통합 계획 생성 (dry-run)
trader migrate consolidate --dry-run

# 통합 마이그레이션 생성
trader migrate consolidate --output migrations_v2

# 의존성 그래프 시각화
trader migrate graph [--format mermaid|dot|text]

# 마이그레이션 적용
trader migrate apply --db-url "postgres://..." --dir migrations_v2

# 적용 상태 확인
trader migrate status --db-url "postgres://..."
```

### 검출 코드

| 코드 | 심각도 | 설명 | 해결 방법 |
|------|--------|------|----------|
| `DUP001` | ⚠️ WARNING | 중복 정의 | 최신 버전만 유지 |
| `CASC001` | ⚠️ WARNING | CASCADE 사용 | 명시적 삭제 순서로 변경 |
| `CIRC001` | 🔴 ERROR | 순환 의존성 | 의존성 구조 재설계 |
| `IDEM001` | ℹ️ INFO | IF NOT EXISTS 누락 | 자동 주입 또는 수동 추가 |
| `DCPAT001` | ⚠️ WARNING | DROP 후 CREATE 패턴 | 데이터 백업 후 진행 |
| `DATA001-003` | ⚠️ WARNING | 데이터 손실 위험 | 사전 백업 필수 |

### 통합 마이그레이션 구조

```
migrations_v2/
├── 01_core_foundation.sql        # Extensions, ENUM, symbols, credentials
├── 02_data_management.sql        # symbol_info, ohlcv, fundamental, 뷰
├── 03_trading_analytics.sql      # trade_executions, position_snapshots
├── 04_strategy_signals.sql       # signal_marker, alert_rule, alert_history
├── 05_evaluation_ranking.sql     # global_score, reality_check, score_history
├── 06_user_settings.sql          # watchlist, preset, notification, checkpoint
├── 07_performance_optimization.sql # 인덱스, Materialized View, Hypertable
├── 08_paper_trading.sql          # Mock 거래소, 전략-계정 연결, Paper Trading 세션 [v0.8.0]
└── 09_strategy_watched_tickers.sql # 전략별 관심 종목, Collector 우선순위 연동 [v0.8.1]
```

### 안전한 마이그레이션 절차

```bash
# 1. 현재 스키마 백업
podman exec -it trader-timescaledb pg_dump -U trader -s trader > schema_backup.sql

# 2. 중요 데이터 백업
podman exec -it trader-timescaledb pg_dump -U trader -t symbols -t ohlcv -t trade_executions trader > data_backup.sql

# 3. 마이그레이션 검증
trader migrate verify --verbose

# 4. 테스트 DB에서 먼저 적용
trader migrate apply --db-url "postgres://test_db..." --dir migrations_v2

# 5. 스키마 비교 후 운영 적용
trader migrate apply --dir migrations_v2
```

### 프로그래매틱 마이그레이션

```rust
use trader_core::migration::{SafeMigrationBuilder, MigrationValidator};

// 안전한 마이그레이션 빌더
let migration = SafeMigrationBuilder::new()
    .with_backup("symbols", "symbols_backup")
    .create_table("new_table", r#"
        CREATE TABLE IF NOT EXISTS new_table (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )
    "#)
    .add_index("new_table", "idx_new_table_name", &["name"])
    .build();

// 검증 후 적용
let validator = MigrationValidator::new(&files);
let report = validator.validate();
if report.has_errors() {
    eprintln!("{}", report);
    return Err("마이그레이션 검증 실패");
}
```

> 📖 상세 가이드: [docs/migration_guide.md](docs/migration_guide.md)

---

## 문서

| 문서 | 설명 |
|------|------|
| [API 문서](docs/api.md) | REST/WebSocket API 레퍼런스 |
| [아키텍처](docs/architecture.md) | 시스템 아키텍처 상세 |
| [마이그레이션 가이드](docs/migration_guide.md) | 데이터베이스 마이그레이션 관리 |
| [설치/배포 가이드](docs/setup_guide.md) | 인프라, 환경설정, 배포 |
| [운영 가이드](docs/operations.md) | 일일 운영, 모니터링, 백업 |
| [데이터 수집](docs/data_collection.md) | Collector, 내부 시스템 |
| [트러블슈팅](docs/troubleshooting.md) | 문제 해결 가이드 |
| [개발 규칙](docs/development_rules.md) | 코드 작성 규칙 및 가이드라인 |
| [전략 가이드](docs/STRATEGY_GUIDE.md) | 전략별 상세 파라미터 및 데이터 연동 |
| [Claude 가이드](CLAUDE.md) | AI 세션 컨텍스트 |

## 라이선스

MIT License
