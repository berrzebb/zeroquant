//! 향상된 트레일링 스톱 시스템.
//!
//! 이 모듈은 다양한 모드를 지원하는 고급 트레일링 스톱 기능을 제공합니다:
//! - 고정 퍼센트 트레일링
//! - ATR 기반 동적 트레일링
//! - 단계별 트레일링 (수익 구간별 다른 트레일링 %)
//! - Parabolic SAR 트레일링

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use trader_core::Side;

/// 트레일링 스톱 모드 열거형
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrailingStopMode {
    /// 고정 퍼센트 트레일링
    /// 최고가에서 고정된 퍼센트 거리만큼 스톱이 따라갑니다.
    FixedPercentage {
        /// 트레일 거리 퍼센트 (예: 2.0은 2%)
        trail_pct: Decimal,
    },

    /// ATR 기반 트레일링
    /// ATR(평균 진폭 범위)을 사용하여 스톱 거리를 계산합니다.
    AtrBased {
        /// ATR 배수 (예: 2.0은 2배 ATR)
        atr_multiplier: Decimal,
        /// 현재 ATR 값 (외부에서 업데이트 필요)
        current_atr: Decimal,
    },

    /// 단계별 트레일링
    /// 수익 구간별로 다른 트레일링 퍼센트가 적용됩니다.
    /// 수익이 커질수록 더 많은 이익을 보호하는 데 유용합니다.
    Step {
        /// 수익 구간 설정
        profit_levels: Vec<ProfitLevel>,
    },

    /// Parabolic SAR 트레일링
    /// Parabolic SAR 알고리즘을 사용한 동적 트레일링
    ParabolicSar {
        /// 가속 계수 (일반적으로 0.02로 시작)
        acceleration: Decimal,
        /// 최대 가속 계수 (일반적으로 0.2)
        maximum: Decimal,
        /// 현재 가속 계수 (내부 상태)
        current_af: Decimal,
    },
}

impl Default for TrailingStopMode {
    fn default() -> Self {
        Self::FixedPercentage {
            trail_pct: dec!(2.0),
        }
    }
}

/// 단계별 트레일링을 위한 수익 구간 설정
///
/// 설정 예시:
/// - 5% 수익 시: 2% 트레일링 스톱 사용
/// - 10% 수익 시: 1.5% 트레일링 스톱 사용 (더 타이트)
/// - 20% 수익 시: 1% 트레일링 스톱 사용 (더욱 타이트)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitLevel {
    /// 이 레벨을 활성화하는 수익률 기준 (예: 5.0은 5%)
    pub profit_pct: Decimal,
    /// 이 레벨에서 사용할 트레일링 퍼센트 (예: 2.0은 2%)
    pub trail_pct: Decimal,
}

impl ProfitLevel {
    /// 새 수익 구간 생성
    pub fn new(profit_pct: impl Into<Decimal>, trail_pct: impl Into<Decimal>) -> Self {
        Self {
            profit_pct: profit_pct.into(),
            trail_pct: trail_pct.into(),
        }
    }
}

/// 다중 모드를 지원하는 향상된 트레일링 스톱 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedTrailingStop {
    /// 트레일링 스톱 모드
    pub mode: TrailingStopMode,
    /// 현재 트리거 가격
    pub trigger_price: Decimal,
    /// 최고가 (롱: 최고가, 숏: 최저가)
    pub best_price: Decimal,
    /// 진입가 (수익률 계산용)
    pub entry_price: Decimal,
    /// 포지션 방향
    pub position_side: Side,
    /// 활성화 여부
    pub activated: bool,
    /// 현재 수익률
    pub current_profit_pct: Decimal,
    /// 가격 업데이트 횟수
    pub update_count: u64,
    /// 트레일링 스톱 활성화 가격 (선택) - 이 가격 도달 시에만 트레일링 시작
    pub activation_price: Option<Decimal>,
    /// 부분 익절 임계값 (%) - 이 수익률 도달 시 부분 익절 신호
    pub profit_lock_threshold: Option<Decimal>,
    /// 익절 시 매도 비율 (%)
    pub profit_lock_sell_pct: Decimal,
    /// 부분 익절 실행 여부
    pub profit_locked: bool,
}

