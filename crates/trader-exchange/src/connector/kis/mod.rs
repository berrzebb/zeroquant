//! 한국투자증권 (KIS) 거래소 연동 모듈.
//!
//! 이 모듈은 한국투자증권 API와의 연동을 제공하여
//! 국내 주식/ETF 및 해외 주식/ETF 거래를 지원합니다.
//!
//! # 기능
//!
//! - OAuth 2.0 인증 및 자동 토큰 갱신
//! - 국내 주식/ETF 거래
//! - KIS를 통한 해외 주식/ETF 거래
//! - WebSocket을 통한 실시간 시세 수신
//! - 모의투자 지원
//!
//! # API 문서
//!
//! 공식 API 문서: <https://apiportal.koreainvestment.com/>
//!
//! # 사용 예제
//!
//! ```rust,ignore
//! use trader_exchange::connector::kis::{KisConfig, KisEnvironment, KisOAuth};
//!
//! // 설정 생성
//! let config = KisConfig::new(
//!     "your_app_key".to_string(),
//!     "your_app_secret".to_string(),
//!     "12345678-01".to_string(),
//! )
//! .with_environment(KisEnvironment::Paper);
//!
//! // OAuth 관리자 생성
//! let oauth = KisOAuth::new(config)?;
//!
//! // 접근 토큰 획득
//! let token = oauth.get_token().await?;
//! println!("Token: {}", token.access_token);
//! ```

pub mod auth;
pub mod client;
pub mod client_kr;
pub mod client_us;
pub mod config;
pub mod holiday;
pub mod websocket_kr;
pub mod websocket_us;

pub use auth::{KisOAuth, TokenState};
pub use client::KisClient;
// connector 내부 타입 (crate 외부로 노출하지 않음)
pub(crate) use client_kr::{KrMinuteOhlcv, KrOhlcv};
pub(crate) use client_us::{KisUsClient, UsOhlcv};
pub use config::{KisAccountType, KisConfig, KisEnvironment};
pub use holiday::{HolidayChecker, MarketStatus};
pub use websocket_kr::{
    KisKrWebSocket, KrRealtimeMessage, KrRealtimeOrderbook, KrRealtimeTrade, WsCommand,
};
pub use websocket_us::{
    KisUsWebSocket, UsRealtimeMessage, UsRealtimeOrderbook, UsRealtimeTrade, UsWsCommand,
};

/// KIS 거래 ID (tr_id) 상수 모음.
///
/// 거래 ID는 모든 API 호출에서 작업 유형을 식별하기 위해 필요합니다.
pub mod tr_id {
    // ========================================
    // Korean Domestic Stock (국내 주식)
    // ========================================

    /// 국내 주식 현재가 조회 (실전)
    pub const KR_PRICE_REAL: &str = "FHKST01010100";
    /// 국내 주식 현재가 조회 (모의)
    pub const KR_PRICE_PAPER: &str = "FHKST01010100";

    /// 국내 주식 호가 조회 (실전)
    pub const KR_ORDERBOOK_REAL: &str = "FHKST01010200";
    /// 국내 주식 호가 조회 (모의)
    pub const KR_ORDERBOOK_PAPER: &str = "FHKST01010200";

    /// 국내 주식 체결 조회 (실전)
    pub const KR_TRADES_REAL: &str = "FHKST01010300";
    /// 국내 주식 체결 조회 (모의)
    pub const KR_TRADES_PAPER: &str = "FHKST01010300";

    /// 국내 주식 일/주/월/년 시세 (실전)
    pub const KR_DAILY_PRICE_REAL: &str = "FHKST01010400";
    /// 국내 주식 일/주/월/년 시세 (모의)
    pub const KR_DAILY_PRICE_PAPER: &str = "FHKST01010400";

    /// 국내 주식 분봉 조회 (실전)
    pub const KR_MINUTE_PRICE_REAL: &str = "FHKST03010100";
    /// 국내 주식 분봉 조회 (모의)
    pub const KR_MINUTE_PRICE_PAPER: &str = "FHKST03010100";

