# ZeroQuant Trading Bot - 기술 아키텍처

> 작성일: 2026-02-07
> 버전: 3.5 (v0.8.0 반영)
>
> 주요 변경:
> - StrategyContext 통합 아키텍처 (ExchangeProvider + AnalyticsProvider)
> - 전략 실행 모드 (실거래/페이퍼트레이딩/백테스트) 분리
> - 포지션 ID/그룹 ID 시스템 (Grid/Spread 전략)
> - KIS WebSocket 실시간 연동, Paper Trading

---

## 시스템 구성도

```
┌─────────────────────────────────────────────────────────────┐
│                  Web Dashboard (Frontend)                    │
│                 SolidJS + TailwindCSS                        │
└─────────────────────┬───────────────────────────────────────┘
                      │ WebSocket + REST API
┌─────────────────────▼───────────────────────────────────────┐
│                   API Gateway (Axum)                         │
│          Authentication & Authorization Layer                │
└─────────┬────────────────────────────────────┬──────────────┘
          │                                    │
┌─────────▼────────────┐          ┌───────────▼──────────────┐
│  Strategy Engine     │          │    Risk Manager          │
│  (Plugin System)     │◄─────────┤  (Real-time Monitor)     │
└─────────┬────────────┘          └───────────┬──────────────┘
          │                                    │
┌─────────▼─────────▼────────────────────────────────────────┐
│                 Order Executor                              │
│       (Position Management, Order Routing)                  │
└─────────┬───────────────────────────────────┬───────────────┘
          │                                   │
┌─────────▼──────────┐          ┌────────────▼──────────────┐
│ Exchange Connector │          │     Data Manager          │
│  (Multi-Exchange)  │          │ (Real-time + Historical)  │
└─────────┬──────────┘          └────────────┬──────────────┘
          │                                   │
          └───────────────┬───────────────────┘
                          │
          ┌───────────────▼───────────────────────────┐
          │      Database Layer                       │
          │ PostgreSQL (Timescale) + Redis            │
          └───────────────────────────────────────────┘
```

---

## 기술 스택

### 백엔드
| 기술 | 버전 | 용도 |
|------|------|------|
| Rust | stable (1.93+) | 시스템 프로그래밍 언어 |
| Tokio | 최신 | 비동기 런타임 |
| Axum | 0.7+ | 웹 프레임워크 |
| SQLx | 0.8+ | 데이터베이스 드라이버 (async, compile-time checked) |
| TimescaleDB | 2.x | 시계열 데이터베이스 (PostgreSQL 15 확장) |
| Redis | 7.x | 캐시, 세션, 실시간 데이터 |

### 프론트엔드
| 기술 | 버전 | 용도 |
|------|------|------|
| SolidJS | 1.8+ | 반응형 UI 프레임워크 |
| TailwindCSS | 3.x | 유틸리티 CSS |
| Lightweight Charts | 4.x | 금융 차트 라이브러리 |
| TanStack Query | 5.x | 서버 상태 관리 |
| Vite | 5.x | 빌드 도구 |

### 데이터 및 분석
| 기술 | 용도 |
|------|------|
| Polars | 고성능 데이터프레임 처리 |
| ta-rs | 기술적 지표 라이브러리 |
| ONNX Runtime | ML 모델 추론 (GPU 가속) |
| KRX OPEN API | 국내 주식 OHLCV/Fundamental 데이터 |
| Yahoo Finance | 해외 주식/암호화폐 OHLCV 데이터 |
| ts-rs | TypeScript 바인딩 자동 생성 |

### 인프라
| 기술 | 용도 |
|------|------|
| Podman / Docker | 컨테이너화 |
| Docker Compose | 멀티 컨테이너 오케스트레이션 |
| tracing | 구조화된 로깅 |

---

## 프로젝트 구조