impl EnhancedTrailingStop {
    /// 새 향상된 트레일링 스톱 생성
    pub fn new(
        mode: TrailingStopMode,
        entry_price: Decimal,
        current_price: Decimal,
        position_side: Side,
    ) -> Self {
        let trigger_price =
            Self::calculate_initial_trigger(&mode, current_price, entry_price, position_side);

        Self {
            mode,
            trigger_price,
            best_price: current_price,
            entry_price,
            position_side,
            activated: false,
            current_profit_pct: Decimal::ZERO,
            update_count: 0,
            activation_price: None,
            profit_lock_threshold: None,
            profit_lock_sell_pct: dec!(50),
            profit_locked: false,
        }
    }

    /// 활성화 가격 설정 (빌더 패턴)
    /// 이 가격에 도달해야 트레일링 스톱이 활성화됩니다.
    pub fn with_activation_price(mut self, price: Decimal) -> Self {
        self.activation_price = Some(price);
        self.activated = false; // 활성화 가격이 설정되면 비활성 상태로 시작
        self
    }

    /// 부분 익절 설정 (빌더 패턴)
    /// 지정된 수익률 도달 시 부분 익절 신호를 발생시킵니다.
    pub fn with_profit_lock(mut self, threshold_pct: Decimal, sell_pct: Decimal) -> Self {
        self.profit_lock_threshold = Some(threshold_pct);
        self.profit_lock_sell_pct = sell_pct;
        self
    }

    /// 모드에 따른 초기 트리거 가격 계산
    fn calculate_initial_trigger(
        mode: &TrailingStopMode,
        current_price: Decimal,
        entry_price: Decimal,
        side: Side,
    ) -> Decimal {
        let trail_distance = Self::get_trail_distance(mode, current_price, entry_price);

        match side {
            Side::Buy => current_price - trail_distance,
            Side::Sell => current_price + trail_distance,
        }
    }

    /// 모드에 따른 트레일 거리 계산
    fn get_trail_distance(
        mode: &TrailingStopMode,
        current_price: Decimal,
        entry_price: Decimal,
    ) -> Decimal {
        match mode {
            TrailingStopMode::FixedPercentage { trail_pct } => {
                current_price * trail_pct / dec!(100)
            }
            TrailingStopMode::AtrBased {
                atr_multiplier,
                current_atr,
            } => current_atr * atr_multiplier,
            TrailingStopMode::Step { profit_levels } => {
                // 현재 수익률 계산
                let profit_pct = if entry_price.is_zero() {
                    Decimal::ZERO
                } else {
                    (current_price - entry_price) / entry_price * dec!(100)
                };

                // 현재 수익 구간에 맞는 트레일 퍼센트 찾기
                let trail_pct = Self::get_step_trail_pct(profit_levels, profit_pct);
                current_price * trail_pct / dec!(100)
            }
            TrailingStopMode::ParabolicSar { .. } => {
                // 초기 계산시 퍼센트 기반 거리 사용
                // SAR은 가격 업데이트 시 동적으로 조정됨
                current_price * dec!(2) / dec!(100)
            }
        }
    }

    /// 현재 수익률에 따른 단계별 트레일 퍼센트 조회
    fn get_step_trail_pct(profit_levels: &[ProfitLevel], profit_pct: Decimal) -> Decimal {
        // 매칭되는 레벨이 없을 때 기본 트레일 퍼센트
        let mut trail_pct = dec!(3.0);

        // 도달한 가장 높은 수익 구간 찾기
        for level in profit_levels {
            if profit_pct >= level.profit_pct {
                trail_pct = level.trail_pct;
            }
        }

        trail_pct
    }

