//! 백테스팅 및 모의투자를 위한 시뮬레이션 거래소.
//!
//! 이 모듈은 다음 기능을 제공하는 시뮬레이션 거래소입니다:
//! - 파일 또는 메모리에서 과거 데이터(Kline) 로드
//! - 주문 매칭 및 체결 시뮬레이션
//! - 계정 잔고 및 포지션 추적
//! - 전략 테스트를 위한 시장 이벤트 생성
//!
//! # 예제
//!
//! ```ignore
//! use trader_exchange::simulated::{SimulatedExchange, SimulatedConfig};
//!
//! let config = SimulatedConfig::default()
//!     .with_initial_balance("USDT", dec!(10000))
//!     .with_fee_rate(dec!(0.001));
//!
//! let mut exchange = SimulatedExchange::new(config);
//! exchange.load_klines("BTCUSDT", klines).await?;
//! exchange.connect().await?;
//!
//! // 이제 실제 거래소처럼 사용할 수 있습니다
//! let ticker = exchange.get_ticker(&symbol).await?;
//! ```

mod data_feed;
mod exchange;
mod matching_engine;
mod stream;

pub use data_feed::{DataFeed, DataFeedConfig};
pub use exchange::{SimulatedConfig, SimulatedExchange};
pub use matching_engine::{FillType, MatchingEngine, OrderMatch};
pub use stream::{EventBroadcaster, SimulatedMarketStream, SimulatedUserStream};
