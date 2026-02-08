//! ExchangeProvider 및 MarketDataProvider 구현체.
//!
//! 거래소 중립적인 ExchangeProvider, MarketDataProvider trait의 구현체들을 제공합니다.
//!
//! # Provider 구조
//!
//! 모든 거래소는 `XXXExchangeProvider` 패턴을 따릅니다:
//! - [`KisExchangeProvider`]: KIS 국내/해외/ISA 계좌 통합 Provider
//! - [`BinanceProvider`]: Binance 거래소 Provider
//! - [`MockExchangeProvider`]: 테스트/시뮬레이션용 Mock Provider

mod binance;
mod kis;
// mod mock;

pub use binance::{BinanceExchangeProvider, BinanceProvider};
pub use kis::{KisExchangeProvider, KisProvider};
// pub use mock::{MockConfig, MockExchangeProvider, MockMarketStream};