    /// 새 가격으로 트레일링 스톱 업데이트
    ///
    /// 트리거 가격이 업데이트되면 `true` 반환
    pub fn update(&mut self, current_price: Decimal) -> bool {
        self.update_count += 1;

        // 현재 수익률 업데이트
        if !self.entry_price.is_zero() {
            self.current_profit_pct = match self.position_side {
                Side::Buy => (current_price - self.entry_price) / self.entry_price * dec!(100),
                Side::Sell => (self.entry_price - current_price) / self.entry_price * dec!(100),
            };
        }

        // 활성화 가격 체크 (설정된 경우)
        if !self.activated {
            if let Some(activation) = self.activation_price {
                match self.position_side {
                    Side::Buy => {
                        if current_price < activation {
                            return false; // 아직 활성화 안됨
                        }
                    }
                    Side::Sell => {
                        if current_price > activation {
                            return false; // 아직 활성화 안됨
                        }
                    }
                }
            }
        }

        match self.position_side {
            Side::Buy => self.update_long(current_price),
            Side::Sell => self.update_short(current_price),
        }
    }

    /// 부분 익절 조건 체크
    ///
    /// 부분 익절 조건이 충족되면 `Some((threshold_pct, sell_pct))` 반환
    /// 이미 익절이 실행된 경우 `None` 반환
    pub fn check_profit_lock(&mut self) -> Option<(Decimal, Decimal)> {
        if self.profit_locked {
            return None;
        }

        if let Some(threshold) = self.profit_lock_threshold {
            if self.current_profit_pct >= threshold {
                self.profit_locked = true;
                return Some((threshold, self.profit_lock_sell_pct));
            }
        }

        None
    }

    /// 부분 익절 상태 리셋 (재사용 시)
    pub fn reset_profit_lock(&mut self) {
        self.profit_locked = false;
    }

    /// 롱 포지션 업데이트
    fn update_long(&mut self, current_price: Decimal) -> bool {
        // 신고가 확인
        if current_price <= self.best_price {
            return false;
        }

        self.best_price = current_price;
        let new_trigger = self.calculate_new_trigger(current_price);

        // 새 트리거가 더 높을 때만 업데이트 (더 많은 이익 보호)
        if new_trigger > self.trigger_price {
            self.trigger_price = new_trigger;
            self.activated = true;
            return true;
        }

        false
    }

    /// 숏 포지션 업데이트
    fn update_short(&mut self, current_price: Decimal) -> bool {
        // 신저가 확인
        if current_price >= self.best_price {
            return false;
        }

        self.best_price = current_price;
        let new_trigger = self.calculate_new_trigger(current_price);

        // 새 트리거가 더 낮을 때만 업데이트 (더 많은 이익 보호)
        if new_trigger < self.trigger_price {
            self.trigger_price = new_trigger;
            self.activated = true;
            return true;
        }

        false
    }

    /// 모드에 따른 새 트리거 가격 계산
    fn calculate_new_trigger(&mut self, current_price: Decimal) -> Decimal {
        // 빌림 충돌 방지를 위해 필요한 값 추출
        let position_side = self.position_side;
        let current_profit_pct = self.current_profit_pct;
        let best_price = self.best_price;
        let current_trigger = self.trigger_price;

        match &mut self.mode {
            TrailingStopMode::FixedPercentage { trail_pct } => {
                let distance = current_price * *trail_pct / dec!(100);
                match position_side {
                    Side::Buy => current_price - distance,
                    Side::Sell => current_price + distance,
                }
            }
            TrailingStopMode::AtrBased {
                atr_multiplier,
                current_atr,
            } => {
                let distance = *current_atr * *atr_multiplier;
                match position_side {
                    Side::Buy => current_price - distance,
                    Side::Sell => current_price + distance,
                }
            }
            TrailingStopMode::Step { profit_levels } => {
                let trail_pct = Self::get_step_trail_pct(profit_levels, current_profit_pct);
                let distance = current_price * trail_pct / dec!(100);
                match position_side {
                    Side::Buy => current_price - distance,
                    Side::Sell => current_price + distance,
                }
            }
            TrailingStopMode::ParabolicSar {
                acceleration,
                maximum,
                current_af,
            } => {
                // 가속 계수 증가 (최대값 제한)
                let new_af = (*current_af + *acceleration).min(*maximum);
                *current_af = new_af;

                // Parabolic SAR 공식으로 새 SAR 계산
                let sar_change = new_af * (best_price - current_trigger);

                match position_side {
                    Side::Buy => {
                        // 롱: SAR은 가격 아래에서 위로 이동
                        let new_sar = current_trigger + sar_change;
                        // SAR이 현재가 아래에 유지되도록 보장
                        new_sar.min(current_price - current_price * dec!(0.5) / dec!(100))
                    }
                    Side::Sell => {
                        // 숏: SAR은 가격 위에서 아래로 이동
                        let new_sar = current_trigger - sar_change;
                        // SAR이 현재가 위에 유지되도록 보장
                        new_sar.max(current_price + current_price * dec!(0.5) / dec!(100))
                    }
                }
            }
        }
    }

