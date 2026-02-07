//! 데이터 Provider 모듈.
//!
//! 다양한 소스에서 데이터를 가져오는 Provider들을 정의합니다.
//!
//! ## KRX Open API
//! - `KrxApiClient`: KRX Open API 클라이언트 (인증키 필요)
//! - KOSPI/KOSDAQ 종목 기본 정보, PER/PBR, OHLCV 데이터
//! - Yahoo Finance 국내 주식 의존성 대체
//!
//! ## 네이버 금융
//! - `NaverFinanceFetcher`: 네이버 금융 크롤러
//! - 시가총액, PER/PBR, ROE, 섹터, KOSPI/KOSDAQ/ETF 구분
//! - 국내 주식 펀더멘털 데이터 수집
//!
//! ## Yahoo Finance
//! - `YahooFundamentalFetcher`: Yahoo Finance 펀더멘털 크롤러
//! - 글로벌 주식 펀더멘털 데이터 수집 (미국, 일본, 홍콩 등)
//! - PER, PBR, PSR, ROE, ROA, 배당수익률, 재무비율 등
//!
//! ## 심볼 정보 Provider
//! - `KrxSymbolProvider`: 한국거래소(KRX) 종목 정보
//! - `BinanceSymbolProvider`: Binance 암호화폐 종목 정보
//! - `YahooSymbolProvider`: Yahoo Finance 미국/글로벌 주식 정보
//! - `CompositeSymbolProvider`: 모든 Provider 통합

pub mod krx_api;
pub mod naver;
pub mod symbol_info;
pub mod yahoo_fundamental;

pub use krx_api::{KrxApiClient, KrxEtfInfo, KrxOhlcv, KrxStockInfo, KrxValuation};
pub use naver::{KrMarketType, NaverError, NaverFinanceFetcher, NaverFundamentalData};
pub use symbol_info::{
    BinanceSymbolProvider, CompositeSymbolProvider, KrxSymbolProvider, SymbolInfoProvider,
    SymbolMetadata, SymbolResolver, YahooSymbolProvider,
};
pub use yahoo_fundamental::{YahooFundamentalData, YahooFundamentalError, YahooFundamentalFetcher};
