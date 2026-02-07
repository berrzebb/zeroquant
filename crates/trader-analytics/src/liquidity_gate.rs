//! 유동성 게이트 (Liquidity Gate).
//!
//! 시장별 최소 거래대금 기준을 정의하고 검증합니다.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_core::types::MarketType;

/// 유동성 게이트 설정.
///
/// 시장별 최소 거래대금 기준과 완화 기준을 정의합니다.
#[derive(Debug, Clone)]
pub struct LiquidityGate {
    /// 시장 유형
    pub market_type: MarketType,

    /// 최소 거래대금 (기본 기준)
    pub min_volume_amount: Decimal,

    /// 완화 기준 거래대금 (후보 부족 시)
    pub relaxed_min: Decimal,
}

impl LiquidityGate {
    /// 시장별 기본 게이트 생성.
    ///
    /// MarketType만으로는 국가를 구분할 수 없으므로,
    /// 보수적인 기준(한국 주식 기준)을 기본값으로 사용합니다.
    /// 국가별 세분화가 필요한 경우 `for_country` 메서드를 사용하세요.
    pub fn for_market(market_type: MarketType) -> Self {
        match market_type {
            // 주식 시장 - 한국 기준 (보수적)
            MarketType::Stock | MarketType::KrStock => Self {
                market_type: MarketType::Stock,
                min_volume_amount: dec!(10_000_000_000), // 100억원
                relaxed_min: dec!(8_000_000_000),        // 80억원
            },
            // 미국 주식 (레거시 타입 지원)
            MarketType::UsStock => Self {
                market_type: MarketType::Stock,
                min_volume_amount: dec!(100_000_000), // $100M
                relaxed_min: dec!(50_000_000),        // $50M
            },
            // 인덱스 - 주식과 동일 기준
            MarketType::Index => Self {
                market_type: MarketType::Stock,
                min_volume_amount: dec!(10_000_000_000), // 100억원
                relaxed_min: dec!(8_000_000_000),        // 80억원
            },
            // 암호화폐 - USDT 기준
            MarketType::Crypto => Self {
                market_type,
                min_volume_amount: dec!(1_000_000), // $1M
                relaxed_min: dec!(500_000),         // $500K
            },
            // 외환 - 유동성이 매우 높으므로 기준 낮음
            MarketType::Forex => Self {
                market_type,
                min_volume_amount: dec!(10_000_000), // $10M
                relaxed_min: dec!(5_000_000),        // $5M
            },
            // 선물/파생상품 - 높은 레버리지로 유동성 중요
            MarketType::Futures => Self {
                market_type,
                min_volume_amount: dec!(50_000_000), // $50M
                relaxed_min: dec!(25_000_000),       // $25M
            },
        }
    }

    /// KOSPI 전용 게이트 생성.
    pub fn kospi() -> Self {
        Self {
            market_type: MarketType::Stock,
            min_volume_amount: dec!(20_000_000_000), // 200억원
            relaxed_min: dec!(15_000_000_000),       // 150억원
        }
    }

    /// KOSDAQ 전용 게이트 생성.
    pub fn kosdaq() -> Self {
        Self {
            market_type: MarketType::Stock,
            min_volume_amount: dec!(10_000_000_000), // 100억원
            relaxed_min: dec!(8_000_000_000),        // 80억원
        }
    }

    /// 거래대금이 기본 기준을 통과하는지 확인.
    ///
    /// # 인자
    ///
    /// * `volume_amount` - 평균 거래대금
    ///
    /// # 반환
    ///
    /// true면 기본 기준 통과
    pub fn passes(&self, volume_amount: Decimal) -> bool {
        volume_amount >= self.min_volume_amount
    }

    /// 거래대금이 완화 기준을 통과하는지 확인.
    ///
    /// # 인자
    ///
    /// * `volume_amount` - 평균 거래대금
    ///
    /// # 반환
    ///
    /// true면 완화 기준 통과 (후보 부족 시 허용)
    pub fn passes_relaxed(&self, volume_amount: Decimal) -> bool {
        volume_amount >= self.relaxed_min
    }

    /// 통과 여부를 단계별로 반환.
    ///
    /// # 반환
    ///
    /// - `LiquidityLevel::Pass`: 기본 기준 통과
    /// - `LiquidityLevel::Relaxed`: 완화 기준 통과
    /// - `LiquidityLevel::Fail`: 기준 미달
    pub fn check_level(&self, volume_amount: Decimal) -> LiquidityLevel {
        if self.passes(volume_amount) {
            LiquidityLevel::Pass
        } else if self.passes_relaxed(volume_amount) {
            LiquidityLevel::Relaxed
        } else {
            LiquidityLevel::Fail
        }
    }
}

/// 유동성 레벨.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityLevel {
    /// 기본 기준 통과
    Pass,
    /// 완화 기준 통과
    Relaxed,
    /// 기준 미달
    Fail,
}

impl LiquidityLevel {
    /// 통과 여부 (Pass 또는 Relaxed).
    pub fn is_pass(&self) -> bool {
        matches!(self, LiquidityLevel::Pass | LiquidityLevel::Relaxed)
    }

    /// 엄격한 통과 여부 (Pass만).
    pub fn is_strict_pass(&self) -> bool {
        matches!(self, LiquidityLevel::Pass)
    }
}

// ================================================================================================
// 테스트
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kr_stock_gate() {
        let gate = LiquidityGate::for_market(MarketType::Stock);

        // 100억원 (기본 기준)
        assert!(gate.passes(dec!(10_000_000_000)));

        // 90억원 (완화 기준)
        assert!(!gate.passes(dec!(9_000_000_000)));
        assert!(gate.passes_relaxed(dec!(9_000_000_000)));

        // 70억원 (미달)
        assert!(!gate.passes(dec!(7_000_000_000)));
        assert!(!gate.passes_relaxed(dec!(7_000_000_000)));
    }

    #[test]
    fn test_kospi_specific() {
        let gate = LiquidityGate::kospi();

        // 200억원 기준
        assert_eq!(gate.min_volume_amount, dec!(20_000_000_000));
        assert!(gate.passes(dec!(20_000_000_000)));
        assert!(!gate.passes(dec!(15_000_000_000)));
        assert!(gate.passes_relaxed(dec!(15_000_000_000)));
    }

    #[test]
    fn test_us_stock_gate() {
        let gate = LiquidityGate::for_market(MarketType::UsStock);

        // $100M (기본)
        assert!(gate.passes(dec!(100_000_000)));

        // $60M (완화)
        assert!(!gate.passes(dec!(60_000_000)));
        assert!(gate.passes_relaxed(dec!(60_000_000)));

        // $30M (미달)
        assert!(!gate.passes_relaxed(dec!(30_000_000)));
    }

    #[test]
    fn test_check_level() {
        let gate = LiquidityGate::for_market(MarketType::Stock);

        assert_eq!(gate.check_level(dec!(12_000_000_000)), LiquidityLevel::Pass);
        assert_eq!(
            gate.check_level(dec!(9_000_000_000)),
            LiquidityLevel::Relaxed
        );
        assert_eq!(gate.check_level(dec!(7_000_000_000)), LiquidityLevel::Fail);
    }

    #[test]
    fn test_liquidity_level_methods() {
        assert!(LiquidityLevel::Pass.is_pass());
        assert!(LiquidityLevel::Relaxed.is_pass());
        assert!(!LiquidityLevel::Fail.is_pass());

        assert!(LiquidityLevel::Pass.is_strict_pass());
        assert!(!LiquidityLevel::Relaxed.is_strict_pass());
        assert!(!LiquidityLevel::Fail.is_strict_pass());
    }
}