    /// 스톱 트리거 여부 확인
    pub fn check_triggered(&self, current_price: Decimal) -> bool {
        match self.position_side {
            Side::Buy => current_price <= self.trigger_price,
            Side::Sell => current_price >= self.trigger_price,
        }
    }

    /// ATR 기반 모드의 ATR 값 업데이트
    pub fn update_atr(&mut self, new_atr: Decimal) {
        if let TrailingStopMode::AtrBased { current_atr, .. } = &mut self.mode {
            *current_atr = new_atr;
        }
    }

    /// 현재 사용 중인 트레일 퍼센트 조회
    pub fn get_current_trail_pct(&self) -> Decimal {
        match &self.mode {
            TrailingStopMode::FixedPercentage { trail_pct } => *trail_pct,
            TrailingStopMode::AtrBased {
                atr_multiplier,
                current_atr,
            } => {
                if self.best_price.is_zero() {
                    Decimal::ZERO
                } else {
                    (*current_atr * *atr_multiplier) / self.best_price * dec!(100)
                }
            }
            TrailingStopMode::Step { profit_levels } => {
                Self::get_step_trail_pct(profit_levels, self.current_profit_pct)
            }
            TrailingStopMode::ParabolicSar { current_af, .. } => {
                // 현재 가속 계수를 대리값으로 반환
                *current_af * dec!(100)
            }
        }
    }

    /// 현재가에서 트리거까지의 거리 퍼센트 조회
    pub fn get_distance_pct(&self, current_price: Decimal) -> Decimal {
        if current_price.is_zero() {
            return Decimal::ZERO;
        }

        let distance = match self.position_side {
            Side::Buy => current_price - self.trigger_price,
            Side::Sell => self.trigger_price - current_price,
        };

        distance / current_price * dec!(100)
    }

    /// 통계 요약 조회
    pub fn get_stats(&self) -> TrailingStopStats {
        TrailingStopStats {
            mode_name: self.mode_name(),
            trigger_price: self.trigger_price,
            best_price: self.best_price,
            entry_price: self.entry_price,
            current_profit_pct: self.current_profit_pct,
            current_trail_pct: self.get_current_trail_pct(),
            activated: self.activated,
            update_count: self.update_count,
            activation_price: self.activation_price,
            profit_lock_threshold: self.profit_lock_threshold,
            profit_locked: self.profit_locked,
        }
    }

    /// 모드 이름 문자열 조회
    fn mode_name(&self) -> String {
        match &self.mode {
            TrailingStopMode::FixedPercentage { .. } => "고정 %".to_string(),
            TrailingStopMode::AtrBased { .. } => "ATR".to_string(),
            TrailingStopMode::Step { .. } => "단계별".to_string(),
            TrailingStopMode::ParabolicSar { .. } => "Parabolic SAR".to_string(),
        }
    }
}

/// 트레일링 스톱 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailingStopStats {
    /// 모드 이름
    pub mode_name: String,
    /// 현재 트리거 가격
    pub trigger_price: Decimal,
    /// 최고가/최저가
    pub best_price: Decimal,
    /// 진입가
    pub entry_price: Decimal,
    /// 현재 수익률
    pub current_profit_pct: Decimal,
    /// 현재 트레일 퍼센트
    pub current_trail_pct: Decimal,
    /// 활성화 여부
    pub activated: bool,
    /// 업데이트 횟수
    pub update_count: u64,
    /// 활성화 가격
    pub activation_price: Option<Decimal>,
    /// 부분 익절 임계값
    pub profit_lock_threshold: Option<Decimal>,
    /// 부분 익절 실행 여부
    pub profit_locked: bool,
}