    /// 국내 주식 현금 매수 (실전)
    pub const KR_BUY_REAL: &str = "TTTC0802U";
    /// 국내 주식 현금 매수 (모의)
    pub const KR_BUY_PAPER: &str = "VTTC0802U";

    /// 국내 주식 현금 매도 (실전)
    pub const KR_SELL_REAL: &str = "TTTC0801U";
    /// 국내 주식 현금 매도 (모의)
    pub const KR_SELL_PAPER: &str = "VTTC0801U";

    /// 국내 주식 주문 정정 (실전)
    pub const KR_MODIFY_REAL: &str = "TTTC0803U";
    /// 국내 주식 주문 정정 (모의)
    pub const KR_MODIFY_PAPER: &str = "VTTC0803U";

    /// 국내 주식 주문 취소 (실전)
    pub const KR_CANCEL_REAL: &str = "TTTC0803U";
    /// 국내 주식 주문 취소 (모의)
    pub const KR_CANCEL_PAPER: &str = "VTTC0803U";

    /// 국내 주식 잔고 조회 (실전)
    pub const KR_BALANCE_REAL: &str = "TTTC8434R";
    /// 국내 주식 잔고 조회 (모의)
    pub const KR_BALANCE_PAPER: &str = "VTTC8434R";
    /// 국내 주식 잔고 조회 (실전 - ISA/중개형ISA 계좌)
    pub const KR_BALANCE_ISA_REAL: &str = "CTSC9015R";

    /// 국내 주식 매수 가능 조회 (실전)
    pub const KR_BUYABLE_REAL: &str = "TTTC8908R";
    /// 국내 주식 매수 가능 조회 (모의)
    pub const KR_BUYABLE_PAPER: &str = "VTTC8908R";

    /// 국내 주식 일별 주문체결 조회 (실전 - 일반계좌, 3개월 이내)
    pub const KR_ORDER_HISTORY_REAL: &str = "TTTC0081R";
    /// 국내 주식 일별 주문체결 조회 (모의)
    pub const KR_ORDER_HISTORY_PAPER: &str = "VTTC8001R";
    /// 국내 주식 일별 주문체결 조회 (실전 - ISA/연금저축 등 특수계좌, 1년 이내)
    pub const KR_ORDER_HISTORY_ISA_REAL: &str = "CTSC9115R";

    // ========================================
    // US Stock (해외 주식 - 미국)
    // ========================================

    /// 해외 주식 현재가 상세 (실전)
    pub const US_PRICE_DETAIL_REAL: &str = "HHDFS76200200";
    /// 해외 주식 현재가 상세 (모의)
    pub const US_PRICE_DETAIL_PAPER: &str = "HHDFS76200200";

    /// 해외 주식 현재가 (실전)
    pub const US_PRICE_REAL: &str = "HHDFS00000300";
    /// 해외 주식 현재가 (모의)
    pub const US_PRICE_PAPER: &str = "HHDFS00000300";

    /// 해외 주식 기간별 시세 (실전)
    pub const US_DAILY_PRICE_REAL: &str = "HHDFS76240000";
    /// 해외 주식 기간별 시세 (모의)
    pub const US_DAILY_PRICE_PAPER: &str = "HHDFS76240000";

    /// 해외 주식 매수 (실전)
    pub const US_BUY_REAL: &str = "JTTT1002U";
    /// 해외 주식 매수 (모의)
    pub const US_BUY_PAPER: &str = "VTTT1002U";

    /// 해외 주식 매도 (실전)
    pub const US_SELL_REAL: &str = "JTTT1006U";
    /// 해외 주식 매도 (모의)
    pub const US_SELL_PAPER: &str = "VTTT1006U";

    /// 해외 주식 주문 정정 (실전)
    pub const US_MODIFY_REAL: &str = "JTTT1004U";
    /// 해외 주식 주문 정정 (모의)
    pub const US_MODIFY_PAPER: &str = "VTTT1004U";

