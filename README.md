<p align="center">
  <h1 align="center">ZeroQuant</h1>
  <p align="center">
    <strong>Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템</strong>
  </p>
  <p align="center">
    <a href="#주요-기능">주요 기능</a> •
    <a href="#지원-전략">지원 전략</a> •
    <a href="#전략-개발-가이드">전략 개발</a> •
    <a href="#빠른-시작">빠른 시작</a> •
    <a href="#문서">문서</a>
  </p>
</p>

---

## 소개

ZeroQuant는 암호화폐와 주식 시장에서 **24/7 자동화된 거래**를 수행하는 트레이딩 시스템입니다.

검증된 27가지 전략과 47개의 ML 패턴 인식을 통해 **그리드 트레이딩**, **자산배분**, **모멘텀** 등 다양한 투자 방법론을 지원합니다. 웹 대시보드에서 실시간 모니터링과 전략 제어가 가능하며, 리스크 관리 시스템이 자동으로 자산을 보호합니다.

## 주요 기능

### 🏦 다중 시장 지원
| 시장 | 거래소 | 기능 |
|------|--------|------|
| 암호화폐 | Binance | 현물 거래, WebSocket 실시간 시세 |
| 한국/미국 주식 | 한국투자증권 (KIS) | 국내/해외 주식, 모의투자 지원 |

### 📊 데이터 & 분석
- **실시간 시세**: WebSocket 기반 실시간 가격/호가/체결
- **과거 데이터**: TimescaleDB 시계열 저장, 백테스팅 지원
- **데이터셋 관리**: Yahoo Finance 데이터 다운로드, 캔들 데이터 CRUD
- **ML 패턴 인식**: 캔들스틱 25개 + 차트 패턴 22개 (ONNX 추론)
- **성과 지표**: Sharpe Ratio, MDD, Win Rate, CAGR 등

### 🛡️ 리스크 관리
- 자동 스톱로스 / 테이크프로핏
- 포지션 크기 및 일일 손실 한도
- ATR 기반 변동성 필터
- Circuit Breaker 패턴

### 🖥️ 웹 대시보드
- 실시간 포트폴리오 모니터링
- 전략 등록/시작/중지/설정
- 데이터셋 관리 (심볼 데이터 다운로드/조회/삭제)
- 백테스트 실행 및 결과 저장/비교
- 동기화된 멀티 차트 패널
- 거래소 API 키 관리 (AES-256-GCM 암호화)

### 📒 매매일지 (Trading Journal)
거래 내역을 체계적으로 관리하고 투자 성과를 분석합니다:
- **체결 내역 동기화**: 거래소 API에서 자동 수집
- **종목별 보유 현황**: 보유 수량, 평균 매입가, 투자 금액
- **물타기 자동 계산**: 추가 매수 시 평균 단가 자동 갱신
- **손익 분석**: 실현/미실현 손익, 기간별 수익률
- **매매 패턴 분석**: 빈도, 성공률, 평균 보유 기간
- **포트폴리오 비중**: 종목별 비중 시각화 및 리밸런싱 추천

## 지원 전략

### 실시간 전략
| 전략 | 설명 |
|------|------|
| **Grid Trading** | 가격 범위 내 자동 매수/매도 그리드 (고정/동적/트렌드 필터) |
| **RSI Mean Reversion** | RSI 과매수/과매도 기반 평균회귀 |
| **Bollinger Bands** | 볼린저 밴드 이탈 시 진입/청산 |
| **Magic Split** | 10차수 분할매수 익절 전략 |
| **Infinity Bot** | 무한매수봇 (50라운드, 트레일링 스탑) |
| **Trailing Stop** | 동적 손절/익절 관리 |

### 일간 전략
| 전략 | 설명 |
|------|------|
| **Volatility Breakout** | 변동성 돌파 (래리 윌리엄스) |
| **SMA Crossover** | 이동평균선 교차 추세 추종 |
| **Snow** | TIP 모멘텀 기반 공격/안전 자산 전환 |
| **Stock Rotation** | 모멘텀 기반 종목 로테이션 |
| **Market Interest Day** | 거래량 급증 종목 단타 |
| **Candle Pattern** | 35개 캔들스틱 패턴 인식 |