```
d:\Trader\
├── Cargo.toml                 # Workspace 루트
├── .env.example               # 환경 변수 템플릿
├── docker-compose.yml         # Docker 서비스 정의
│
├── crates/                    # Rust 크레이트 (백엔드)
│   ├── trader-core/           # 도메인 모델 [v0.8.0 확장]
│   │   ├── domain/
│   │   │   ├── account.rs         # 계좌 정보 (AccountInfo, AccountBalance) [v0.8.0]
│   │   │   ├── exchange_types.rs  # 거래소 제약조건 (ExchangeConstraints) [v0.8.0]
│   │   │   ├── context.rs         # StrategyContext (계좌/제약 통합) [v0.8.0 확장]
│   │   │   ├── exchange_provider.rs # ExchangeProvider trait
│   │   │   ├── signal.rs          # 전략 신호
│   │   │   ├── order.rs           # 주문 타입 정의
│   │   │   ├── position.rs        # 포지션 타입 정의
│   │   │   └── symbol.rs          # 심볼 정의
│   │   └── migration/         # 마이그레이션 관리 [v0.7.2]
│   │       ├── analyzer.rs        # SQL 파싱, 의존성 그래프
│   │       ├── validator.rs       # 중복/CASCADE/순환 검출
│   │       ├── consolidator.rs    # 통합 계획 생성
│   │       └── models.rs          # 데이터 모델
│   │
│   ├── trader-api/            # REST API 서버 [v0.8.0 대폭 확장]
│   │   ├── routes/            # 18개 API 라우트
│   │   │   ├── backtest.rs    # 백테스트 실행
│   │   │   ├── strategies.rs  # 전략 CRUD + 스트림 자동 연결 [v0.8.0]
│   │   │   ├── paper_trading.rs # Paper Trading API [v0.8.0 신규]
│   │   │   ├── portfolio.rs   # 포트폴리오 조회
│   │   │   └── ...
│   │   ├── services/          # 서비스 계층 [v0.8.0 신규]
│   │   │   ├── market_stream.rs    # MarketStreamHandle (Singleton) [v0.8.0]
│   │   │   └── signal_processor.rs # Signal 처리 서비스 [v0.8.0]
│   │   ├── websocket/         # WebSocket 모듈 [v0.8.0 확장]
│   │   │   ├── handler.rs     # 세션 관리 + 거래소 스트림 브릿지 [v0.8.0]
│   │   │   ├── aggregator.rs  # MarketDataAggregator [v0.8.0]
│   │   │   ├── subscriptions.rs # 구독 관리
│   │   │   └── messages.rs    # WebSocket 메시지 타입
│   │   ├── openapi.rs         # OpenAPI 3.0 스펙 중앙 집계
│   │   ├── state.rs           # 앱 상태 관리 (market_streams 추가) [v0.8.0]
│   │   └── main.rs            # 서버 엔트리포인트 (Swagger UI: /swagger-ui)
│   │
│   ├── trader-strategy/       # 전략 엔진
│   │   ├── strategies/        # 16개 통합 전략 (v0.7.0 리팩토링)
│   │   │   ├── common/            # 공통 모듈 (v0.7.0 대폭 확장)
│   │   │   │   ├── exit_config.rs      # 청산 설정 프리셋 [v0.7.0]
│   │   │   │   ├── global_score_utils.rs # GlobalScore 유틸리티 [v0.7.0]
│   │   │   │   ├── indicators.rs       # 기술 지표 (RSI, SMA, BB 등)
│   │   │   │   ├── position_sizing.rs  # 포지션 사이징
│   │   │   │   ├── risk_checks.rs      # 리스크 검증
│   │   │   │   └── signal_filters.rs   # 신호 필터링
│   │   │   ├── day_trading.rs     # 단타/그리드 (Grid, Market Interest Day 통합)
│   │   │   ├── mean_reversion.rs  # 평균회귀 (RSI, Bollinger 통합)
│   │   │   ├── rotation.rs        # 모멘텀 로테이션 (4개 전략 통합)
│   │   │   ├── asset_allocation.rs # 자산배분 (HAA/XAA/BAA/All Weather 통합)
│   │   │   └── ...
│   │   ├── engine.rs          # 전략 실행 엔진
│   │   └── registry.rs        # 전략 레지스트리
│   │
│   ├── trader-risk/           # 리스크 관리
│   │   ├── manager.rs         # 중앙 RiskManager
│   │   ├── position_sizing.rs # 포지션 사이징
│   │   ├── stop_loss.rs       # 스톱로스/테이크프로핏
│   │   ├── limits.rs          # 일일 손실 한도
│   │   ├── trailing_stop.rs   # 트레일링 스탑 (4가지 모드)
│   │   └── config.rs          # 리스크 설정
│   │
│   ├── trader-execution/      # 주문 실행 [v0.7.2 확장]
│   │   ├── signal_processor.rs    # SignalProcessor trait (공통 인터페이스)
│   │   ├── simulated_executor.rs  # SimulatedExecutor (백테스트/시뮬레이션)
│   │   ├── executor.rs            # 주문 실행기 (실거래)
│   │   ├── order_manager.rs       # 주문 관리
│   │   └── position_tracker.rs    # 포지션 추적
│   │
│   ├── trader-exchange/       # 거래소 연동 [v0.8.0 통합]
│   │   ├── connector/
│   │   │   ├── binance.rs     # Binance 커넥터
│   │   │   └── kis/           # KIS 커넥터 [v0.8.0 확장]
│   │   │       ├── client.rs      # 공통 HTTP 클라이언트 [v0.8.0 신규]
│   │   │       ├── client_kr.rs   # KR 주문/조회
│   │   │       ├── client_us.rs   # US 주문/조회
│   │   │       ├── websocket_kr.rs # KR WebSocket (동적 구독) [v0.8.0]
│   │   │       └── websocket_us.rs # US WebSocket (동적 구독) [v0.8.0]
│   │   ├── provider/          # ExchangeProvider 구현
│   │   │   ├── kis.rs         # KIS 통합 프로바이더 [v0.8.0 통합]
│   │   │   ├── binance.rs     # Binance 프로바이더
│   │   │   └── mock.rs        # Mock 프로바이더 [v0.8.0 신규]
│   │   ├── stream.rs          # UnifiedMarketStream (Bridge Task) [v0.8.0]
│   │   ├── simulated/         # 시뮬레이션 모드
│   │   └── traits.rs          # MarketStream trait
│   │
│   ├── trader-collector/      # Standalone 데이터 수집기 [v0.8.0 확장]
│   │   ├── main.rs            # CLI 엔트리포인트 (데몬 모드 지원)
│   │   ├── config.rs          # 환경변수 설정
│   │   └── modules/           # 수집 모듈
│   │       ├── ohlcv_collect.rs         # OHLCV 수집 [v0.8.0 개선]
│   │       ├── indicator_sync.rs        # 지표 동기화
│   │       ├── global_score_sync.rs     # GlobalScore 동기화
│   │       ├── market_breadth_sync.rs   # Market Breadth 동기화 [v0.8.0 신규]
│   │       ├── fundamental_sync.rs      # Fundamental 동기화
│   │       ├── scheduler.rs             # 시장 시간 스케줄러
│   │       ├── signal_performance_sync.rs # 신호 성과 추적
│   │       └── checkpoint.rs            # 체크포인트 관리
│   │
│   ├── trader-data/           # 데이터 관리
│   │   ├── storage/           # TimescaleDB 저장소
│   │   ├── cache/             # Redis 캐시
│   │   └── provider/          # 데이터 프로바이더
│   │       ├── krx_api.rs          # KRX OPEN API (국내 OHLCV/Fundamental)
│   │       ├── naver.rs            # 네이버 금융 크롤러 (국내 Fundamental)
│   │       ├── yahoo_fundamental.rs # Yahoo Finance 펀더멘털 (해외) [v0.7.0]
│   │       └── symbol_info.rs      # Yahoo Finance 심볼 정보
│   │
│   ├── trader-analytics/      # 분석 엔진
│   │   ├── backtest/          # 백테스트 엔진
│   │   │   └── engine.rs      # Multi-TF 백테스트 지원
│   │   ├── metrics.rs         # 성과 지표 14개
│   │   ├── indicators.rs      # 기술 지표 11개
│   │   ├── seven_factor.rs    # 7Factor 스코어링 시스템
│   │   ├── multi_timeframe_helpers.rs # 다중 TF 헬퍼
│   │   ├── timeframe_alignment.rs     # TF 정렬 (Bias 방지)
│   │   └── ml/                # ML 패턴 인식
│   │       ├── pattern.rs     # 캔들/차트 패턴 48종
│   │       ├── predictor.rs   # ONNX 추론
│   │       └── features.rs    # Feature Engineering
│   │
│   ├── trader-cli/            # CLI 도구
│   │   ├── commands/
│   │   │   ├── download.rs        # 데이터 다운로드
│   │   │   ├── backtest.rs        # CLI 백테스트
│   │   │   ├── import.rs          # 데이터 임포트
│   │   │   └── strategy_test.rs   # 전략 통합 테스트 [v0.7.0]
│   │   └── main.rs
│   │
│   └── trader-notification/   # 알림 서비스 [v0.7.2 확장]
│       ├── telegram.rs        # 텔레그램 봇
│       ├── discord.rs         # Discord 웹훅
│       ├── slack.rs           # Slack 웹훅 [v0.7.2]
│       ├── email.rs           # SMTP 이메일 [v0.7.2]
│       ├── sms.rs             # Twilio SMS [v0.7.2]
│       └── types.rs           # 공통 알림 타입
│
├── migrations/                # DB 마이그레이션 (원본 18개)
│   └── ...                    # 개별 마이그레이션 파일
│
├── migrations_v2/             # 통합 마이그레이션 (7개) [v0.7.2]
│   ├── 01_core_foundation.sql      # Extensions, ENUM, symbols, credentials
│   ├── 02_data_management.sql      # symbol_info, ohlcv, fundamental
│   ├── 03_trading_analytics.sql    # trade_executions, 분석 뷰
│   ├── 04_strategy_signals.sql     # signal_marker, alert_rule
│   ├── 05_evaluation_ranking.sql   # global_score, reality_check
│   ├── 06_user_settings.sql        # watchlist, preset, notification
│   └── 07_performance_optimization.sql # 인덱스, MV, Hypertable
│
├── frontend/                  # 웹 대시보드
│   ├── src/
│   │   ├── pages/             # 11개 페이지 (Lazy Loading)
│   │   │   ├── Dashboard.tsx
│   │   │   ├── Backtest.tsx
│   │   │   ├── Strategies.tsx
│   │   │   ├── GlobalRanking.tsx  # 글로벌 랭킹
│   │   │   ├── SymbolDetail.tsx   # 종목 상세
│   │   │   └── ...
│   │   ├── components/        # UI 컴포넌트
│   │   │   ├── charts/        # 차트 컴포넌트 20개+
│   │   │   │   ├── MultiTimeframeChart.tsx
│   │   │   │   ├── VolumeProfile.tsx
│   │   │   │   └── ...
│   │   │   ├── strategy/      # 전략 컴포넌트
│   │   │   │   └── MultiTimeframeSelector.tsx
│   │   │   ├── screening/     # 스크리닝 컴포넌트
│   │   │   └── ui/            # 공통 UI (VirtualizedTable 등)
│   │   ├── api/               # API 클라이언트
│   │   ├── hooks/             # 커스텀 훅 (useStrategies, useJournal 등)
│   │   └── types/generated/   # ts-rs 자동 생성 타입
│   ├── package.json
│   └── vite.config.ts         # manualChunks 코드 스플리팅
│
├── config/                    # 설정 파일
├── tests/                     # 통합 테스트
└── docs/                      # 문서
    ├── architecture.md        # (이 문서)
    ├── api.md                 # API 문서
    ├── STRATEGY_GUIDE.md      # 전략 가이드
    └── todo.md                # TODO 목록
```