/// 단계별 트레일링 스톱 빌더
pub struct StepTrailingStopBuilder {
    levels: Vec<ProfitLevel>,
}

impl StepTrailingStopBuilder {
    /// 새 빌더 생성
    pub fn new() -> Self {
        Self { levels: Vec::new() }
    }

    /// 수익 구간 추가
    ///
    /// # 인자
    /// * `profit_pct` - 수익률 기준
    /// * `trail_pct` - 해당 레벨에서 사용할 트레일링 퍼센트
    pub fn add_level(
        mut self,
        profit_pct: impl Into<Decimal>,
        trail_pct: impl Into<Decimal>,
    ) -> Self {
        self.levels.push(ProfitLevel::new(profit_pct, trail_pct));
        self
    }

    /// 트레일링 스톱 모드 빌드
    pub fn build(mut self) -> TrailingStopMode {
        // 수익률 기준 오름차순 정렬
        self.levels.sort_by(|a, b| a.profit_pct.cmp(&b.profit_pct));

        TrailingStopMode::Step {
            profit_levels: self.levels,
        }
    }

    /// 설정된 단계별 모드로 EnhancedTrailingStop 생성
    pub fn create(
        self,
        entry_price: Decimal,
        current_price: Decimal,
        position_side: Side,
    ) -> EnhancedTrailingStop {
        let mode = self.build();
        EnhancedTrailingStop::new(mode, entry_price, current_price, position_side)
    }
}

impl Default for StepTrailingStopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 사전 정의된 트레일링 스톱 프리셋
pub mod presets {
    use super::*;

    /// 보수적 프리셋: 넓은 스톱, 변동성 높은 시장에 적합
    pub fn conservative() -> TrailingStopMode {
        TrailingStopMode::FixedPercentage {
            trail_pct: dec!(5.0),
        }
    }

    /// 중립 프리셋: 균형잡힌 스톱
    pub fn moderate() -> TrailingStopMode {
        TrailingStopMode::FixedPercentage {
            trail_pct: dec!(3.0),
        }
    }

    /// 공격적 프리셋: 타이트한 스톱, 빠른 이익 확정
    pub fn aggressive() -> TrailingStopMode {
        TrailingStopMode::FixedPercentage {
            trail_pct: dec!(1.5),
        }
    }

    /// 단계별 프리셋: 수익 증가에 따라 스톱 타이트닝
    pub fn step_based() -> TrailingStopMode {
        StepTrailingStopBuilder::new()
            .add_level(dec!(0), dec!(5.0))    // 0% 수익: 5% 트레일
            .add_level(dec!(5), dec!(3.0))    // 5% 수익: 3% 트레일
            .add_level(dec!(10), dec!(2.0))   // 10% 수익: 2% 트레일
            .add_level(dec!(20), dec!(1.5))   // 20% 수익: 1.5% 트레일
            .add_level(dec!(30), dec!(1.0))   // 30% 수익: 1% 트레일
            .build()
    }

    /// ATR 기반 프리셋: 2배 ATR 사용
    pub fn atr_based(current_atr: Decimal) -> TrailingStopMode {
        TrailingStopMode::AtrBased {
            atr_multiplier: dec!(2.0),
            current_atr,
        }
    }