### 월간 자산배분 전략
| 전략 | 설명 |
|------|------|
| **All Weather** | 레이 달리오 올웨더 포트폴리오 (US/KR) |
| **HAA** | Hybrid Asset Allocation (카나리아 자산 기반) |
| **XAA** | eXtended Asset Allocation (TOP 4 선택) |
| **Simple Power** | TQQQ/SCHD/PFIX/TMF 조합 + MA 필터 |
| **Market Cap Top** | 시가총액 상위 종목 월간 리밸런싱 |
| **BAA** | Bold Asset Allocation (공격/수비 모드 전환) |
| **Dual Momentum** | 절대/상대 모멘텀 기반 자산배분 |
| **Pension Bot** | 연금 계좌 자동 운용 (MDD 최소화) |

### 섹터/레버리지 전략
| 전략 | 설명 |
|------|------|
| **Sector Momentum** | 섹터 ETF 로테이션 전략 |
| **Sector VB** | 섹터별 변동성 돌파 |
| **US 3X Leverage** | 미국 3배 레버리지 ETF (TQQQ/SOXL) |

### 국내 주식 전략
| 전략 | 설명 |
|------|------|
| **Kosdaq Fire Rain** | 코스닥 단타 변동성 돌파 |
| **KOSPI Bothside** | 코스피 롱숏 양방향 매매 |
| **Small Cap Quant** | 소형주 퀀트 팩터 전략 |
| **Stock Gugan** | 구간별 분할 매매 전략 |

## 전략 개발 가이드

ZeroQuant는 플러그인 기반 전략 시스템을 지원합니다. `Strategy` trait를 구현하여 새로운 전략을 추가할 수 있습니다.

### Strategy Trait

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 전략 이름
    fn name(&self) -> &str;

    /// 전략 버전
    fn version(&self) -> &str;

    /// 전략 설명
    fn description(&self) -> &str;

    /// 설정으로 전략 초기화
    async fn initialize(&mut self, config: Value) -> Result<()>;

    /// 새 시장 데이터 수신 시 호출 → 트레이딩 신호 반환
    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>>;

    /// 주문 체결 시 호출
    async fn on_order_filled(&mut self, order: &Order) -> Result<()>;

    /// 포지션 업데이트 시 호출
    async fn on_position_update(&mut self, position: &Position) -> Result<()>;

    /// 전략 종료 및 리소스 정리
    async fn shutdown(&mut self) -> Result<()>;

    /// 현재 전략 상태 (모니터링용)
    fn get_state(&self) -> Value;
}
```

### 새 전략 추가 방법

1. **전략 파일 생성**: `crates/trader-strategy/src/strategies/my_strategy.rs`

```rust
use crate::Strategy;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MyStrategyConfig {
    pub symbol: String,
    pub parameter1: f64,
    // ... 전략 파라미터
}

pub struct MyStrategy {
    config: MyStrategyConfig,
    // ... 내부 상태
}

#[async_trait]
impl Strategy for MyStrategy {
    fn name(&self) -> &str { "my_strategy" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "나만의 전략 설명" }

    async fn on_market_data(&mut self, data: &MarketData) -> Result<Vec<Signal>> {
        // 시장 데이터 분석 후 매수/매도 신호 생성
        let signals = vec![];

        if /* 매수 조건 */ {
            signals.push(Signal::new(SignalType::Buy, ...));
        }

        Ok(signals)
    }
    // ... 나머지 구현
}
```

2. **모듈 등록**: `crates/trader-strategy/src/strategies/mod.rs`

```rust
pub mod my_strategy;
pub use my_strategy::*;
```

3. **엔진에 등록**: `crates/trader-strategy/src/engine.rs`의 전략 팩토리에 추가

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
│       ├── common/         # 공통 유틸리티 (모멘텀, 리밸런스)
│       ├── grid.rs         # 그리드 트레이딩
│       ├── rsi.rs          # RSI 평균회귀
│       └── ...             # 기타 전략들
```