---

## 크레이트 의존성 그래프

```
                    ┌────────────────┐
                    │  trader-api    │
                    │  (Entry Point) │
                    └───────┬────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌───────────────┐  ┌────────────────┐  ┌───────────────┐
│trader-strategy│  │ trader-risk    │  │trader-exchange│
│   (Signals)   │  │ (Validation)   │  │   (Market)    │
└───────┬───────┘  └───────┬────────┘  └───────┬───────┘
        │                  │                   │
        └──────────────────┼───────────────────┘
                           │
                    ┌──────▼──────┐
                    │trader-exec  │
                    │(Order Flow) │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
      ┌────────────┐ ┌──────────┐ ┌────────────┐
      │trader-data │ │trader-   │ │trader-     │
      │ (Storage)  │ │analytics │ │notification│
      └──────┬─────┘ └────┬─────┘ └────────────┘
             │            │
             ▼            ▼
      ┌────────────────────────┐
      │     trader-core        │
      │   (Domain Models)      │
      └────────────────────────┘
```

---

## 데이터 흐름

### 1. 백테스트 플로우

```
Frontend (Backtest.tsx)
    │
    │ POST /api/v1/backtest/run
    ▼
API Layer (backtest.rs)
    │
    │ 1. 파라미터 검증
    │ 2. 히스토리컬 데이터 로드
    ▼
Data Layer (trader-data)
    │
    │ TimescaleDB에서 OHLCV 조회
    ▼
Strategy Engine (trader-strategy)
    │
    │ 전략 실행, 신호 생성
    ▼
Backtest Engine (trader-analytics)
    │
    │ 1. 주문 시뮬레이션
    │ 2. 슬리피지/수수료 적용
    │ 3. 포지션 관리
    │ 4. 성과 지표 계산
    ▼
API Layer
    │
    │ BacktestResult 반환
    ▼
Frontend
    │
    │ 차트 및 통계 렌더링
    ▼
```

### 2. 실시간 트레이딩 플로우

```
Exchange WebSocket
    │
    │ 실시간 시세 수신
    ▼
Data Layer (캐시)
    │
    │ Redis에 틱 데이터 저장
    ▼
Strategy Engine
    │
    │ 전략 평가, 신호 생성
    ▼
Risk Manager                    ◄── 검증 실패 시 거부
    │
    │ 1. 포지션 크기 검증
    │ 2. 일일 손실 한도 확인
    │ 3. 변동성 필터 적용
    ▼
Order Executor
    │
    │ 1. 주문 생성
    │ 2. 스톱로스/테이크프로핏 자동 생성
    ▼
Exchange Connector
    │
    │ 거래소 API 호출
    ▼
Notification Service
    │
    │ 텔레그램/Discord 알림
    ▼
```

