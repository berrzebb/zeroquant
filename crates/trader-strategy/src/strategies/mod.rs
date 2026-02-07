//! 내장 트레이딩 전략.
//!
//! 이 모듈은 검증된 여러 트레이딩 전략을 제공합니다:
//!
//! ## 그룹 전략 (통합된 베이스 전략)
//!
//! - **AssetAllocation**: HAA, XAA, BAA, All Weather, Dual Momentum 통합
//! - **MeanReversion**: RSI, Bollinger 평균회귀 전략
//! - **DCA**: Grid Trading, Magic Split, Infinity Bot 통합 (스프레드 기반 분할매수)
//! - **Rotation**: Sector Momentum, Stock Rotation, Market Cap Top 통합
//! - **DayTrading**: Volatility Breakout, SMA Crossover, Volume Surge 통합
//! - **ScreeningBased**: SmallCapQuant, PensionBot, DynamicUniverse 통합 (스크리닝 기반 동적 유니버스)
//!
//! ## 독립 전략
//!
//! - **Candle Pattern**: 35가지 캔들스틱 패턴 인식.
//! - **Range Trading**: 구간분할 장기 투자 전략.
//! - **Sector VB**: 섹터 ETF 변동성 돌파 전략.
//! - **Compound Momentum**: MA130 필터를 적용한 TQQQ/SCHD/PFIX/TMF 모멘텀 자산 배분.
//! - **Momentum Power**: TIP 기반 이동평균 모멘텀 전략 (US/KR).
//! - **Small Cap Quant**: 코스닥 소형주 퀀트 전략.
//! - **Pension Bot**: 연금 자동화 정적+동적 자산배분.
//! - **US 3X Leverage**: 미국 3배 레버리지/인버스 ETF 조합 전략.
//! - **RSI Multi TF**: RSI 다중 타임프레임 전략.
//!
//! ## 한국 지수 전략
//!
//! - **Market Both Side**: 코스피 레버리지/인버스 양방향 매매 전략.
//! - **Momentum Surge**: 급등 모멘텀 포착 전략.
//!
//! ## 공통 유틸리티
//!
//! `common` 서브모듈은 재사용 가능한 컴포넌트를 제공합니다:
//! - **Momentum Calculator**: 자산 배분을 위한 다기간 모멘텀 스코어링

// 그룹 전략 (통합)
pub mod asset_allocation;
pub mod day_trading;
pub mod dca;
pub mod mean_reversion;
pub mod rotation;
pub mod screening_based;

// 공통 유틸리티
pub mod common;

// 독립 전략
pub mod candle_pattern;
pub mod compound_momentum;
pub mod market_bothside;
pub mod momentum_power;
pub mod momentum_surge;
pub mod pension_bot;
pub mod range_trading;
pub mod rsi_multi_tf;
pub mod sector_vb;
pub mod small_cap_quant;
pub mod us_3x_leverage;

// 그룹 전략 re-exports
pub use asset_allocation::{
    AssetAllocationConfig, AssetAllocationStrategy, AssetCategory, AssetDefinition, MomentumMethod,
    PortfolioMode, StrategyVariant as AssetAllocationVariant,
};
pub use day_trading::{
    BreakoutConfig, CrossoverConfig, DayTradingConfig, DayTradingStrategy, DayTradingVariant,
    ExitConfig as DayTradingExitConfig, VolumeSurgeConfig,
};
pub use dca::{
    DcaConfig, DcaStrategy, DcaVariant, GridTradingConfig, InfinityBotConfig, MagicSplitConfig,
    SplitLevel,
};
pub use mean_reversion::{
    BollingerConfig, MeanReversionConfig, MeanReversionStrategy, MeanReversionVariant, RsiConfig,
};
pub use rotation::{
    AssetInfo as RotationAssetInfo, MarketType as RotationMarketType, RankingMetric,
    RebalanceFrequency, RotationConfig, RotationStrategy, RotationVariant,
    WeightingMethod as RotationWeightingMethod,
};
pub use screening_based::{
    ScreeningBasedConfig, ScreeningBasedStrategy, ScreeningVariant,
    WeightingMethod as ScreeningWeightingMethod,
};

// 공통 모듈 re-exports
pub use common::*;

// 독립 전략 re-exports
pub use candle_pattern::*;
pub use compound_momentum::*;
pub use market_bothside::*;
pub use momentum_power::*;
pub use momentum_surge::*;
pub use pension_bot::*;
pub use range_trading::*;
pub use rsi_multi_tf::*;
pub use sector_vb::*;
pub use small_cap_quant::*;
pub use us_3x_leverage::*;