    /// 해외 주식 주문 취소 (실전)
    pub const US_CANCEL_REAL: &str = "JTTT1004U";
    /// 해외 주식 주문 취소 (모의)
    pub const US_CANCEL_PAPER: &str = "VTTT1004U";

    /// 해외 주식 잔고 (실전)
    pub const US_BALANCE_REAL: &str = "JTTT3012R";
    /// 해외 주식 잔고 (모의)
    pub const US_BALANCE_PAPER: &str = "VTTS3012R";

    /// 해외 주식 주야간 구분 (실전)
    pub const US_DAY_NIGHT_REAL: &str = "JTTT3010R";
    /// 해외 주식 주야간 구분 (모의)
    pub const US_DAY_NIGHT_PAPER: &str = "JTTT3010R";

    /// 해외 주식 미체결 주문 조회 (실전)
    pub const US_PENDING_ORDERS_REAL: &str = "TTTT3039R";
    /// 해외 주식 미체결 주문 조회 (모의)
    pub const US_PENDING_ORDERS_PAPER: &str = "VTTT3039R";

    /// 해외 주식 체결 내역 조회 (실전)
    pub const US_ORDER_EXECUTION_REAL: &str = "TTTS3035R";
    /// 해외 주식 체결 내역 조회 (모의)
    pub const US_ORDER_EXECUTION_PAPER: &str = "VTTS3035R";

    // ========================================
    // WebSocket Real-time (실시간 시세)
    // ========================================

    /// 국내 주식 실시간 체결가
    pub const WS_KR_TRADE: &str = "H0STCNT0";
    /// 국내 주식 실시간 호가
    pub const WS_KR_ORDERBOOK: &str = "H0STASP0";
    /// 해외 주식 실시간 체결
    pub const WS_US_TRADE: &str = "HDFSCNT0";
    /// 해외 주식 실시간 호가
    pub const WS_US_ORDERBOOK: &str = "HDFSASP0";
}

/// KIS API에서 사용하는 거래소 코드.
pub mod exchange_code {
    /// 미국 NYSE
    pub const NYSE: &str = "NYS";
    /// 미국 NASDAQ
    pub const NASDAQ: &str = "NAS";
    /// 미국 AMEX
    pub const AMEX: &str = "AMS";
    /// 한국 KRX (KOSPI + KOSDAQ)
    pub const KRX: &str = "KRX";
}

/// KIS API 주문 유형.
pub mod order_type {
    /// 지정가 (Limit order)
    pub const LIMIT: &str = "00";
    /// 시장가 (Market order)
    pub const MARKET: &str = "01";
    /// 조건부 지정가
    pub const CONDITIONAL_LIMIT: &str = "02";
    /// 최유리 지정가
    pub const BEST_LIMIT: &str = "03";
    /// 최우선 지정가
    pub const PRIORITY_LIMIT: &str = "04";
    /// 장전 시간외 (Pre-market)
    pub const PRE_MARKET: &str = "05";
    /// 장후 시간외 (After-hours)
    pub const AFTER_HOURS: &str = "06";
    /// 시간외 단일가
    pub const SINGLE_PRICE: &str = "07";
    /// 자기주식
    pub const TREASURY: &str = "08";
    /// 자기주식 S-Option
    pub const TREASURY_S_OPTION: &str = "09";
    /// 자기주식 금전신탁
    pub const TREASURY_TRUST: &str = "10";
    /// IOC 지정가 (Immediate-or-Cancel)
    pub const IOC_LIMIT: &str = "11";
    /// FOK 지정가 (Fill-or-Kill)
    pub const FOK_LIMIT: &str = "12";
    /// IOC 시장가
    pub const IOC_MARKET: &str = "13";
    /// FOK 시장가
    pub const FOK_MARKET: &str = "14";
    /// IOC 최유리
    pub const IOC_BEST: &str = "15";
    /// FOK 최유리
    pub const FOK_BEST: &str = "16";
}