---

## StrategyContext 통합 아키텍처 (v0.8.0)

전략들이 공유하는 통합 컨텍스트로, 거래소 정보와 분석 결과를 중앙에서 관리합니다.

### 데이터 소스 및 흐름

```
┌──────────────────────────────────────────────────────────────────────┐
│                         데이터 소스                                   │
│  ┌────────────────┐              ┌────────────────────────────────┐  │
│  │  거래소 API    │              │      분석 엔진                  │  │
│  │  (Binance,KIS) │              │  (GlobalScorer, RouteState)    │  │
│  └───────┬────────┘              └───────────────┬────────────────┘  │
│          │                                       │                   │
│          ▼                                       ▼                   │
│  ┌────────────────┐              ┌────────────────────────────────┐  │
│  │ExchangeProvider│              │     AnalyticsProvider          │  │
│  │ (1~5초 갱신)   │              │ (1~10분 갱신)                   │  │
│  │ - 계좌 정보    │              │ - GlobalScore                  │  │
│  │ - 포지션       │              │ - RouteState                   │  │
│  │ - 미체결 주문  │              │ - Screening 결과               │  │
│  │ - 거래소 제약  │              │ - StructuralFeatures           │  │
│  └───────┬────────┘              └───────────────┬────────────────┘  │
│          │                                       │                   │
│          └───────────────┬───────────────────────┘                   │
│                          ▼                                           │
└──────────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      StrategyContext                                  │
│        (전략 간 공유되는 통합 컨텍스트 - Arc<RwLock<>>)               │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌─────────────────────────┐      ┌─────────────────────────────┐   │
│  │  거래소 정보 (1~5초)     │      │  분석 결과 (1~10분)          │   │
│  │  - AccountInfo          │      │  - global_scores            │   │
│  │  - positions            │      │  - route_states             │   │
│  │  - pending_orders       │      │  - screening_results        │   │
│  │  - exchange_constraints │      │  - structural_features      │   │
│  └────────────┬────────────┘      └──────────────┬──────────────┘   │
│               │                                   │                  │
│               └─────────────┬─────────────────────┘                  │
│                             ▼                                        │
│              ┌──────────────────────────────┐                        │
│              │       충돌 방지 + 의사결정    │                        │
│              │  - 중복 주문 차단             │                        │
│              │  - 잔고/포지션 한도 체크      │                        │
│              │  - GlobalScore 기반 종목 선택 │                        │
│              │  - RouteState 기반 진입/청산  │                        │
│              └──────────────────────────────┘                        │
│                             │                                        │
│         ┌───────────────────┼───────────────────┐                    │
│         ▼                   ▼                   ▼                    │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐            │
│  │ 전략 A      │     │ 전략 B      │     │ 전략 C      │            │
│  │ (RSI)       │     │ (Grid)      │     │ (Momentum)  │            │
│  └─────────────┘     └─────────────┘     └─────────────┘            │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### 컨텍스트 데이터 분류

| 데이터 | 용도 | 갱신 주기 |
|--------|------|----------|
| **AccountInfo** | 계좌 잔고, 가용 자금 | 1~5초 |
| **positions** | 현재 보유 포지션 | 1~5초 |
| **pending_orders** | 미체결 주문 | 1~5초 |
| **exchange_constraints** | 최소 주문, 호가 단위 등 | 초기화 시 |
| **GlobalScore** | 종목별 종합 점수 (0~100) | 1~10분 |
| **RouteState** | 종목별 현재 상태 (Attack/Wait/Overheat) | 1~10분 |
| **ScreeningResult** | 스크리닝 결과 캐시 | 1~10분 |
| **StructuralFeatures** | RSI, MACD 등 기술 지표 | 1~10분 |

---

## 전략 실행 모드 아키텍처 (v0.8.0)

데이터 발행과 Signal 처리를 분리하여 동일한 전략 로직으로 다양한 실행 모드를 지원합니다.

### 전체 실행 흐름

```
┌──────────────────────────────────────────────────────────────────────────┐
│                      데이터 발행 (DataProvider)                           │
│  ┌────────────────────────────┐    ┌────────────────────────────────┐   │
│  │  ExchangeProvider (실환경) │    │  BacktestEngine (과거 데이터)   │   │
│  │  • KIS (KR/US)             │    │  • TimescaleDB OHLCV           │   │
│  │  • Binance                 │    │  • CSV 파일                    │   │
│  │  • Mock (페이퍼)           │    │                                │   │
│  └────────────┬───────────────┘    └───────────────┬────────────────┘   │
│               └────────────────┬───────────────────┘                    │
│                                ▼                                         │
└──────────────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
              ┌──────────────────────────────────┐
              │              전략                 │
              │  (StrategyContext 활용)          │
              │  → Signal 발행                   │
              └────────────────┬─────────────────┘
                               │ Signal
                               ▼
╔══════════════════════════════════════════════════════════════════════════╗
║                     SignalProcessor (trait)                               ║
╠══════════════════════════════════════════════════════════════════════════╣
║  async fn process_signal(&self, signal, price, timestamp) → TradeResult  ║
║  fn balance(&self) -> Decimal                                            ║
║  fn positions(&self) -> &HashMap<String, ProcessorPosition>              ║
║  fn trades(&self) -> &[TradeResult]                                      ║
║  fn unrealized_pnl(&self, prices) -> Decimal                             ║
║  fn positions_by_group(&self, group_id) -> Vec<&ProcessorPosition>       ║
╚═══════════════════════┬════════════════════════┬═════════════════════════╝
                        │                        │
          ┌─────────────▼─────────────┐ ┌────────▼────────────────┐
          │    SimulatedExecutor      │ │      LiveExecutor       │
          │   (백테스트/시뮬레이션)    │ │      (실거래)           │
          │                           │ │                         │
          │ • 가상 체결               │ │ • OrderExecutionProvider│
          │ • 슬리피지 시뮬레이션     │ │ • 브라켓 주문 (SL/TP)   │
          │ • 수수료 계산             │ │ • 체결 대기/콜백        │
          └───────────────────────────┘ └─────────────────────────┘
