# ZeroQuant 구조 개선 분석

> 작성일: 2026-01-30
> 버전: 1.0
> 분석 대상: ZeroQuant v0.3.0

---

## 📋 목차

1. [개요](#개요)
2. [현재 아키텍처 분석](#1-현재-아키텍처-분석)
3. [모듈 구조 개선](#2-모듈-구조-개선)
4. [디자인 패턴 적용](#3-디자인-패턴-적용)
5. [레이어 분리 개선](#4-레이어-분리-개선)
6. [의존성 관리](#5-의존성-관리)
7. [테스트 구조](#6-테스트-구조)
8. [우선순위 요약](#7-우선순위-요약)

---

## 개요

본 문서는 ZeroQuant 프로젝트의 코드 구조와 아키텍처를 분석하여, 더 나은 설계 및 유지보수성을 위한 구조적 개선 사항을 제안합니다. **코드 수정은 하지 않고**, 순수하게 구조적 개선 기회만 식별합니다.

### 현재 구조 강점 ✅

- **명확한 크레이트 분리**: 10개의 독립적인 크레이트
- **Domain-Driven Design**: trader-core에 도메인 모델 집중
- **플러그인 시스템**: 전략 동적 로딩 지원
- **Repository 패턴**: 일부 데이터 접근 계층에 적용

### 개선 기회 영역

| 영역 | 현재 상태 | 개선 필요도 |
|------|-----------|-------------|
| 레이어 분리 | 부분적 | 🟡 중간 |
| 의존성 방향 | 양호 | 🟢 낮음 |
| 에러 처리 | 분산됨 | 🔴 높음 |
| 도메인 서비스 | 부재 | 🟡 중간 |
| 이벤트 시스템 | 부재 | 🟢 낮음 |
| 테스트 구조 | 기본적 | 🟡 중간 |

---

## 1. 현재 아키텍처 분석

### 1.1 크레이트 구조

**현재 구조**:
```
zeroquant/
├── trader-core          # 도메인 모델
├── trader-exchange      # 거래소 연동
├── trader-strategy      # 전략 엔진
├── trader-risk          # 리스크 관리
├── trader-execution     # 주문 실행
├── trader-data          # 데이터 저장/캐싱
├── trader-analytics     # 분석 및 백테스트
├── trader-api           # REST API
├── trader-cli           # CLI 도구
└── trader-notification  # 알림
```

**의존성 흐름**:
```
trader-api
  ├─→ trader-core
  ├─→ trader-exchange
  ├─→ trader-strategy
  ├─→ trader-risk
  ├─→ trader-execution
  ├─→ trader-data
  ├─→ trader-analytics
  └─→ trader-notification

trader-analytics
  ├─→ trader-core
  ├─→ trader-strategy
  └─→ trader-data

trader-execution
  ├─→ trader-core
  ├─→ trader-exchange
  └─→ trader-risk
```

---

### 1.2 현재 레이어 구조

```
┌─────────────────────────────────────────┐
│   Presentation Layer (trader-api)       │
│   - REST endpoints                      │
│   - WebSocket                           │
│   - Auth/Middleware                     │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│   Application Layer (분산됨)            │
│   - StrategyEngine                      │
│   - OrderExecutor                       │
│   - RiskManager                         │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│   Domain Layer (trader-core)            │
│   - Order, Position, Trade              │
│   - Signal, MarketData                  │
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│   Infrastructure Layer                  │
│   - trader-exchange (외부 API)          │
│   - trader-data (DB, Cache)             │
└─────────────────────────────────────────┘
```

**문제점**:
- Application Layer가 여러 크레이트에 분산됨
- Domain Service 개념이 명확하지 않음
- 일부 비즈니스 로직이 API 레이어에 존재

---

## 2. 모듈 구조 개선

### 2.1 Domain Service 레이어 추가 🟡 중간

**문제점**:
- 복잡한 비즈니스 로직이 API 핸들러에 존재
- 도메인 엔티티 간의 조율 로직 산재

**제안**: `trader-domain-services` 크레이트 신설

```rust
// crates/trader-domain-services/src/lib.rs

/// 포트폴리오 관리 서비스
pub struct PortfolioService {
    position_repo: Arc<dyn PositionRepository>,
    order_repo: Arc<dyn OrderRepository>,
    risk_manager: Arc<RiskManager>,
}

impl PortfolioService {
    /// 전체 포트폴리오 가치 계산
    pub async fn calculate_portfolio_value(&self) -> Result<PortfolioValue> {
        let positions = self.position_repo.get_all_open().await?;
        let market_prices = self.fetch_current_prices(&positions).await?;
        
        // 복잡한 비즈니스 로직
        Ok(PortfolioValue::calculate(positions, market_prices))
    }
    
    /// 리밸런싱 필요 여부 판단
    pub async fn needs_rebalancing(&self, strategy: &Strategy) -> Result<bool> {
        // 전략별 리밸런싱 로직
    }
}

/// 주문 처리 서비스
pub struct OrderService {
    executor: Arc<OrderExecutor>,
    risk_manager: Arc<RiskManager>,
    position_tracker: Arc<PositionTracker>,
}

impl OrderService {
    /// 신호를 받아 주문 생성 및 실행
    pub async fn process_signal(&self, signal: Signal) -> Result<ExecutionResult> {
        // 1. 리스크 검증
        self.risk_manager.validate_signal(&signal).await?;
        
        // 2. 주문 생성
        let order = self.create_order_from_signal(&signal)?;
        
        // 3. 실행
        let result = self.executor.execute(order).await?;
        
        // 4. 포지션 업데이트
        self.position_tracker.update_from_execution(&result).await?;
        
        Ok(result)
    }
}
```

**효과**:
- API 핸들러가 얇아짐 (thin controller)
- 비즈니스 로직 재사용 가능 (CLI, 테스트 등)
- 단위 테스트 용이

---

### 2.2 Use Case 레이어 (Application Service) 🟢 낮음

**제안**: Clean Architecture의 Use Case 패턴 적용

```rust
// crates/trader-application/src/use_cases/mod.rs

/// 백테스트 실행 Use Case
pub struct RunBacktestUseCase {
    strategy_repo: Arc<dyn StrategyRepository>,
    data_provider: Arc<dyn HistoricalDataProvider>,
    backtest_engine: Arc<BacktestEngine>,
}

impl RunBacktestUseCase {
    pub async fn execute(&self, request: RunBacktestRequest) -> Result<BacktestReport> {
        // 1. 전략 조회
        let strategy = self.strategy_repo.find_by_id(&request.strategy_id).await?;
        
        // 2. 데이터 로드
        let klines = self.data_provider.get_klines(
            &request.symbol,
            request.start_date,
            request.end_date,
        ).await?;
        
        // 3. 백테스트 실행
        let report = self.backtest_engine.run(strategy, klines).await?;
        
        Ok(report)
    }
}

/// 전략 시작 Use Case
pub struct StartStrategyUseCase {
    strategy_engine: Arc<StrategyEngine>,
    market_stream: Arc<dyn MarketStream>,
    order_service: Arc<OrderService>,
}

impl StartStrategyUseCase {
    pub async fn execute(&self, strategy_id: Uuid) -> Result<()> {
        // 복잡한 전략 시작 로직
        // - 마켓 데이터 구독
        // - 신호 처리 루프
        // - 에러 복구
    }
}
```

**구조**:
```
trader-api (Presentation)
    │
    ├─→ RunBacktestUseCase
    ├─→ StartStrategyUseCase
    ├─→ PlaceOrderUseCase
    │
    └─→ trader-domain-services
            │
            └─→ trader-core (Domain)
```

---

### 2.3 Repository 패턴 완성 🟡 중간

**현재 상태**: 부분적으로만 적용됨

**제안**: 모든 데이터 접근을 Repository 인터페이스로 통일

```rust
// crates/trader-core/src/repositories/mod.rs

/// 주문 저장소 인터페이스
#[async_trait]
pub trait OrderRepository: Send + Sync {
    async fn save(&self, order: &Order) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Order>>;
    async fn find_by_symbol(&self, symbol: &str) -> Result<Vec<Order>>;
    async fn find_open_orders(&self) -> Result<Vec<Order>>;
    async fn update_status(&self, id: Uuid, status: OrderStatus) -> Result<()>;
}

/// 포지션 저장소 인터페이스
#[async_trait]
pub trait PositionRepository: Send + Sync {
    async fn save(&self, position: &Position) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Position>>;
    async fn find_open_positions(&self) -> Result<Vec<Position>>;
    async fn close(&self, id: Uuid) -> Result<()>;
}

/// 전략 저장소 인터페이스
#[async_trait]
pub trait StrategyRepository: Send + Sync {
    async fn save(&self, strategy: &StrategyConfig) -> Result<Uuid>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<StrategyConfig>>;
    async fn find_all(&self) -> Result<Vec<StrategyConfig>>;
    async fn update(&self, id: Uuid, config: &StrategyConfig) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
}
```

**구현**:
```rust
// crates/trader-data/src/repositories/postgres_order_repo.rs

pub struct PostgresOrderRepository {
    pool: PgPool,
}

#[async_trait]
impl OrderRepository for PostgresOrderRepository {
    async fn save(&self, order: &Order) -> Result<()> {
        sqlx::query!(
            "INSERT INTO orders (...) VALUES (...)",
            // ...
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    // ... 나머지 구현
}

// 테스트용 In-Memory 구현
pub struct InMemoryOrderRepository {
    orders: Arc<RwLock<HashMap<Uuid, Order>>>,
}

#[async_trait]
impl OrderRepository for InMemoryOrderRepository {
    async fn save(&self, order: &Order) -> Result<()> {
        let mut orders = self.orders.write().await;
        orders.insert(order.id, order.clone());
        Ok(())
    }
    
    // ...
}
```

**효과**:
- 데이터 소스 교체 용이 (DB, 파일, 메모리)
- 테스트 시 Mock Repository 사용
- 도메인 로직과 인프라 완전 분리

---

### 2.4 에러 처리 통합 🔴 높음

**문제점**:
- 각 크레이트마다 독립적인 에러 타입
- 에러 변환 코드 중복

**현재**:
```rust
// trader-exchange/src/error.rs
pub enum ExchangeError { ... }

// trader-data/src/error.rs
pub enum DataError { ... }

// trader-strategy/src/error.rs
pub enum StrategyError { ... }

// 변환 코드 필요
impl From<ExchangeError> for ApplicationError { ... }
impl From<DataError> for ApplicationError { ... }
```

**제안**: 통합 에러 계층 구조

```rust
// crates/trader-core/src/error.rs

/// 최상위 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 도메인 에러
    #[error(transparent)]
    Domain(#[from] DomainError),
    
    /// 인프라 에러
    #[error(transparent)]
    Infrastructure(#[from] InfrastructureError),
    
    /// 외부 서비스 에러
    #[error(transparent)]
    External(#[from] ExternalError),
    
    /// 검증 에러
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// 도메인 에러 (비즈니스 규칙 위반)
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Insufficient funds: required={required}, available={available}")]
    InsufficientFunds { required: Decimal, available: Decimal },
    
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    
    #[error("Position not found: {0}")]
    PositionNotFound(Uuid),
    
    #[error("Risk limit exceeded: {0}")]
    RiskLimitExceeded(String),
}

/// 인프라 에러 (DB, 캐시 등)
#[derive(Debug, thiserror::Error)]
pub enum InfrastructureError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
}

/// 외부 서비스 에러 (거래소 API 등)
#[derive(Debug, thiserror::Error)]
pub enum ExternalError {
    #[error("Exchange API error: {code} - {message}")]
    ExchangeApi { code: String, message: String },
    
    #[error("Network timeout: {0}")]
    NetworkTimeout(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

/// 검증 에러
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid parameter: {field} - {reason}")]
    InvalidParameter { field: String, reason: String },
    
    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

**HTTP 응답 매핑**:
```rust
// crates/trader-api/src/error.rs

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            AppError::Domain(e) => match e {
                DomainError::InsufficientFunds { .. } => {
                    (StatusCode::BAD_REQUEST, "INSUFFICIENT_FUNDS", e.to_string())
                }
                DomainError::RiskLimitExceeded(_) => {
                    (StatusCode::FORBIDDEN, "RISK_LIMIT_EXCEEDED", e.to_string())
                }
                _ => (StatusCode::BAD_REQUEST, "DOMAIN_ERROR", e.to_string()),
            },
            AppError::Infrastructure(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INFRASTRUCTURE_ERROR", "Internal server error".to_string())
            },
            AppError::External(e) => match e {
                ExternalError::RateLimitExceeded => {
                    (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT", "Rate limit exceeded".to_string())
                }
                _ => (StatusCode::BAD_GATEWAY, "EXTERNAL_ERROR", e.to_string()),
            },
            AppError::Validation(_) => {
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", self.to_string())
            },
        };
        
        let body = json!({
            "error": {
                "code": error_code,
                "message": message,
            }
        });
        
        (status, Json(body)).into_response()
    }
}
```

---

## 3. 디자인 패턴 적용

### 3.1 Factory 패턴 강화 🟢 낮음

**현재**: 전략 생성이 분산됨

**제안**: 통합 Factory

```rust
// crates/trader-strategy/src/factory.rs

pub struct StrategyFactory {
    builders: HashMap<String, Box<dyn StrategyBuilder>>,
}

pub trait StrategyBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn Strategy>>;
    fn schema(&self) -> StrategySchema;
}

impl StrategyFactory {
    pub fn new() -> Self {
        let mut factory = Self {
            builders: HashMap::new(),
        };
        
        // 내장 전략 등록
        factory.register("rsi", Box::new(RsiStrategyBuilder));
        factory.register("grid", Box::new(GridStrategyBuilder));
        factory.register("haa", Box::new(HaaStrategyBuilder));
        // ...
        
        factory
    }
    
    pub fn create(&self, strategy_type: &str, config: Value) -> Result<Box<dyn Strategy>> {
        let builder = self.builders.get(strategy_type)
            .ok_or_else(|| StrategyError::UnknownType(strategy_type.to_string()))?;
        
        builder.build(config)
    }
    
    pub fn list_available(&self) -> Vec<StrategyMetadata> {
        self.builders.iter()
            .map(|(name, builder)| StrategyMetadata {
                name: name.clone(),
                schema: builder.schema(),
            })
            .collect()
    }
}
```

---

### 3.2 Observer 패턴 (이벤트 시스템) 🟢 낮음

**제안**: 도메인 이벤트 시스템

```rust
// crates/trader-core/src/events/mod.rs

/// 도메인 이벤트
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    /// 주문 생성됨
    OrderPlaced {
        id: Uuid,
        symbol: String,
        side: Side,
        quantity: Decimal,
        price: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// 주문 체결됨
    OrderFilled {
        id: Uuid,
        filled_quantity: Decimal,
        average_price: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// 포지션 개설됨
    PositionOpened {
        id: Uuid,
        symbol: String,
        side: Side,
        quantity: Decimal,
        entry_price: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// 포지션 청산됨
    PositionClosed {
        id: Uuid,
        exit_price: Decimal,
        pnl: Decimal,
        timestamp: DateTime<Utc>,
    },
    
    /// 리스크 위반
    RiskViolation {
        reason: String,
        severity: RiskSeverity,
        timestamp: DateTime<Utc>,
    },
}

/// 이벤트 핸들러
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &DomainEvent) -> Result<()>;
}

/// 이벤트 버스
pub struct EventBus {
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self { handlers: Vec::new() }
    }
    
    pub fn subscribe(&mut self, handler: Arc<dyn EventHandler>) {
        self.handlers.push(handler);
    }
    
    pub async fn publish(&self, event: DomainEvent) {
        for handler in &self.handlers {
            if let Err(e) = handler.handle(&event).await {
                tracing::error!("Event handler error: {:?}", e);
            }
        }
    }
}
```

**사용 예시**:
```rust
// 주문 실행 시 이벤트 발행
impl OrderExecutor {
    pub async fn execute(&self, order: Order) -> Result<ExecutionResult> {
        // 주문 실행
        let result = self.exchange.place_order(&order).await?;
        
        // 이벤트 발행
        self.event_bus.publish(DomainEvent::OrderPlaced {
            id: order.id,
            symbol: order.symbol.clone(),
            side: order.side,
            quantity: order.quantity,
            price: order.price,
            timestamp: Utc::now(),
        }).await;
        
        Ok(result)
    }
}

// 이벤트 핸들러 예시
struct NotificationHandler {
    telegram: TelegramBot,
}

#[async_trait]
impl EventHandler for NotificationHandler {
    async fn handle(&self, event: &DomainEvent) -> Result<()> {
        match event {
            DomainEvent::OrderFilled { id, average_price, .. } => {
                self.telegram.send_message(&format!(
                    "주문 체결: {} @ {}",
                    id, average_price
                )).await?;
            }
            DomainEvent::RiskViolation { reason, severity, .. } => {
                if *severity == RiskSeverity::Critical {
                    self.telegram.send_alert(reason).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

---

### 3.3 Strategy 패턴 개선 🟢 낮음

**현재**: 잘 적용됨 ✅

**추가 제안**: Context 객체로 전략 실행 캡슐화

```rust
// crates/trader-strategy/src/context.rs

/// 전략 실행 컨텍스트
pub struct StrategyContext {
    strategy: Box<dyn Strategy>,
    state: StrategyState,
    performance: PerformanceTracker,
}

impl StrategyContext {
    pub async fn execute(&mut self, market_data: &MarketData) -> Result<Vec<Signal>> {
        // 1. 성능 측정 시작
        let start = Instant::now();
        
        // 2. 전략 실행
        let signals = self.strategy.on_market_data(market_data).await?;
        
        // 3. 성능 기록
        self.performance.record_execution(start.elapsed());
        
        // 4. 상태 업데이트
        self.state.last_executed = Utc::now();
        self.state.execution_count += 1;
        
        Ok(signals)
    }
    
    pub fn get_statistics(&self) -> StrategyStatistics {
        StrategyStatistics {
            execution_count: self.state.execution_count,
            average_execution_time: self.performance.average_time(),
            signals_generated: self.state.signals_generated,
            // ...
        }
    }
}
```

---

### 3.4 Builder 패턴 🟢 낮음

**제안**: 복잡한 객체 생성에 Builder 사용

```rust
// crates/trader-analytics/src/backtest/builder.rs

pub struct BacktestBuilder {
    config: BacktestConfig,
    strategies: Vec<Box<dyn Strategy>>,
    data_sources: Vec<Box<dyn HistoricalDataProvider>>,
    plugins: Vec<Box<dyn BacktestPlugin>>,
}

impl BacktestBuilder {
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            config: BacktestConfig::new(initial_capital),
            strategies: Vec::new(),
            data_sources: Vec::new(),
            plugins: Vec::new(),
        }
    }
    
    pub fn with_strategy(mut self, strategy: Box<dyn Strategy>) -> Self {
        self.strategies.push(strategy);
        self
    }
    
    pub fn with_commission(mut self, rate: Decimal) -> Self {
        self.config.commission_rate = rate;
        self
    }
    
    pub fn with_slippage(mut self, rate: Decimal) -> Self {
        self.config.slippage_rate = rate;
        self
    }
    
    pub fn with_plugin(mut self, plugin: Box<dyn BacktestPlugin>) -> Self {
        self.plugins.push(plugin);
        self
    }
    
    pub fn build(self) -> Result<BacktestEngine> {
        // 검증
        if self.strategies.is_empty() {
            return Err(BacktestError::NoStrategies);
        }
        
        Ok(BacktestEngine {
            config: self.config,
            strategies: self.strategies,
            data_sources: self.data_sources,
            plugins: self.plugins,
        })
    }
}

// 사용
let backtest = BacktestBuilder::new(dec!(100_000))
    .with_strategy(Box::new(RsiStrategy::new()))
    .with_commission(dec!(0.001))
    .with_slippage(dec!(0.0005))
    .with_plugin(Box::new(PerformanceAnalyzer))
    .build()?;
```

---

## 4. 레이어 분리 개선

### 4.1 Hexagonal Architecture (Ports & Adapters) 🟡 중간

**제안**: 명확한 Port와 Adapter 분리

```
┌─────────────────────────────────────────────────┐
│              Application Core                   │
│  ┌──────────────────────────────────────┐      │
│  │      Domain Layer                     │      │
│  │  - Order, Position, Trade             │      │
│  │  - Strategy, Signal                   │      │
│  └──────────────────────────────────────┘      │
│  ┌──────────────────────────────────────┐      │
│  │   Application Services (Use Cases)    │      │
│  │  - RunBacktestUseCase                 │      │
│  │  - StartStrategyUseCase               │      │
│  │  - PlaceOrderUseCase                  │      │
│  └──────────────────────────────────────┘      │
│                                                  │
│  ┌──────────────────────────────────────┐      │
│  │      Ports (Interfaces)               │      │
│  │  - OrderRepository                    │      │
│  │  - Exchange                           │      │
│  │  - MarketDataProvider                 │      │
│  └──────────────────────────────────────┘      │
└──────────────────┬──────────────────────────────┘
                   │
    ┌──────────────┴───────────────┐
    │                              │
┌───▼──────────────┐  ┌───────────▼──────────┐
│  Adapters (In)   │  │  Adapters (Out)      │
│  - REST API      │  │  - PostgresRepo      │
│  - CLI           │  │  - BinanceExchange   │
│  - WebSocket     │  │  - KisExchange       │
└──────────────────┘  │  - RedisCache        │
                      └──────────────────────┘
```

**구현 예시**:
```rust
// crates/trader-core/src/ports/exchange.rs (Port)

#[async_trait]
pub trait ExchangePort: Send + Sync {
    async fn place_order(&self, order: &Order) -> Result<OrderId>;
    async fn cancel_order(&self, order_id: OrderId) -> Result<()>;
    async fn get_balance(&self, asset: &str) -> Result<Decimal>;
    async fn get_ticker(&self, symbol: &Symbol) -> Result<Ticker>;
}

// crates/trader-exchange/src/adapters/binance.rs (Adapter)

pub struct BinanceAdapter {
    client: BinanceClient,
}

#[async_trait]
impl ExchangePort for BinanceAdapter {
    async fn place_order(&self, order: &Order) -> Result<OrderId> {
        // Binance API 호출
        let response = self.client.new_order(/* ... */).await?;
        Ok(OrderId::from(response.order_id))
    }
    
    // ...
}
```

---

### 4.2 API 레이어 얇게 만들기 🟡 중간

**현재 문제**: 일부 핸들러가 너무 많은 로직 포함

**예시** (현재):
```rust
// crates/trader-api/src/routes/backtest.rs (3,854줄!)

pub async fn run_backtest(
    State(state): State<AppState>,
    Json(request): Json<RunBacktestRequest>,
) -> Result<Json<BacktestReport>> {
    // 100줄 이상의 로직...
    // - 전략 생성
    // - 데이터 로드
    // - 백테스트 실행
    // - 결과 가공
    // - 데이터베이스 저장
}
```

**개선**:
```rust
// crates/trader-api/src/routes/backtest.rs

pub async fn run_backtest(
    State(state): State<AppState>,
    Json(request): Json<RunBacktestRequest>,
) -> Result<Json<BacktestReport>> {
    // Use Case에 위임 (5-10줄)
    let use_case = RunBacktestUseCase::new(
        state.strategy_repo,
        state.data_provider,
        state.backtest_engine,
    );
    
    let report = use_case.execute(request.into()).await?;
    
    Ok(Json(report))
}

// crates/trader-application/src/use_cases/run_backtest.rs

impl RunBacktestUseCase {
    pub async fn execute(&self, request: RunBacktestCommand) -> Result<BacktestReport> {
        // 실제 비즈니스 로직 (100줄)
        // - 전략 생성
        // - 데이터 로드
        // - 백테스트 실행
        // - 결과 가공
        // - 데이터베이스 저장
    }
}
```

---

## 5. 의존성 관리

### 5.1 의존성 역전 원칙 (DIP) 🟡 중간

**현재**: 일부 구체적 구현에 의존

**개선**:
```rust
// Before: 구체적 구현 의존
pub struct OrderService {
    binance: BinanceExchange,  // 구체적!
    postgres: PostgresRepo,    // 구체적!
}

// After: 인터페이스 의존
pub struct OrderService {
    exchange: Arc<dyn ExchangePort>,      // 추상!
    order_repo: Arc<dyn OrderRepository>, // 추상!
}
```

**Dependency Injection**:
```rust
// crates/trader-api/src/di/container.rs

pub struct Container {
    // Repositories
    order_repo: Arc<dyn OrderRepository>,
    position_repo: Arc<dyn PositionRepository>,
    strategy_repo: Arc<dyn StrategyRepository>,
    
    // Services
    order_service: Arc<OrderService>,
    portfolio_service: Arc<PortfolioService>,
    
    // Use Cases
    run_backtest: Arc<RunBacktestUseCase>,
    start_strategy: Arc<StartStrategyUseCase>,
}

impl Container {
    pub fn new(config: &Config) -> Result<Self> {
        // 의존성 조립
        let db_pool = PgPool::connect(&config.database_url).await?;
        
        // Repositories
        let order_repo = Arc::new(PostgresOrderRepository::new(db_pool.clone()));
        let position_repo = Arc::new(PostgresPositionRepository::new(db_pool.clone()));
        
        // Services
        let order_service = Arc::new(OrderService::new(
            order_repo.clone(),
            position_repo.clone(),
        ));
        
        // Use Cases
        let run_backtest = Arc::new(RunBacktestUseCase::new(
            strategy_repo.clone(),
            data_provider,
            backtest_engine,
        ));
        
        Ok(Self {
            order_repo,
            position_repo,
            order_service,
            run_backtest,
        })
    }
}
```

---

### 5.2 Feature Flags 🟢 낮음

**제안**: 선택적 기능 컴파일

```toml
# crates/trader-analytics/Cargo.toml

[features]
default = ["backtest"]
backtest = []
ml = ["ort", "ndarray"]
advanced-indicators = ["ta"]
plotting = ["plotters"]

[dependencies]
ort = { version = "2.0", optional = true }
ndarray = { version = "0.15", optional = true }
ta = { version = "0.5", optional = true }
plotters = { version = "0.3", optional = true }
```

**사용**:
```rust
#[cfg(feature = "ml")]
pub mod ml;

#[cfg(feature = "plotting")]
pub mod visualization;
```

---

## 6. 테스트 구조

### 6.1 테스트 계층 구조 🟡 중간

**제안**: 명확한 테스트 계층

```
tests/
├── unit/              # 단위 테스트 (각 크레이트)
│   ├── domain/
│   ├── services/
│   └── repositories/
│
├── integration/       # 통합 테스트
│   ├── api/          # API 엔드포인트
│   ├── database/     # DB 연동
│   └── exchange/     # 거래소 연동
│
├── e2e/              # End-to-End 테스트
│   ├── backtest/
│   └── trading/
│
└── fixtures/         # 테스트 데이터
    ├── klines.json
    ├── orders.json
    └── strategies.toml
```

---

### 6.2 Test Double 패턴 🟡 중간

**제안**: Mock, Stub, Fake 구현

```rust
// crates/trader-core/src/testing/mod.rs

/// 테스트용 Mock Exchange
pub struct MockExchange {
    orders: Arc<RwLock<Vec<Order>>>,
    balances: Arc<RwLock<HashMap<String, Decimal>>>,
}

#[async_trait]
impl ExchangePort for MockExchange {
    async fn place_order(&self, order: &Order) -> Result<OrderId> {
        let mut orders = self.orders.write().await;
        orders.push(order.clone());
        Ok(OrderId::new())
    }
    
    async fn get_balance(&self, asset: &str) -> Result<Decimal> {
        let balances = self.balances.read().await;
        Ok(balances.get(asset).copied().unwrap_or(Decimal::ZERO))
    }
}

/// 테스트용 Fake Repository (In-Memory)
pub struct FakeOrderRepository {
    orders: Arc<RwLock<HashMap<Uuid, Order>>>,
}

#[async_trait]
impl OrderRepository for FakeOrderRepository {
    async fn save(&self, order: &Order) -> Result<()> {
        let mut orders = self.orders.write().await;
        orders.insert(order.id, order.clone());
        Ok(())
    }
    
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Order>> {
        let orders = self.orders.read().await;
        Ok(orders.get(&id).cloned())
    }
}
```

**사용**:
```rust
#[tokio::test]
async fn test_order_service() {
    // Arrange
    let exchange = Arc::new(MockExchange::new());
    let repo = Arc::new(FakeOrderRepository::new());
    let service = OrderService::new(exchange, repo);
    
    // Act
    let order = Order::new(/* ... */);
    let result = service.place_order(order).await;
    
    // Assert
    assert!(result.is_ok());
}
```

---

### 6.3 테스트 헬퍼 및 Fixture 🟢 낮음

**제안**: 테스트 데이터 생성 헬퍼

```rust
// crates/trader-core/src/testing/builders.rs

pub struct OrderBuilder {
    order: Order,
}

impl OrderBuilder {
    pub fn new() -> Self {
        Self {
            order: Order {
                id: Uuid::new_v4(),
                symbol: "BTC/USDT".to_string(),
                side: Side::Buy,
                order_type: OrderType::Limit,
                quantity: dec!(1.0),
                price: dec!(50000),
                status: OrderStatus::new(OrderStatusType::New),
                // ...
            }
        }
    }
    
    pub fn with_symbol(mut self, symbol: &str) -> Self {
        self.order.symbol = symbol.to_string();
        self
    }
    
    pub fn with_quantity(mut self, quantity: Decimal) -> Self {
        self.order.quantity = quantity;
        self
    }
    
    pub fn build(self) -> Order {
        self.order
    }
}

// 사용
let order = OrderBuilder::new()
    .with_symbol("ETH/USDT")
    .with_quantity(dec!(10.0))
    .build();
```

---

## 7. 우선순위 요약

### 🔴 높은 우선순위 (즉시 고려)

| 항목 | 효과 | 난이도 | 시간 |
|------|------|--------|------|
| 에러 처리 통합 | 안정성 대폭 향상 | 중 | 1주 |
| Repository 패턴 완성 | 테스트 용이성 ↑ | 중 | 1주 |
| API 레이어 얇게 만들기 | 유지보수성 ↑ | 중 | 2주 |

**총 시간**: 4주

---

### 🟡 중간 우선순위 (계획 수립)

| 항목 | 효과 | 난이도 | 시간 |
|------|------|--------|------|
| Domain Service 레이어 | 코드 재사용성 ↑ | 중 | 2주 |
| 의존성 역전 (DIP) | 유연성 ↑ | 중 | 1-2주 |
| Hexagonal Architecture | 명확한 경계 | 높 | 3-4주 |
| 테스트 구조 개선 | 테스트 품질 ↑ | 중 | 1-2주 |

**총 시간**: 7-10주

---

### 🟢 낮은 우선순위 (여유 시)

| 항목 | 효과 | 난이도 | 시간 |
|------|------|--------|------|
| Use Case 레이어 | Clean Architecture | 중 | 2주 |
| 이벤트 시스템 | 디커플링 | 중 | 1-2주 |
| Factory 패턴 강화 | 미미 | 하 | 1주 |
| Builder 패턴 | 편의성 | 하 | 1주 |
| Feature Flags | 컴파일 시간 | 하 | 1주 |

---

## 실용적인 접근법

### Phase 1: 기반 다지기 (4주)

**목표**: 안정성 및 테스트 가능성 확보

```
Week 1-2: 에러 처리 통합
  - AppError 계층 구조 설계
  - 각 크레이트 에러 변환
  - HTTP 응답 매핑

Week 3-4: Repository 패턴 완성
  - 인터페이스 정의 (OrderRepository 등)
  - PostgreSQL 구현
  - In-Memory 테스트 구현
```

**효과**:
- 에러 처리 일관성
- 테스트 작성 용이
- 데이터 소스 교체 가능

---

### Phase 2: 레이어 정리 (2-3주)

**목표**: 명확한 책임 분리

```
Week 1: Domain Service 추출
  - PortfolioService
  - OrderService
  - 비즈니스 로직 이동

Week 2-3: API 레이어 리팩토링
  - 핸들러를 얇게 (5-10줄)
  - Use Case로 위임
  - 중복 코드 제거
```

**효과**:
- 비즈니스 로직 재사용
- API 핸들러 단순화
- CLI에서도 동일 로직 사용 가능

---

### Phase 3: 아키텍처 개선 (3-4주)

**목표**: 유연하고 확장 가능한 구조

```
Week 1-2: 의존성 역전
  - Port 인터페이스 정의
  - Adapter 구현 분리
  - DI Container 구축

Week 3-4: 테스트 인프라
  - Mock/Stub/Fake 구현
  - 테스트 헬퍼
  - Fixture 정리
```

**효과**:
- 구현체 교체 용이
- 통합 테스트 속도 향상
- 외부 의존성 제거 (테스트)

---

### Phase 4: 고급 기능 (선택적)

```
- Use Case 레이어
- 이벤트 시스템
- Hexagonal Architecture 완성
```

---

## 마이그레이션 전략

### 1. Strangler Fig 패턴 적용

**점진적 마이그레이션**:
```
1. 새 구조로 신규 기능 개발
2. 기존 코드 점진적 이전
3. 레거시 코드 단계적 제거
```

**예시**:
```
// 1. 새 구조로 시작
crates/trader-application/
  └── use_cases/
      └── create_order_v2.rs  (새 구조)

// 2. 병행 운영
crates/trader-api/src/routes/
  ├── orders.rs              (기존)
  └── orders_v2.rs           (새 구조)

// 3. 기존 코드 이전
GET /api/v1/orders      → 기존 구조
POST /api/v2/orders     → 새 구조

// 4. 완전 전환 후 기존 제거
```

---

### 2. 점진적 리팩토링 체크리스트

```
□ 1. 테스트 작성 (기존 동작 보호)
□ 2. 인터페이스 추출
□ 3. 새 구현 작성
□ 4. 기존 코드를 새 인터페이스 호출로 변경
□ 5. 테스트 실행 (회귀 방지)
□ 6. 기존 구현 제거
□ 7. 정리 및 문서화
```

---

## 측정 지표

### 1. 코드 품질 지표

```bash
# 복잡도 측정
cargo install cargo-complexity
cargo complexity

# 의존성 그래프
cargo install cargo-deps
cargo deps --all-deps | dot -Tpng > deps.png

# 코드 중복
cargo install cargo-clippy
cargo clippy -- -W clippy::all
```

---

### 2. 아키텍처 지표

**측정 항목**:
- 크레이트 간 의존성 수
- 순환 의존성 존재 여부
- 평균 함수 길이
- 평균 파일 크기

**목표**:
- 순환 의존성: 0
- 함수 길이: < 50줄
- 파일 크기: < 500줄
- 테스트 커버리지: > 70%

---

## 결론

### 핵심 요약

1. **에러 처리**: 통합 계층 구조로 일관성 확보
2. **Repository 패턴**: 데이터 접근 추상화로 테스트 용이
3. **레이어 분리**: API 레이어 얇게, 비즈니스 로직 분리
4. **의존성 역전**: 인터페이스 의존으로 유연성 확보

### 실행 순서

```
Phase 1 (4주): 에러 처리 + Repository
  → 즉시 가치 제공

Phase 2 (2-3주): Domain Service + API 리팩토링
  → 점진적 개선

Phase 3 (3-4주): DIP + 테스트 인프라
  → 장기적 유연성

Phase 4 (선택): 고급 패턴
  → 필요시 적용
```

### 주의사항

⚠️ **구조 개선 전에**:
1. 현재 동작을 보호하는 테스트 작성
2. 작은 단위로 점진적 변경
3. 각 단계마다 테스트 실행
4. 문서화 병행

**"Good architecture is more about partitioning than construction"**

명확한 경계와 책임 분리가 핵심입니다! 🚀

---

*작성일: 2026-01-30*
*작성자: GitHub Copilot Agent*