## 아키텍처

```
zeroquant/
├── crates/
│   ├── trader-core/         # 도메인 모델, 공통 유틸리티
│   ├── trader-exchange/     # 거래소 연동 (Binance, KIS)
│   ├── trader-strategy/     # 전략 엔진, 27개 전략
│   ├── trader-risk/         # 리스크 관리
│   ├── trader-execution/    # 주문 실행 엔진
│   ├── trader-data/         # 데이터 수집/저장 (OHLCV)
│   ├── trader-analytics/    # ML 추론, 성과 분석
│   ├── trader-api/          # REST/WebSocket API
│   │   └── repository/      # 데이터 접근 계층 (Repository 패턴)
│   ├── trader-cli/          # CLI 도구
│   └── trader-notification/ # 알림 (Telegram)
├── frontend/                # SolidJS + TypeScript + Vite
│   └── src/pages/
│       ├── Dashboard.tsx    # 포트폴리오 모니터링
│       ├── Strategies.tsx   # 전략 등록/관리
│       ├── Dataset.tsx      # 데이터셋 관리
│       ├── Backtest.tsx     # 백테스트 실행
│       └── Simulation.tsx   # 시뮬레이션
├── migrations/              # DB 마이그레이션 (13개)
└── monitoring/              # Prometheus/Grafana
```

## 기술 스택

| 영역 | 기술 |
|------|------|
| Backend | Rust, Tokio, Axum |
| Database | PostgreSQL (TimescaleDB), Redis |
| Frontend | SolidJS, TypeScript, Vite |
| ML | ONNX Runtime, XGBoost, LightGBM |
| Infrastructure | Docker, Prometheus, Grafana |

## 빠른 시작

### 요구사항
- Rust 1.83+ (ONNX Runtime 호환)
- Node.js 18+
- Docker & Docker Compose
- PostgreSQL 15+ (TimescaleDB) / Redis 7+

### 설치

```bash
# 저장소 클론
git clone https://github.com/berrzebb/zeroquant.git
cd zeroquant

# 환경 설정
cp .env.example .env
# .env 파일에서 DB 연결 정보 설정

# Docker로 실행 (권장)
docker-compose up -d
```

### 개발 모드

```bash
# 백엔드
cargo build --release
cargo run --bin trader-cli

# 프론트엔드
cd frontend
npm install
npm run dev
```

## 설정

### 환경 변수
```env
DATABASE_URL=postgresql://trader:password@localhost:5432/trader
REDIS_URL=redis://localhost:6379
JWT_SECRET=your-secret-key
ENCRYPTION_KEY=your-32-byte-key-base64
```

### 거래소 API 키
웹 대시보드 **설정 > API 키 관리**에서 등록합니다. 모든 키는 AES-256-GCM으로 암호화 저장됩니다.

## 문서

| 문서 | 설명 |
|------|------|
| [아키텍처](docs/architecture.md) | 시스템 아키텍처 및 기술 스택 |
| [코드 최적화 분석](docs/code_optimization_analysis.md) | 코드 최적화 기회 및 성능 개선 제안 🔥 |
| [개선 제안서](docs/improvement_suggestions.md) | 프로젝트 개선 제안 및 로드맵 (상세, 한국어) ⭐ |
| [Improvement Suggestions](docs/IMPROVEMENTS_EN.md) | Project improvement suggestions (Summary, English) ⭐ |
| [API 문서](docs/api.md) | REST/WebSocket API 레퍼런스 |
| [배포 가이드](docs/deployment.md) | 프로덕션 배포 방법 |
| [모니터링](docs/monitoring.md) | Prometheus/Grafana 설정 |
| [운영 가이드](docs/operations.md) | 일상 운영 및 관리 |
| [트러블슈팅](docs/troubleshooting.md) | 문제 해결 가이드 |
| [전략 비교](docs/STRATEGY_COMPARISON.md) | 전략별 상세 파라미터 |
| [TODO](docs/todo.md) | 작업 현황 및 계획 |

## 라이선스

MIT License