```

### 실행 모드 조합

| 데이터 발행 | Signal 처리 | 결과 | 용도 |
|------------|------------|------|------|
| ExchangeProvider (KIS/Binance) | LiveExecutor | **실거래** | 실제 매매 |
| ExchangeProvider (Mock) | SimulatedExecutor | **페이퍼 트레이딩** | 전략 검증 |
| BacktestEngine | SimulatedExecutor | **백테스트** | 과거 성과 분석 |

### SignalProcessor 구현체

#### ProcessorConfig (공통 설정)

```rust
pub struct ProcessorConfig {
    pub commission_rate: Decimal,      // 수수료율 (0.001 = 0.1%)
    pub slippage_rate: Decimal,        // 슬리피지율 (0.0005 = 0.05%)
    pub max_position_size_pct: Decimal, // 최대 포지션 비율 (0.2 = 20%)
    pub max_positions: usize,          // 최대 포지션 수 (10)
    pub allow_short: bool,             // 숏 허용 여부
}
```

#### LiveExecutor 구조 (v0.8.0)

```rust
pub struct LiveExecutor {
    config: ProcessorConfig,
    order_provider: Arc<dyn OrderExecutionProvider>,  // 거래소 추상화
    bracket_manager: BracketOrderManager,             // SL/TP OCO 관리
    positions: HashMap<String, ProcessorPosition>,    // position_id 기반
    trades: Vec<TradeResult>,
}
```

#### ProcessorPosition (스프레드 전략 지원)

```rust
pub struct ProcessorPosition {
    pub symbol: String,
    pub side: Side,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub position_id: Option<String>,   // Grid 레벨별 ID
    pub group_id: Option<String>,      // 그룹 청산용
}
```

### 거래소 추상화 계층

```
┌─────────────────────────────────────────────────────────────────┐
│                    OrderExecutionProvider (trait)                │
├─────────────────────────────────────────────────────────────────┤
│  async fn place_order(&self, request) -> OrderResult            │
│  async fn cancel_order(&self, order_id) -> Result              │
│  async fn modify_order(&self, order_id, request) -> Result     │
│  async fn get_order_status(&self, order_id) -> OrderStatus     │
└──────────────┬───────────────┬───────────────┬──────────────────┘
               │               │               │
    ┌──────────▼──────┐ ┌──────▼──────┐ ┌──────▼──────┐
    │ KisOrderClient  │ │BinanceClient│ │    Mock     │
    │   (KR/US)       │ │   (Spot)    │ │  (테스트)   │
    └─────────────────┘ └─────────────┘ └─────────────┘
```

### 핵심 원칙

1. **전략은 실행 환경을 모름** - 데이터를 받고 Signal만 발행
2. **SignalProcessor trait** - 모든 실행기가 동일한 인터페이스 구현
3. **의존성 주입** - 런타임에 DataProvider와 SignalProcessor 교체
4. **거래소 중립** - OrderExecutionProvider로 거래소별 차이 추상화

---

## 포지션 ID / 그룹 ID 시스템 (v0.8.0)

스프레드 기반 전략(Grid, MagicSplit, DCA)에서 레벨별 독립 포지션 관리와 그룹 단위 청산을 지원하는 2계층 식별 체계입니다.

### 2계층 식별 체계

```rust
Signal {
    ticker: "005930",              // 실제 거래 심볼
    position_id: "005930_grid_L1", // 개별 포지션 식별
    group_id: "grid_55000_1707...", // 관련 포지션 그룹
}
```

### 전략별 ID 형식

| 전략 | position_id 형식 | group_id 형식 |
|------|-----------------|---------------|
| Grid | `{ticker}_grid_L{level}` | `grid_{base_price}_{timestamp}` |
| MagicSplit | `{ticker}_split_L{level}` | `split_{ticker}_{timestamp}` |
| InfinityBot | `{ticker}_inf_R{round}` | `inf_{ticker}_{timestamp}` |

### 사용 패턴

```rust
// 1. 레벨별 독립 포지션 생성
Signal::entry("strategy", ticker, Side::Buy)
    .with_position_id(format!("{}_grid_L{}", ticker, level))
    .with_group_id(session_group_id)

// 2. 특정 레벨만 청산
Signal::exit("strategy", ticker, Side::Sell)
    .with_position_id(format!("{}_grid_L{}", ticker, level))

// 3. 그룹 전체 청산
let keys = executor.position_keys_by_group("grid_session_1");
for key in keys { /* 각각 청산 */ }
```

---

## WebSocket 실시간 시세 아키텍처 (v0.8.0)

KIS 거래소 WebSocket을 통한 실시간 시세 데이터 흐름입니다.

### 구조

```
┌─────────────────────────────────────────────────────────────────┐
│                  Frontend (WebSocket Client)                      │
│              market:{symbol} 채널 구독                            │
└─────────────────────┬───────────────────────────────────────────┘
                      │ Subscribe
                      ▼
╔═════════════════════════════════════════════════════════════════╗
║                    WsState (WebSocket Handler)                   ║
║  forward_subscribe_to_exchange_streams()                         ║
║  → market:{symbol} 채널에서 심볼 추출                             ║
║  → 모든 활성 MarketStreamHandle에 구독 전달                       ║
╚═══════════════════┬═════════════════════════════════════════════╝
                    │
                    ▼