    /// Parabolic SAR 프리셋: 클래식 파라미터
    pub fn parabolic_sar() -> TrailingStopMode {
        TrailingStopMode::ParabolicSar {
            acceleration: dec!(0.02),
            maximum: dec!(0.20),
            current_af: dec!(0.02),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_percentage_trailing_long() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(
            mode,
            dec!(100), // 진입가
            dec!(100), // 현재가
            Side::Buy,
        );

        // 초기 트리거는 현재가의 2% 아래
        assert_eq!(stop.trigger_price, dec!(98));

        // 가격이 110으로 상승
        assert!(stop.update(dec!(110)));
        assert_eq!(stop.trigger_price, dec!(107.8)); // 110 - 2.2 (110의 2%)
        assert!(stop.activated);

        // 가격이 108로 하락 - 트리거 업데이트 안됨
        assert!(!stop.update(dec!(108)));
        assert_eq!(stop.trigger_price, dec!(107.8)); // 그대로 유지
    }

    #[test]
    fn test_fixed_percentage_trailing_short() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(
            mode,
            dec!(100), // 진입가
            dec!(100), // 현재가
            Side::Sell,
        );

        // 초기 트리거는 현재가의 2% 위
        assert_eq!(stop.trigger_price, dec!(102));

        // 가격이 90으로 하락
        assert!(stop.update(dec!(90)));
        assert_eq!(stop.trigger_price, dec!(91.8)); // 90 + 1.8 (90의 2%)

        // 가격이 92로 상승 - 트리거 업데이트 안됨
        assert!(!stop.update(dec!(92)));
        assert_eq!(stop.trigger_price, dec!(91.8));
    }

    #[test]
    fn test_step_based_trailing() {
        let mode = StepTrailingStopBuilder::new()
            .add_level(dec!(0), dec!(5.0))   // 0% 수익: 5% 트레일
            .add_level(dec!(10), dec!(3.0))  // 10% 수익: 3% 트레일
            .add_level(dec!(20), dec!(1.5))  // 20% 수익: 1.5% 트레일
            .build();

        let mut stop = EnhancedTrailingStop::new(
            mode,
            dec!(100), // 진입가
            dec!(100), // 현재가
            Side::Buy,
        );

        // 0% 수익 시: 5% 트레일
        assert_eq!(stop.get_current_trail_pct(), dec!(5.0));

        // 가격이 112로 이동 (12% 수익)
        stop.update(dec!(112));
        assert!(stop.current_profit_pct > dec!(10));
        assert_eq!(stop.get_current_trail_pct(), dec!(3.0)); // 이제 3% 트레일 사용

        // 가격이 125로 이동 (25% 수익)
        stop.update(dec!(125));
        assert!(stop.current_profit_pct > dec!(20));
        assert_eq!(stop.get_current_trail_pct(), dec!(1.5)); // 이제 1.5% 트레일 사용
    }

    #[test]
    fn test_atr_based_trailing() {
        let mode = TrailingStopMode::AtrBased {
            atr_multiplier: dec!(2.0),
            current_atr: dec!(5.0), // $5 ATR
        };

        let mut stop = EnhancedTrailingStop::new(
            mode,
            dec!(100), // 진입가
            dec!(100), // 현재가
            Side::Buy,
        );

        // 초기 트리거: 100 - (2 * 5) = 90
        assert_eq!(stop.trigger_price, dec!(90));

        // 가격이 110으로 상승
        stop.update(dec!(110));
        // 새 트리거: 110 - (2 * 5) = 100
        assert_eq!(stop.trigger_price, dec!(100));

        // ATR을 3으로 업데이트
        stop.update_atr(dec!(3.0));
        stop.update(dec!(115));
        // 새 트리거: 115 - (2 * 3) = 109
        assert_eq!(stop.trigger_price, dec!(109));
    }

