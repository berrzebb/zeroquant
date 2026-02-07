//! 전략 타입 정의.
//!
//! 지원하는 모든 전략을 enum으로 정의하여 타입 안전성을 보장합니다.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 지원하는 전략 타입.
///
/// 모든 내장 전략을 열거형으로 정의합니다.
/// 문자열 변환을 지원하여 API 요청/응답에서 사용할 수 있습니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyType {
    // ==================== 단일 종목 전략 ====================
    /// RSI 평균회귀
    #[serde(alias = "rsi_mean_reversion")]
    Rsi,
    /// 그리드 트레이딩
    #[serde(alias = "grid_trading")]
    Grid,
    /// 볼린저 밴드
    #[serde(alias = "bollinger_bands")]
    Bollinger,
    /// 변동성 돌파
    #[serde(alias = "volatility")]
    VolatilityBreakout,
    /// Magic Split
    #[serde(alias = "split")]
    MagicSplit,
    /// 이동평균 크로스오버
    #[serde(alias = "sma_crossover", alias = "ma_crossover")]
    Sma,
    /// 캔들 패턴
    CandlePattern,
    /// 무한매수
    InfinityBot,
    /// 거래량 급증
    MarketInterestDay,
    /// 박스권 매매
    RangeTrading,
    /// 섹터 VB
    #[serde(alias = "sector_volatility")]
    SectorVb,

    // ==================== 자산배분 전략 ====================
    /// 복합 모멘텀
    CompoundMomentum,
    /// HAA (Hierarchical Asset Allocation)
    Haa,
    /// XAA (Expanded Asset Allocation)
    Xaa,
    /// BAA (Bold Asset Allocation)
    Baa,
    /// All Weather
    AllWeather,
    /// 모멘텀 파워
    MomentumPower,
    /// 종목 갈아타기
    #[serde(alias = "rotation")]
    StockRotation,
    /// 시총 상위
    MarketCapTop,
    /// 미국 3배 레버리지
    #[serde(alias = "us_leverage")]
    Us3xLeverage,
    /// 섹터 모멘텀
    SectorMomentum,
    /// 듀얼 모멘텀
    DualMomentum,
    /// 소형주 퀀트
    SmallCapQuant,
    /// 연금 봇
    #[serde(alias = "pension")]
    PensionBot,

    // ==================== 양방향/급등 전략 ====================
    /// 시장 양방향
    MarketBothSide,
    /// 모멘텀 급등
    MomentumSurge,
}

impl StrategyType {
    /// 모든 전략 타입 목록.
    pub fn all() -> &'static [StrategyType] {
        use StrategyType::*;
        &[
            // 단일 종목
            Rsi,
            Grid,
            Bollinger,
            VolatilityBreakout,
            MagicSplit,
            Sma,
            CandlePattern,
            InfinityBot,
            MarketInterestDay,
            RangeTrading,
            SectorVb,
            // 자산배분
            CompoundMomentum,
            Haa,
            Xaa,
            Baa,
            AllWeather,
            MomentumPower,
            StockRotation,
            MarketCapTop,
            Us3xLeverage,
            SectorMomentum,
            DualMomentum,
            SmallCapQuant,
            PensionBot,
            // 양방향/급등
            MarketBothSide,
            MomentumSurge,
        ]
    }

    /// 단일 종목 전략 여부.
    pub fn is_single_asset(&self) -> bool {
        use StrategyType::*;
        matches!(
            self,
            Rsi | Grid
                | Bollinger
                | VolatilityBreakout
                | MagicSplit
                | Sma
                | CandlePattern
                | InfinityBot
                | MarketInterestDay
                | RangeTrading
                | SectorVb
        )
    }

    /// 자산배분 전략 여부.
    pub fn is_asset_allocation(&self) -> bool {
        use StrategyType::*;
        matches!(
            self,
            CompoundMomentum
                | Haa
                | Xaa
                | Baa
                | AllWeather
                | MomentumPower
                | StockRotation
                | MarketCapTop
                | Us3xLeverage
                | SectorMomentum
                | DualMomentum
                | SmallCapQuant
                | PensionBot
        )
    }

    /// 양방향/급등 전략 여부.
    pub fn is_momentum_strategy(&self) -> bool {
        use StrategyType::*;
        matches!(self, MarketBothSide | MomentumSurge)
    }

    /// 전략의 기본 이름.
    pub fn display_name(&self) -> &'static str {
        use StrategyType::*;
        match self {
            Rsi => "RSI 평균회귀",
            Grid => "그리드 트레이딩",
            Bollinger => "볼린저 밴드",
            VolatilityBreakout => "변동성 돌파",
            MagicSplit => "Magic Split",
            Sma => "이동평균 크로스오버",
            CandlePattern => "캔들 패턴",
            InfinityBot => "무한매수",
            MarketInterestDay => "거래량 급증",
            RangeTrading => "박스권 매매",
            SectorVb => "섹터 VB",
            CompoundMomentum => "복합 모멘텀",
            Haa => "HAA",
            Xaa => "XAA",
            Baa => "BAA",
            AllWeather => "올웨더",
            MomentumPower => "모멘텀 파워",
            StockRotation => "종목 갈아타기",
            MarketCapTop => "시총 상위",
            Us3xLeverage => "미국 3배 레버리지",
            SectorMomentum => "섹터 모멘텀",
            DualMomentum => "듀얼 모멘텀",
            SmallCapQuant => "소형주 퀀트",
            PensionBot => "연금 봇",
            MarketBothSide => "시장 양방향",
            MomentumSurge => "모멘텀 급등",
        }
    }

    /// API 식별자 (snake_case).
    pub fn api_id(&self) -> &'static str {
        use StrategyType::*;
        match self {
            Rsi => "rsi",
            Grid => "grid",
            Bollinger => "bollinger",
            VolatilityBreakout => "volatility_breakout",
            MagicSplit => "magic_split",
            Sma => "sma",
            CandlePattern => "candle_pattern",
            InfinityBot => "infinity_bot",
            MarketInterestDay => "market_interest_day",
            RangeTrading => "range_trading",
            SectorVb => "sector_vb",
            CompoundMomentum => "compound_momentum",
            Haa => "haa",
            Xaa => "xaa",
            Baa => "baa",
            AllWeather => "all_weather",
            MomentumPower => "momentum_power",
            StockRotation => "stock_rotation",
            MarketCapTop => "market_cap_top",
            Us3xLeverage => "us_3x_leverage",
            SectorMomentum => "sector_momentum",
            DualMomentum => "dual_momentum",
            SmallCapQuant => "small_cap_quant",
            PensionBot => "pension_bot",
            MarketBothSide => "market_both_side",
            MomentumSurge => "momentum_surge",
        }
    }
}