╔═════════════════════════════════════════════════════════════════╗
║          MarketStreamHandle (Singleton per credential)           ║
║  services/market_stream.rs                                       ║
║                                                                  ║
║  • get_or_create_market_stream() - Lazy 초기화                   ║
║  • subscribe(symbol) - 참조 카운트 기반 구독                      ║
║  • unsubscribe(symbol) - 참조 카운트 0이면 실제 해제              ║
╚═══════════════════┬═════════════════════════════════════════════╝
                    │ StreamCommand (mpsc)
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│              UnifiedMarketStream (Bridge Task)                    │
│  stream.rs                                                       │
│                                                                  │
│  ┌──────────────┐  event_tx  ┌───────────────┐                  │
│  │ KR Bridge    │──────────►│               │                  │
│  │ (tokio::spawn)│           │  mpsc Channel │──► next_event() │
│  └──────────────┘           │               │                  │
│  ┌──────────────┐  event_tx  │               │                  │
│  │ US Bridge    │──────────►│               │                  │
│  │ (tokio::spawn)│           └───────────────┘                  │
│  └──────────────┘                                               │
└─────────────────────┬───────────────────────────────────────────┘
                      │ MarketEvent
                      ▼
╔═════════════════════════════════════════════════════════════════╗
║              MarketDataAggregator (Bridge Task)                  ║
║  websocket/aggregator.rs                                         ║
║                                                                  ║
║  handle_event(MarketEvent) → ServerMessage 변환                  ║
║  → SubscriptionManager.broadcast()                               ║
╚═══════════════════┬═════════════════════════════════════════════╝
                    │ ServerMessage
                    ▼
┌─────────────────────────────────────────────────────────────────┐
│              SubscriptionManager → WebSocket Clients              │
│  should_session_receive() 필터링 후 전달                          │
└─────────────────────────────────────────────────────────────────┘
```

### 핵심 패턴

| 패턴 | 설명 |
|------|------|
| **Singleton** | credential_id별 하나의 WebSocket 스트림 |
| **Bridge Task** | KR/US 스트림을 별도 tokio 태스크로 분리, mpsc 채널 통합 |
| **참조 카운트** | 여러 전략/클라이언트가 같은 심볼 구독 시 하나의 구독만 유지 |
| **Lazy 초기화** | 전략 시작 시 필요한 스트림만 생성 |
| **동적 구독** | 연결 중에도 심볼 추가/제거 가능 |

---

## 다채널 알림 아키텍처 (v0.7.2)

5개 알림 채널을 지원하며, 모든 자격 증명은 DB에 암호화 저장됩니다.

### 구조

```
┌─────────────────────────────────────────────────────────────────┐
│                    NotificationEvent                             │
│  (TradeExecuted, SignalGenerated, RiskAlert, DailyReport)       │
└─────────────────────┬───────────────────────────────────────────┘
                      │
         ┌────────────▼────────────┐
         │   NotificationRouter    │
         │  (채널별 라우팅)         │
         └────────────┬────────────┘
                      │
    ┌─────────────────┼─────────────────┬─────────────────┐
    │                 │                 │                 │
┌───▼───┐       ┌─────▼─────┐     ┌─────▼─────┐     ┌─────▼─────┐
│Telegram│       │  Discord  │     │   Slack   │     │   Email   │
│  Bot   │       │  Webhook  │     │  Webhook  │     │   SMTP    │
└────────┘       └───────────┘     └───────────┘     └───────────┘
                                                           │
                                                     ┌─────▼─────┐
                                                     │    SMS    │
                                                     │  (Twilio) │
                                                     └───────────┘
```

### 설정 관리

| 저장 위치 | 항목 | 보안 |
|----------|------|------|
| 환경변수 | `*_ENABLED` 플래그 | 평문 |
| DB (암호화) | 토큰, 웹훅 URL, 자격증명 | AES-256-GCM |
| UI | 채널 설정 CRUD | 웹 대시보드 |

---

## 시장 시간 스케줄러 (v0.7.2)

각 시장의 운영 시간을 인식하여 워크플로우 실행 시점을 최적화합니다.

### 지원 시장

| 시장 | 코드 | 시간대 | 개장 | 폐장 |
|------|------|--------|------|------|
| 한국 | KR | Asia/Seoul | 09:00 | 15:30 |
| 미국 | US | America/New_York | 09:30 | 16:00 |
| 일본 | JP | Asia/Tokyo | 09:00 | 15:30 |

### 스케줄링 로직

```
┌─────────────────────────────────────────────────────────────────┐
│                      Scheduler                                   │
├─────────────────────────────────────────────────────────────────┤
│  should_run_daily_workflow(market) → bool                       │
│                                                                 │
│  1. 주말 체크 (SCHEDULING_SKIP_WEEKENDS)                        │
│  2. 공휴일 체크 (SCHEDULING_SKIP_HOLIDAYS)                      │
│  3. 장 마감 확인 (MarketStatus::Closed)                         │
│  4. 지연 시간 경과 확인 (SCHEDULING_KRX_DELAY_MINUTES)          │
│  5. 오늘 이미 실행 여부 (last_daily_run)                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## 리스크 관리 아키텍처

### 검증 파이프라인

```
Signal (from Strategy)
    │
    ▼
┌────────────────────────────────────────────────┐
│              RiskManager.validate_order()       │
├────────────────────────────────────────────────┤
│ 1. 일일 손실 한도 확인                          │
│    - can_trade() → false면 거부                │
├────────────────────────────────────────────────┤
│ 2. 심볼 활성화 확인                             │
│    - 비활성 심볼이면 거부                       │
├────────────────────────────────────────────────┤
│ 3. 변동성 필터                                  │
│    - volatility > threshold → 거부/경고        │
├────────────────────────────────────────────────┤
│ 4. 포지션 사이징 검증                           │
│    - 단일 포지션 한도 (10%)                    │
│    - 총 노출 한도 (50%)                        │
│    - 동시 포지션 제한 (10개)                   │
│    - 최소 주문 크기                            │
├────────────────────────────────────────────────┤
│ 5. 일일 손실 경고                               │
│    - 70%+ 경고, 90%+ 위험                      │
└────────────────────────────────────────────────┘
    │
    ▼
Order Execution (if valid)
```

### 트레일링 스탑 모드

| 모드 | 동작 |
|------|------|
| FixedPercentage | 고정 비율로 가격 추적 (기본 1.5%) |
| AtrBased | ATR × 배수로 변동성 기반 추적 |
| Step-Based | 수익률 구간별 다른 추적 비율 |
| Parabolic SAR | 가속 계수 기반 포물선 추적 |

---

## 데이터베이스 스키마