    #[test]
    fn test_check_triggered() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy);

        // 트리거 가격 98
        assert!(!stop.check_triggered(dec!(99))); // 트리거 위
        assert!(stop.check_triggered(dec!(98))); // 트리거에서
        assert!(stop.check_triggered(dec!(97))); // 트리거 아래
    }

    #[test]
    fn test_parabolic_sar_mode() {
        let mode = presets::parabolic_sar();
        let mut stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy);

        // 초기 상태
        assert!(!stop.activated);

        // 가격 상승
        stop.update(dec!(105));
        stop.update(dec!(110));
        stop.update(dec!(115));

        // 트리거가 상승해야 함
        assert!(stop.trigger_price > dec!(98)); // 초기값은 약 98
        assert!(stop.activated);
    }

    #[test]
    fn test_presets() {
        // 보수적 프리셋 테스트
        let mode = presets::conservative();
        if let TrailingStopMode::FixedPercentage { trail_pct } = mode {
            assert_eq!(trail_pct, dec!(5.0));
        }

        // 단계별 프리셋 테스트
        let mode = presets::step_based();
        if let TrailingStopMode::Step { profit_levels } = mode {
            assert_eq!(profit_levels.len(), 5);
        }
    }

    #[test]
    fn test_step_trailing_stop_builder() {
        let stop = StepTrailingStopBuilder::new()
            .add_level(0, 5)
            .add_level(10, 3)
            .add_level(20, 2)
            .create(dec!(100), dec!(100), Side::Buy);

        assert_eq!(stop.entry_price, dec!(100));
        assert_eq!(stop.position_side, Side::Buy);
    }

    #[test]
    fn test_stats() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy);

        stop.update(dec!(110));

        let stats = stop.get_stats();
        assert_eq!(stats.mode_name, "고정 %");
        assert!(stats.current_profit_pct > Decimal::ZERO);
        assert!(stats.activated);
        assert_eq!(stats.update_count, 1);
    }

    #[test]
    fn test_distance_pct() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy);

        let distance = stop.get_distance_pct(dec!(100));
        assert_eq!(distance, dec!(2.0)); // 2% 거리
    }

    #[test]
    fn test_activation_price_long() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy)
            .with_activation_price(dec!(105)); // 105에서 활성화

        // 초기에는 활성화되지 않음
        assert!(!stop.activated);

        // 102로 상승해도 활성화 안됨
        let updated = stop.update(dec!(102));
        assert!(!updated);
        assert!(!stop.activated);

        // 106으로 상승하면 활성화
        let updated = stop.update(dec!(106));
        assert!(updated);
        assert!(stop.activated);
        // 트리거: 106 - 2.12 = 103.88
        assert!(stop.trigger_price > dec!(103));
    }

    #[test]
    fn test_activation_price_short() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Sell)
            .with_activation_price(dec!(95)); // 95 이하에서 활성화

        // 98로 하락해도 활성화 안됨
        let updated = stop.update(dec!(98));
        assert!(!updated);
        assert!(!stop.activated);

        // 94로 하락하면 활성화
        let updated = stop.update(dec!(94));
        assert!(updated);
        assert!(stop.activated);
    }

    #[test]
    fn test_profit_lock() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let mut stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy)
            .with_profit_lock(dec!(10), dec!(50)); // 10% 수익 시 50% 매도

        // 초기에는 익절 조건 미충족
        assert!(stop.check_profit_lock().is_none());

        // 5% 수익 - 아직 미충족
        stop.update(dec!(105));
        assert!(stop.check_profit_lock().is_none());

        // 12% 수익 - 조건 충족
        stop.update(dec!(112));
        let result = stop.check_profit_lock();
        assert!(result.is_some());
        let (threshold, sell_pct) = result.unwrap();
        assert_eq!(threshold, dec!(10));
        assert_eq!(sell_pct, dec!(50));

        // 이미 익절했으므로 다시 호출해도 None
        assert!(stop.check_profit_lock().is_none());
        assert!(stop.profit_locked);

        // 리셋 후 다시 체크 가능
        stop.reset_profit_lock();
        assert!(!stop.profit_locked);
    }

    #[test]
    fn test_stats_with_new_fields() {
        let mode = TrailingStopMode::FixedPercentage {
            trail_pct: dec!(2.0),
        };
        let stop = EnhancedTrailingStop::new(mode, dec!(100), dec!(100), Side::Buy)
            .with_activation_price(dec!(105))
            .with_profit_lock(dec!(10), dec!(50));

        let stats = stop.get_stats();
        assert_eq!(stats.activation_price, Some(dec!(105)));
        assert_eq!(stats.profit_lock_threshold, Some(dec!(10)));
        assert!(!stats.profit_locked);
    }
}