impl fmt::Display for StrategyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.api_id())
    }
}

/// 문자열 파싱 에러.
#[derive(Debug, Clone)]
pub struct ParseStrategyTypeError {
    pub input: String,
}

impl fmt::Display for ParseStrategyTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown strategy type: {}", self.input)
    }
}

impl std::error::Error for ParseStrategyTypeError {}

impl FromStr for StrategyType {
    type Err = ParseStrategyTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use StrategyType::*;

        match s.to_lowercase().as_str() {
            // 단일 종목 전략
            "rsi" | "rsi_mean_reversion" => Ok(Rsi),
            "grid" | "grid_trading" => Ok(Grid),
            "bollinger" | "bollinger_bands" => Ok(Bollinger),
            "volatility_breakout" | "volatility" => Ok(VolatilityBreakout),
            "magic_split" | "split" => Ok(MagicSplit),
            "sma" | "sma_crossover" | "ma_crossover" => Ok(Sma),
            "candle_pattern" => Ok(CandlePattern),
            "infinity_bot" => Ok(InfinityBot),
            "market_interest_day" => Ok(MarketInterestDay),
            "range_trading" => Ok(RangeTrading),
            "sector_vb" | "sector_volatility" => Ok(SectorVb),
            // 자산배분 전략
            "compound_momentum" => Ok(CompoundMomentum),
            "haa" => Ok(Haa),
            "xaa" => Ok(Xaa),
            "baa" => Ok(Baa),
            "all_weather" => Ok(AllWeather),
            "momentum_power" => Ok(MomentumPower),
            "stock_rotation" | "rotation" => Ok(StockRotation),
            "market_cap_top" => Ok(MarketCapTop),
            "us_3x_leverage" | "us_leverage" => Ok(Us3xLeverage),
            "sector_momentum" => Ok(SectorMomentum),
            "dual_momentum" => Ok(DualMomentum),
            "small_cap_quant" => Ok(SmallCapQuant),
            "pension_bot" | "pension" => Ok(PensionBot),
            // 양방향/급등 전략
            "market_both_side" => Ok(MarketBothSide),
            "momentum_surge" => Ok(MomentumSurge),
            _ => Err(ParseStrategyTypeError {
                input: s.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_strategy_type() {
        assert_eq!("rsi".parse::<StrategyType>().unwrap(), StrategyType::Rsi);
        assert_eq!(
            "grid_trading".parse::<StrategyType>().unwrap(),
            StrategyType::Grid
        );
        assert_eq!("haa".parse::<StrategyType>().unwrap(), StrategyType::Haa);
    }

    #[test]
    fn test_parse_with_aliases() {
        assert_eq!(
            "rsi_mean_reversion".parse::<StrategyType>().unwrap(),
            StrategyType::Rsi
        );
        assert_eq!(
            "volatility".parse::<StrategyType>().unwrap(),
            StrategyType::VolatilityBreakout
        );
        assert_eq!(
            "range_trading".parse::<StrategyType>().unwrap(),
            StrategyType::RangeTrading
        );
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!("RSI".parse::<StrategyType>().unwrap(), StrategyType::Rsi);
        assert_eq!("Grid".parse::<StrategyType>().unwrap(), StrategyType::Grid);
    }

    #[test]
    fn test_parse_unknown() {
        assert!("unknown_strategy".parse::<StrategyType>().is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(StrategyType::Rsi.to_string(), "rsi");
        assert_eq!(StrategyType::Grid.to_string(), "grid");
    }

    #[test]
    fn test_is_single_asset() {
        assert!(StrategyType::Rsi.is_single_asset());
        assert!(StrategyType::Grid.is_single_asset());
        assert!(!StrategyType::Haa.is_single_asset());
    }

    #[test]
    fn test_is_asset_allocation() {
        assert!(!StrategyType::Rsi.is_asset_allocation());
        assert!(StrategyType::Haa.is_asset_allocation());
        assert!(StrategyType::CompoundMomentum.is_asset_allocation());
    }

    #[test]
    fn test_all_strategies_count() {
        assert_eq!(StrategyType::all().len(), 26);
    }

    #[test]
    fn test_serde_roundtrip() {
        let st = StrategyType::Rsi;
        let json = serde_json::to_string(&st).unwrap();
        let parsed: StrategyType = serde_json::from_str(&json).unwrap();
        assert_eq!(st, parsed);
    }
}