> **마이그레이션 위치**: `migrations_v2/` (9개 파일)

### 마이그레이션 구성

| 파일 | 내용 |
|------|------|
| 01_core_foundation | Extensions, ENUM 타입, 핵심 테이블 (symbols, orders, trades, positions, signals, strategies, credentials) |
| 02_data_management | symbol_info, symbol_fundamental, ohlcv, execution_cache, 메타데이터 뷰 |
| 03_trading_analytics | trade_executions, position_snapshots, 분석 뷰 |
| 04_strategy_signals | route_state ENUM, signal_marker, alert_rule, alert_history |
| 05_evaluation_ranking | price_snapshot, global_score, reality_check, score_history |
| 06_user_settings | backtest_results, portfolio_equity_history |
| 07_performance_optimization | 인덱스 최적화, Materialized View |
| 08_paper_trading | mock_exchange_state, mock_positions, mock_executions, paper_trading_sessions |
| 09_strategy_watched_tickers | 전략별 관심 종목 (Collector 우선순위 연동) |

### TimescaleDB Hypertables

| 테이블 | 파티션 키 | 보존 정책 | 용도 |
|--------|----------|----------|------|
| ohlcv | open_time | - | OHLCV 캔들 데이터 (Yahoo Finance, KRX 등 통합) |
| trade_ticks | time | 6개월 | 실시간 틱 데이터 |
| credential_access_logs | accessed_at | 90일 | 자격증명 접근 감사 로그 |
| price_snapshot | snapshot_date | - | 추천 종목 스냅샷 |

### 핵심 테이블 그룹

**거래 핵심**
| 테이블 | 설명 |
|--------|------|
| symbols | 심볼 메타데이터 (거래소 제약 조건 포함) |
| orders | 주문 기록 (상태 추적) |
| trades | 실제 체결 내역 |
| positions | 보유 포지션 |
| signals | 전략 발생 시그널 |

**전략 관리**
| 테이블 | 설명 |
|--------|------|
| strategies | 전략 설정 및 상태 (credential_id 연결) |
| strategy_watched_tickers | 전략별 관심 종목 (Collector 연동) |
| strategy_presets | 전략 파라미터 프리셋 |

**시장 데이터**
| 테이블 | 설명 |
|--------|------|
| symbol_info | 종목 기본 정보 (ticker, name, sector) |
| symbol_fundamental | 펀더멘털 데이터 (PER, PBR, ROE 등) |
| ohlcv_metadata | OHLCV 캐시 상태 메타데이터 |
| mv_latest_prices | 최신 가격 Materialized View |

**Paper Trading (v0.8.0)**
| 테이블 | 설명 |
|--------|------|
| mock_exchange_state | Mock 거래소 잔고 상태 |
| mock_positions | Mock 포지션 (strategy_id 연결) |
| mock_executions | Mock 체결 내역 (strategy_id 연결) |
| paper_trading_sessions | 전략별 Paper Trading 세션 |

**인증 및 설정**
| 테이블 | 설명 |
|--------|------|
| exchange_credentials | 암호화된 거래소 API 키 (AES-256-GCM) |
| telegram_settings | 텔레그램 알림 설정 |
| app_settings | 애플리케이션 전역 설정 (key-value) |
| watchlist | 사용자 관심 종목 |

---

## 실행 환경

### Docker 구성 (인프라만)

| 서비스 | 포트 | 설명 |
|--------|------|------|
| timescaledb | 5432 | TimescaleDB (PostgreSQL 15) |
| redis | 6379 | Redis 7 |

### 로컬 실행

API 서버와 프론트엔드는 로컬에서 직접 실행합니다:

```bash
# 인프라 시작
docker-compose up -d timescaledb redis

# API 서버 (별도 터미널)
export DATABASE_URL=postgresql://trader:trader_secret@localhost:5432/trader
export REDIS_URL=redis://localhost:6379
cargo run --bin trader-api --features ml --release

# 프론트엔드 (별도 터미널)
cd frontend && npm run dev
```

### ML 훈련 (선택적)

```bash
docker-compose --profile ml run --rm trader-ml python scripts/train_ml_model.py
```

---

## 보안

### 자격증명 암호화
- **알고리즘**: AES-256-GCM
- **키 관리**: 환경 변수 또는 Docker Secret
- **저장**: `exchange_credentials` 테이블 (암호화된 상태)

### API 보안
- Rate Limiting (향후 구현)
- CORS 설정
- 입력 유효성 검증 (Validator)

---

## 성능 최적화

### TimescaleDB
- Hypertable 자동 파티셔닝
- 압축 정책 (7일 이상 데이터)
- 연속 집계 (continuous aggregates)

### Redis 캐싱
- 실시간 시세 데이터
- 세션 관리
- Rate Limit 카운터

### API Repository 최적화 (v0.7.4)
- N+1 쿼리 제거: HashMap 배치 사전 조회, ANY($1) 배치 중복 체크
- UNNEST 배치 Upsert: 루프 INSERT → 단일 `UNNEST($1::type[], ...)` (500건/배치)
- Materialized View 활용: `mv_sector_rs` (섹터 RS), `mv_symbol_screening` (스크리닝)
- Collector 사전계산 활용: RouteState, Indicator → DB에서 직접 JOIN
- GlobalScore/SevenFactor Redis 캐싱 (6h/2h TTL)

### Rust 최적화
- async/await 비동기 처리
- Zero-copy 직렬화
- 컴파일 타임 SQL 검증 (SQLx)

---

## 데이터 프로바이더 아키텍처

### 다중 소스 구조

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Data Provider Layer                                │
├─────────────────────┬─────────────────────┬─────────────────────────────────┤
│   KRX OPEN API      │   Naver Finance     │        Yahoo Finance            │
│   (국내 OHLCV)      │  (국내 펀더멘털)    │   (해외 주식 / 암호화폐)         │
├─────────────────────┼─────────────────────┼─────────────────────────────────┤
│ • OHLCV 데이터      │ • PER/PBR/ROE       │ • OHLCV 데이터                  │
│ • 호가/체결         │ • EPS/BPS           │ • 심볼 정보                     │
│ • 실시간 시세       │ • 배당수익률        │ • Fundamental (Yahoo Quote)     │
│                     │ • 시가총액/업종     │ • 실시간 시세 (Fallback)        │
│                     │ • 52주 고저         │                                 │
└─────────────────────┴─────────────────────┴─────────────────────────────────┘
         │                    │                          │
         └────────────────────┼──────────────────────────┘
                              │
                   ┌──────────▼──────────┐
                   │  DataProviderConfig  │
                   │ ────────────────────│
                   │ krx_api_enabled     │  ← PROVIDER_KRX_API_ENABLED
                   │ yahoo_enabled       │  ← PROVIDER_YAHOO_ENABLED
                   │ naver_enabled       │  ← PROVIDER_NAVER_ENABLED
                   └─────────────────────┘
```

### 환경변수 설정

| 변수명 | 기본값 | 설명 |
|--------|--------|------|
| `PROVIDER_KRX_API_ENABLED` | false | KRX API 활성화 (승인 필요) |
| `PROVIDER_YAHOO_ENABLED` | true | Yahoo Finance 활성화 |
| `PROVIDER_NAVER_ENABLED` | true | Naver Finance 활성화 (국내 펀더멘털) |

---

## Multi Timeframe 아키텍처

### 데이터 흐름

```
┌─────────────────────────────────────────────────────────────┐
│                 Multi Timeframe Request                      │
│               GET /api/v1/market/klines/multi               │
└─────────────────────┬───────────────────────────────────────┘
                      │
         ┌────────────▼────────────┐
         │   TimeframeAligner      │
         │ ────────────────────────│
         │ • Look-Ahead Bias 방지  │
         │ • 타임프레임 정렬       │
         └────────────┬────────────┘
                      │
    ┌─────────────────┼─────────────────┐
    │                 │                 │
┌───▼───┐       ┌─────▼─────┐     ┌─────▼─────┐
│ 1분봉  │       │   5분봉   │     │   일봉    │
│(Primary)│       │(Secondary)│     │(Secondary)│
└───┬───┘       └─────┬─────┘     └─────┬─────┘
    │                 │                 │
    └─────────────────┼─────────────────┘
                      │
         ┌────────────▼────────────┐
         │  MultiTimeframeHelpers  │
         │ ────────────────────────│
         │ • analyze_trend()       │
         │ • combine_signals()     │
         │ • detect_divergence()   │
         └─────────────────────────┘
```

---

## 스크리닝 기반 전략 아키텍처 (v0.7.2)

고정된 티커 목록 대신 스크리닝 결과에서 동적으로 종목을 선택하는 전략 패턴입니다.

### 변형 (Variant)

| 변형 | 설명 | 리밸런싱 주기 |
|------|------|--------------|
| SmallCapQuant | 소형주 퀀트 (재무 필터 + GlobalScore) | 월 1회 |
| PensionBot | 연금 자동화 (모멘텀 + 자산 배분) | 월 1회 |
| DynamicUniverse | 일반 동적 유니버스 | 주 1회 |

### 동작 흐름

```
┌─────────────────────────────────────────────────────────────────┐
│                    ScreeningBasedStrategy                        │
└─────────────────────┬───────────────────────────────────────────┘
                      │
         ┌────────────▼────────────┐
         │   스크리닝 결과 조회     │
         │  (StrategyContext)      │
         └────────────┬────────────┘
                      │
         ┌────────────▼────────────┐
         │   상위 N개 종목 선정     │
         │  (GlobalScore 기준)     │
         └────────────┬────────────┘
                      │
         ┌────────────▼────────────┐
         │   리밸런싱 계산          │
         │  (RebalanceCalculator)  │
         └────────────┬────────────┘
                      │
         ┌────────────▼────────────┐
         │   Entry/Exit 신호 발행  │
         └─────────────────────────┘
```

---

## 마이그레이션 관리 아키텍처 (v0.7.2)

SQL 마이그레이션 파일을 분석하고 검증하는 도구입니다.

### 구성 요소

```
trader-core/src/migration/
├── analyzer.rs      # SQL 파싱 (CREATE/DROP/ALTER)
│                    # 의존성 그래프 생성
│
├── validator.rs     # 문제 검출
│                    # - DUP001: 중복 정의
│                    # - CASC001: CASCADE 사용
│                    # - CIRC001: 순환 의존성
│                    # - IDEM001: 멱등성 누락
│
├── consolidator.rs  # 통합 계획 생성
│                    # - 논리적 그룹 분류
│                    # - IF NOT EXISTS 자동 주입
│
└── models.rs        # 데이터 구조
                     # - SqlStatement, MigrationFile
                     # - ValidationIssue, ConsolidationPlan
```

### CLI 명령어

```bash
trader migrate verify      # 검증 (160개 이슈 검출)
trader migrate consolidate # 통합 (18개 → 7개)
trader migrate graph       # 의존성 시각화
trader migrate apply       # 마이그레이션 적용
```

---

## 참고 문서

### 아키텍처 및 설계
- [API 문서](./api.md) - REST/WebSocket API 명세
- [전략 가이드](./STRATEGY_GUIDE.md) - 26개 전략 상세 설명
- [SignalProcessor 설계](./trade_executor_design.md) - 거래 실행 공통 모듈

### 운영 및 배포
- [설치/배포 가이드](./setup_guide.md) - 인프라, 환경설정, 배포
- [데이터 수집 가이드](./data_collection.md) - Standalone 데이터 수집기
- [마이그레이션 가이드](./migration_guide.md) - DB 마이그레이션 도구
- [테스트 워크플로우](./fulltest_workflow.md) - 전략 검증 프로세스, 회귀 테스트

### 외부 연동
- [KRX API 스펙](./krx_openapi_spec.md) - KRX OPEN API 명세
- [Multi KLine 가이드](./multiple_kline_period_implementation_guide.md) - 다중 타임프레임

### 프로젝트 관리
- [TODO 목록](./todo.md) - 진행 중/남은 작업
- [PRD v5.0](./prd.md) - 제품 요구사항

---

*문서 생성일: 2026-02-07*
