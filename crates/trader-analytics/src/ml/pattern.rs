//! 캔들스틱 및 차트 패턴 인식 모듈.
//!
//! 이 모듈은 두 가지 유형의 패턴 인식을 제공합니다:
//!
//! 1. **캔들스틱 패턴** - 개별 또는 연속 캔들의 형태 분석
//!    - 단일 캔들: Doji, Hammer, Shooting Star, Marubozu 등
//!    - 이중 캔들: Engulfing, Harami, Piercing Line 등
//!    - 삼중 캔들: Morning Star, Evening Star, Three White Soldiers 등
//!
//! 2. **차트 패턴** - 다수의 캔들로 형성되는 가격 형태 분석
//!    - 반전 패턴: Head and Shoulders, Double Top/Bottom 등
//!    - 지속 패턴: Triangle, Wedge, Flag, Channel 등
//!
//! # 예제
//!
//! ```ignore
//! use trader_analytics::ml::pattern::{PatternRecognizer, PatternConfig};
//! use trader_core::Kline;
//!
//! let config = PatternConfig::default();
//! let recognizer = PatternRecognizer::new(config);
//!
//! // 캔들스틱 패턴 감지
//! let candle_patterns = recognizer.detect_candlestick_patterns(&klines);
//!
//! // 차트 패턴 감지
//! let chart_patterns = recognizer.detect_chart_patterns(&klines);
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use trader_core::Kline;

// ==================== 캔들스틱 패턴 타입 ====================

/// 캔들스틱 패턴 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandlestickPatternType {
    // === 단일 캔들 패턴 (Bullish) ===
    /// Doji - 시가와 종가가 거의 같음 (중립/반전 신호)
    Doji,
    /// Dragonfly Doji - 긴 아래꼬리, 위꼬리 없음 (상승 반전)
    DragonflyDoji,
    /// Gravestone Doji - 긴 위꼬리, 아래꼬리 없음 (하락 반전)
    GravestoneDoji,
    /// Hammer - 짧은 몸통, 긴 아래꼬리 (상승 반전)
    Hammer,
    /// Inverted Hammer - 짧은 몸통, 긴 위꼬리 (상승 반전)
    InvertedHammer,
    /// Bullish Marubozu - 꼬리 없는 강한 양봉
    BullishMarubozu,
    /// Spinning Top - 짧은 몸통, 긴 양쪽 꼬리 (우유부단)
    SpinningTop,

    // === 단일 캔들 패턴 (Bearish) ===
    /// Shooting Star - 짧은 몸통, 긴 위꼬리 (하락 반전)
    ShootingStar,
    /// Hanging Man - 짧은 몸통, 긴 아래꼬리 (하락 반전)
    HangingMan,
    /// Bearish Marubozu - 꼬리 없는 강한 음봉
    BearishMarubozu,

    // === 이중 캔들 패턴 (Bullish) ===
    /// Bullish Engulfing - 양봉이 이전 음봉을 감싸는 패턴
    BullishEngulfing,
    /// Bullish Harami - 작은 양봉이 이전 음봉 안에 포함
    BullishHarami,
    /// Piercing Line - 음봉 후 중간 이상 반등하는 양봉
    PiercingLine,
    /// Tweezer Bottom - 동일 저가의 두 캔들 (바닥)
    TweezerBottom,

    // === 이중 캔들 패턴 (Bearish) ===
    /// Bearish Engulfing - 음봉이 이전 양봉을 감싸는 패턴
    BearishEngulfing,
    /// Bearish Harami - 작은 음봉이 이전 양봉 안에 포함
    BearishHarami,
    /// Dark Cloud Cover - 양봉 후 중간 이하로 하락하는 음봉
    DarkCloudCover,
    /// Tweezer Top - 동일 고가의 두 캔들 (천장)
    TweezerTop,

    // === 삼중 캔들 패턴 (Bullish) ===
    /// Morning Star - 음봉 + 작은 캔들 + 양봉 (강한 상승 반전)
    MorningStar,
    /// Morning Doji Star - Morning Star의 Doji 버전
    MorningDojiStar,
    /// Three White Soldiers - 연속 3개 양봉 (강한 상승)
    ThreeWhiteSoldiers,
    /// Bullish Abandoned Baby - 갭으로 분리된 Doji 포함 패턴
    BullishAbandonedBaby,

    // === 삼중 캔들 패턴 (Bearish) ===
    /// Evening Star - 양봉 + 작은 캔들 + 음봉 (강한 하락 반전)
    EveningStar,
    /// Evening Doji Star - Evening Star의 Doji 버전
    EveningDojiStar,
    /// Three Black Crows - 연속 3개 음봉 (강한 하락)
    ThreeBlackCrows,
    /// Bearish Abandoned Baby - 갭으로 분리된 Doji 포함 패턴
    BearishAbandonedBaby,
}

/// 캔들스틱 패턴 감지 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickPattern {
    /// 패턴 유형
    pub pattern_type: CandlestickPatternType,
    /// 패턴이 발생한 마지막 캔들 인덱스
    pub end_index: usize,
    /// 패턴에 포함된 캔들 수
    pub candle_count: usize,
    /// 신뢰도 (0.0 ~ 1.0)
    pub confidence: f64,
    /// 예상 방향 (true = 상승, false = 하락)
    pub bullish: bool,
    /// 패턴 발생 시간
    pub timestamp: DateTime<Utc>,
    /// 추가 메타데이터
    pub metadata: HashMap<String, String>,
}

// ==================== 차트 패턴 타입 ====================

/// 차트 패턴 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartPatternType {
    // === 반전 패턴 (Reversal) ===
    /// Head and Shoulders - 머리어깨형 (하락 반전)
    HeadAndShoulders,
    /// Inverse Head and Shoulders - 역머리어깨형 (상승 반전)
    InverseHeadAndShoulders,
    /// Double Top - 이중 천장 (하락 반전)
    DoubleTop,
    /// Double Bottom - 이중 바닥 (상승 반전)
    DoubleBottom,
    /// Triple Top - 삼중 천장 (하락 반전)
    TripleTop,
    /// Triple Bottom - 삼중 바닥 (상승 반전)
    TripleBottom,
    /// Rounding Top - 둥근 천장 (하락 반전)
    RoundingTop,
    /// Rounding Bottom (Cup) - 컵 형태 바닥 (상승 반전)
    RoundingBottom,

    // === 지속 패턴 (Continuation) ===
    /// Ascending Triangle - 상승 삼각형 (상승 지속)
    AscendingTriangle,
    /// Descending Triangle - 하락 삼각형 (하락 지속)
    DescendingTriangle,
    /// Symmetrical Triangle - 대칭 삼각형 (방향 불확실)
    SymmetricalTriangle,
    /// Rising Wedge - 상승 쐐기 (하락 반전)
    RisingWedge,
    /// Falling Wedge - 하락 쐐기 (상승 반전)
    FallingWedge,
    /// Bullish Flag - 상승 깃발 (상승 지속)
    BullishFlag,
    /// Bearish Flag - 하락 깃발 (하락 지속)
    BearishFlag,
    /// Bullish Pennant - 상승 페넌트 (상승 지속)
    BullishPennant,
    /// Bearish Pennant - 하락 페넌트 (하락 지속)
    BearishPennant,
    /// Ascending Channel - 상승 채널
    AscendingChannel,
    /// Descending Channel - 하락 채널
    DescendingChannel,
    /// Horizontal Channel (Rectangle) - 횡보 채널
    HorizontalChannel,

    // === 기타 패턴 ===
    /// Cup and Handle - 컵앤핸들 (상승)
    CupAndHandle,
    /// Inverse Cup and Handle - 역컵앤핸들 (하락)
    InverseCupAndHandle,
    /// Broadening Formation - 확대형 (변동성 증가)
    BroadeningFormation,
}

/// 차트 패턴 감지 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPattern {
    /// 패턴 유형
    pub pattern_type: ChartPatternType,
    /// 패턴 시작 인덱스
    pub start_index: usize,
    /// 패턴 종료 인덱스
    pub end_index: usize,
    /// 주요 지점들 (피크, 밸리 등)
    pub key_points: Vec<PatternPoint>,
    /// 저항선/지지선
    pub trendlines: Vec<Trendline>,
    /// 목표가 (패턴 완성 후 예상 이동 범위)
    pub price_target: Option<Decimal>,
    /// 신뢰도 (0.0 ~ 1.0)
    pub confidence: f64,
    /// 예상 방향 (true = 상승, false = 하락)
    pub bullish: bool,
    /// 패턴 완성 여부
    pub is_complete: bool,
    /// 패턴 발생 시간
    pub timestamp: DateTime<Utc>,
}

/// 패턴의 주요 지점.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternPoint {
    /// 인덱스
    pub index: usize,
    /// 가격
    pub price: Decimal,
    /// 시간
    pub timestamp: DateTime<Utc>,
    /// 포인트 유형 (peak, valley, neckline 등)
    pub point_type: String,
}

/// 추세선.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trendline {
    /// 시작점
    pub start: PatternPoint,
    /// 끝점
    pub end: PatternPoint,
    /// 기울기
    pub slope: Decimal,
    /// 추세선 유형 (support, resistance, neckline)
    pub line_type: String,
}

// ==================== 패턴 인식기 설정 ====================

/// 패턴 인식 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternConfig {
    // === 캔들스틱 패턴 임계값 ===
    /// Doji 인식을 위한 몸통/범위 비율 (기본: 0.1 = 10%)
    pub doji_body_ratio: f64,
    /// Hammer/Shooting Star 꼬리 비율 (기본: 2.0)
    pub shadow_body_ratio: f64,
    /// Marubozu 꼬리 허용 비율 (기본: 0.05 = 5%)
    pub marubozu_shadow_ratio: f64,
    /// Engulfing 감싸기 최소 비율 (기본: 1.0)
    pub engulfing_ratio: f64,
    /// Star 패턴의 갭 비율 (기본: 0.01)
    pub star_gap_ratio: f64,

    // === 차트 패턴 설정 ===
    /// 피크/밸리 감지를 위한 최소 기간
    pub pivot_lookback: usize,
    /// 패턴 감지를 위한 최소 캔들 수
    pub min_pattern_bars: usize,
    /// 패턴 감지를 위한 최대 캔들 수
    pub max_pattern_bars: usize,
    /// 가격 허용 오차 (%, 같은 가격 레벨 판단용)
    pub price_tolerance: f64,
    /// 추세선 기울기 허용 오차
    pub slope_tolerance: f64,
    /// 최소 신뢰도 임계값
    pub min_confidence: f64,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            // 캔들스틱 설정
            doji_body_ratio: 0.1,
            shadow_body_ratio: 2.0,
            marubozu_shadow_ratio: 0.05,
            engulfing_ratio: 1.0,
            star_gap_ratio: 0.01,
            // 차트 패턴 설정
            pivot_lookback: 5,
            min_pattern_bars: 10,
            max_pattern_bars: 100,
            price_tolerance: 0.02,
            slope_tolerance: 0.1,
            min_confidence: 0.6,
        }
    }
}

// ==================== 패턴 인식기 ====================

/// 패턴 인식기.
pub struct PatternRecognizer {
    config: PatternConfig,
}

impl PatternRecognizer {
    /// 새 패턴 인식기를 생성합니다.
    pub fn new(config: PatternConfig) -> Self {
        Self { config }
    }

    /// 기본 설정으로 패턴 인식기를 생성합니다.
    pub fn with_defaults() -> Self {
        Self::new(PatternConfig::default())
    }

    // ==================== 캔들스틱 패턴 감지 ====================

    /// 모든 캔들스틱 패턴을 감지합니다.
    pub fn detect_candlestick_patterns(&self, klines: &[Kline]) -> Vec<CandlestickPattern> {
        let mut patterns = Vec::new();

        if klines.is_empty() {
            return patterns;
        }

        // 단일 캔들 패턴 (모든 캔들에 대해)
        for i in 0..klines.len() {
            patterns.extend(self.detect_single_candle_patterns(klines, i));
        }

        // 이중 캔들 패턴 (두 번째 캔들부터)
        for i in 1..klines.len() {
            patterns.extend(self.detect_double_candle_patterns(klines, i));
        }

        // 삼중 캔들 패턴 (세 번째 캔들부터)
        for i in 2..klines.len() {
            patterns.extend(self.detect_triple_candle_patterns(klines, i));
        }

        // 신뢰도 기준으로 필터링
        patterns
            .into_iter()
            .filter(|p| p.confidence >= self.config.min_confidence)
            .collect()
    }

    /// 단일 캔들 패턴 감지.
    fn detect_single_candle_patterns(
        &self,
        klines: &[Kline],
        index: usize,
    ) -> Vec<CandlestickPattern> {
        let mut patterns = Vec::new();
        let candle = &klines[index];

        let body = candle.body_size();
        let range = candle.range();
        let upper_shadow = candle.high - candle.open.max(candle.close);
        let lower_shadow = candle.open.min(candle.close) - candle.low;

        // 범위가 0이면 패턴 감지 불가
        if range.is_zero() {
            return patterns;
        }

        let body_ratio = body / range;
        let upper_ratio = upper_shadow / range;
        let lower_ratio = lower_shadow / range;

        // Doji 계열
        if body_ratio < Decimal::try_from(self.config.doji_body_ratio).unwrap_or(dec!(0.1)) {
            let pattern_type = if lower_ratio > dec!(0.6) && upper_ratio < dec!(0.1) {
                CandlestickPatternType::DragonflyDoji
            } else if upper_ratio > dec!(0.6) && lower_ratio < dec!(0.1) {
                CandlestickPatternType::GravestoneDoji
            } else {
                CandlestickPatternType::Doji
            };

            let bullish = matches!(
                pattern_type,
                CandlestickPatternType::DragonflyDoji | CandlestickPatternType::Doji
            );

            patterns.push(CandlestickPattern {
                pattern_type,
                end_index: index,
                candle_count: 1,
                confidence: self.calculate_doji_confidence(body_ratio),
                bullish,
                timestamp: candle.open_time,
                metadata: HashMap::new(),
            });
        }

        // Hammer / Hanging Man (긴 아래꼬리, 작은 몸통)
        let shadow_body_threshold =
            Decimal::try_from(self.config.shadow_body_ratio).unwrap_or(dec!(2.0));
        if !body.is_zero()
            && lower_shadow / body >= shadow_body_threshold
            && upper_ratio < dec!(0.2)
        {
            // 이전 추세에 따라 Hammer vs Hanging Man 구분
            let is_downtrend = index >= 3
                && klines[index - 3..index]
                    .iter()
                    .all(|k| k.close < k.open || k.close < klines[index - 3].close);

            let pattern_type = if is_downtrend {
                CandlestickPatternType::Hammer
            } else {
                CandlestickPatternType::HangingMan
            };

            patterns.push(CandlestickPattern {
                pattern_type,
                end_index: index,
                candle_count: 1,
                confidence: self.calculate_shadow_confidence(lower_shadow, body),
                bullish: matches!(pattern_type, CandlestickPatternType::Hammer),
                timestamp: candle.open_time,
                metadata: HashMap::new(),
            });
        }

        // Shooting Star / Inverted Hammer (긴 위꼬리, 작은 몸통)
        if !body.is_zero()
            && upper_shadow / body >= shadow_body_threshold
            && lower_ratio < dec!(0.2)
        {
            let is_uptrend = index >= 3
                && klines[index - 3..index]
                    .iter()
                    .all(|k| k.close > k.open || k.close > klines[index - 3].close);

            let pattern_type = if is_uptrend {
                CandlestickPatternType::ShootingStar
            } else {
                CandlestickPatternType::InvertedHammer
            };

            patterns.push(CandlestickPattern {
                pattern_type,
                end_index: index,
                candle_count: 1,
                confidence: self.calculate_shadow_confidence(upper_shadow, body),
                bullish: matches!(pattern_type, CandlestickPatternType::InvertedHammer),
                timestamp: candle.open_time,
                metadata: HashMap::new(),
            });
        }

        // Marubozu (꼬리 거의 없음)
        let marubozu_threshold =
            Decimal::try_from(self.config.marubozu_shadow_ratio).unwrap_or(dec!(0.05));
        if upper_ratio < marubozu_threshold && lower_ratio < marubozu_threshold {
            let pattern_type = if candle.is_bullish() {
                CandlestickPatternType::BullishMarubozu
            } else {
                CandlestickPatternType::BearishMarubozu
            };

            patterns.push(CandlestickPattern {
                pattern_type,
                end_index: index,
                candle_count: 1,
                confidence: 0.85,
                bullish: candle.is_bullish(),
                timestamp: candle.open_time,
                metadata: HashMap::new(),
            });
        }

        // Spinning Top (짧은 몸통, 긴 양쪽 꼬리)
        if body_ratio < dec!(0.3) && upper_ratio > dec!(0.3) && lower_ratio > dec!(0.3) {
            patterns.push(CandlestickPattern {
                pattern_type: CandlestickPatternType::SpinningTop,
                end_index: index,
                candle_count: 1,
                confidence: 0.6,
                bullish: false, // 중립
                timestamp: candle.open_time,
                metadata: HashMap::new(),
            });
        }

        patterns
    }

    /// 이중 캔들 패턴 감지.
    fn detect_double_candle_patterns(
        &self,
        klines: &[Kline],
        index: usize,
    ) -> Vec<CandlestickPattern> {
        let mut patterns = Vec::new();

        if index < 1 {
            return patterns;
        }

        let prev = &klines[index - 1];
        let curr = &klines[index];

        // Bullish Engulfing
        if prev.is_bearish()
            && curr.is_bullish()
            && curr.open <= prev.close
            && curr.close >= prev.open
            && !prev.body_size().is_zero()
        // Division by zero 방지
        {
            let engulf_ratio = curr.body_size() / prev.body_size();
            if engulf_ratio >= Decimal::try_from(self.config.engulfing_ratio).unwrap_or(dec!(1.0)) {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::BullishEngulfing,
                    end_index: index,
                    candle_count: 2,
                    confidence: self.calculate_engulfing_confidence(engulf_ratio),
                    bullish: true,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }
        }

        // Bearish Engulfing
        if prev.is_bullish()
            && curr.is_bearish()
            && curr.open >= prev.close
            && curr.close <= prev.open
            && !prev.body_size().is_zero()
        // Division by zero 방지
        {
            let engulf_ratio = curr.body_size() / prev.body_size();
            if engulf_ratio >= Decimal::try_from(self.config.engulfing_ratio).unwrap_or(dec!(1.0)) {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::BearishEngulfing,
                    end_index: index,
                    candle_count: 2,
                    confidence: self.calculate_engulfing_confidence(engulf_ratio),
                    bullish: false,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }
        }

        // Bullish Harami
        if prev.is_bearish()
            && curr.is_bullish()
            && curr.open > prev.close
            && curr.close < prev.open
            && curr.body_size() < prev.body_size()
        {
            patterns.push(CandlestickPattern {
                pattern_type: CandlestickPatternType::BullishHarami,
                end_index: index,
                candle_count: 2,
                confidence: 0.7,
                bullish: true,
                timestamp: curr.open_time,
                metadata: HashMap::new(),
            });
        }

        // Bearish Harami
        if prev.is_bullish()
            && curr.is_bearish()
            && curr.open < prev.close
            && curr.close > prev.open
            && curr.body_size() < prev.body_size()
        {
            patterns.push(CandlestickPattern {
                pattern_type: CandlestickPatternType::BearishHarami,
                end_index: index,
                candle_count: 2,
                confidence: 0.7,
                bullish: false,
                timestamp: curr.open_time,
                metadata: HashMap::new(),
            });
        }

        // Piercing Line (음봉 후 중간 이상 반등)
        if prev.is_bearish() && curr.is_bullish() {
            let prev_mid = (prev.open + prev.close) / dec!(2);
            if curr.open < prev.close && curr.close > prev_mid && curr.close < prev.open {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::PiercingLine,
                    end_index: index,
                    candle_count: 2,
                    confidence: 0.75,
                    bullish: true,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }
        }

        // Dark Cloud Cover (양봉 후 중간 이하로 하락)
        if prev.is_bullish() && curr.is_bearish() {
            let prev_mid = (prev.open + prev.close) / dec!(2);
            if curr.open > prev.close && curr.close < prev_mid && curr.close > prev.open {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::DarkCloudCover,
                    end_index: index,
                    candle_count: 2,
                    confidence: 0.75,
                    bullish: false,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }
        }

        // Tweezer Bottom (동일 저가)
        let tolerance =
            prev.range() * Decimal::try_from(self.config.price_tolerance).unwrap_or(dec!(0.02));
        if (prev.low - curr.low).abs() <= tolerance
            && prev.is_bearish() && curr.is_bullish() {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::TweezerBottom,
                    end_index: index,
                    candle_count: 2,
                    confidence: 0.7,
                    bullish: true,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }

        // Tweezer Top (동일 고가)
        if (prev.high - curr.high).abs() <= tolerance
            && prev.is_bullish() && curr.is_bearish() {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::TweezerTop,
                    end_index: index,
                    candle_count: 2,
                    confidence: 0.7,
                    bullish: false,
                    timestamp: curr.open_time,
                    metadata: HashMap::new(),
                });
            }

        patterns
    }

    /// 삼중 캔들 패턴 감지.
    fn detect_triple_candle_patterns(
        &self,
        klines: &[Kline],
        index: usize,
    ) -> Vec<CandlestickPattern> {
        let mut patterns = Vec::new();

        if index < 2 {
            return patterns;
        }

        let first = &klines[index - 2];
        let second = &klines[index - 1];
        let third = &klines[index];

        // Morning Star
        if first.is_bearish()
            && second.body_size() < first.body_size() / dec!(3)
            && third.is_bullish()
            && third.close > (first.open + first.close) / dec!(2)
        {
            patterns.push(CandlestickPattern {
                pattern_type: if self.is_doji(second) {
                    CandlestickPatternType::MorningDojiStar
                } else {
                    CandlestickPatternType::MorningStar
                },
                end_index: index,
                candle_count: 3,
                confidence: 0.85,
                bullish: true,
                timestamp: third.open_time,
                metadata: HashMap::new(),
            });
        }

        // Evening Star
        if first.is_bullish()
            && second.body_size() < first.body_size() / dec!(3)
            && third.is_bearish()
            && third.close < (first.open + first.close) / dec!(2)
        {
            patterns.push(CandlestickPattern {
                pattern_type: if self.is_doji(second) {
                    CandlestickPatternType::EveningDojiStar
                } else {
                    CandlestickPatternType::EveningStar
                },
                end_index: index,
                candle_count: 3,
                confidence: 0.85,
                bullish: false,
                timestamp: third.open_time,
                metadata: HashMap::new(),
            });
        }

        // Three White Soldiers (연속 3 양봉)
        if first.is_bullish()
            && second.is_bullish()
            && third.is_bullish()
            && second.close > first.close
            && third.close > second.close
            && second.open > first.open
            && third.open > second.open
        {
            patterns.push(CandlestickPattern {
                pattern_type: CandlestickPatternType::ThreeWhiteSoldiers,
                end_index: index,
                candle_count: 3,
                confidence: 0.9,
                bullish: true,
                timestamp: third.open_time,
                metadata: HashMap::new(),
            });
        }

        // Three Black Crows (연속 3 음봉)
        if first.is_bearish()
            && second.is_bearish()
            && third.is_bearish()
            && second.close < first.close
            && third.close < second.close
            && second.open < first.open
            && third.open < second.open
        {
            patterns.push(CandlestickPattern {
                pattern_type: CandlestickPatternType::ThreeBlackCrows,
                end_index: index,
                candle_count: 3,
                confidence: 0.9,
                bullish: false,
                timestamp: third.open_time,
                metadata: HashMap::new(),
            });
        }

        // Abandoned Baby (갭으로 분리된 Doji)
        if self.is_doji(second) {
            // Bullish Abandoned Baby
            if first.is_bearish()
                && third.is_bullish()
                && second.high < first.low
                && second.high < third.low
            {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::BullishAbandonedBaby,
                    end_index: index,
                    candle_count: 3,
                    confidence: 0.95,
                    bullish: true,
                    timestamp: third.open_time,
                    metadata: HashMap::new(),
                });
            }

            // Bearish Abandoned Baby
            if first.is_bullish()
                && third.is_bearish()
                && second.low > first.high
                && second.low > third.high
            {
                patterns.push(CandlestickPattern {
                    pattern_type: CandlestickPatternType::BearishAbandonedBaby,
                    end_index: index,
                    candle_count: 3,
                    confidence: 0.95,
                    bullish: false,
                    timestamp: third.open_time,
                    metadata: HashMap::new(),
                });
            }
        }

        patterns
    }

    // ==================== 차트 패턴 감지 ====================

    /// 모든 차트 패턴을 감지합니다.
    pub fn detect_chart_patterns(&self, klines: &[Kline]) -> Vec<ChartPattern> {
        if klines.len() < self.config.min_pattern_bars {
            return Vec::new();
        }

        let mut patterns = Vec::new();

        // 피크와 밸리 감지
        let pivots = self.detect_pivots(klines);

        // 각 패턴 감지
        patterns.extend(self.detect_head_and_shoulders(klines, &pivots));
        patterns.extend(self.detect_double_patterns(klines, &pivots));
        patterns.extend(self.detect_triple_patterns(klines, &pivots));
        patterns.extend(self.detect_triangles(klines, &pivots));
        patterns.extend(self.detect_wedges(klines, &pivots));
        patterns.extend(self.detect_channels(klines, &pivots));
        patterns.extend(self.detect_flags_pennants(klines, &pivots));

        // 신뢰도 기준으로 필터링
        patterns
            .into_iter()
            .filter(|p| p.confidence >= self.config.min_confidence)
            .collect()
    }

    /// 피크와 밸리(피봇 포인트) 감지.
    fn detect_pivots(&self, klines: &[Kline]) -> Vec<PatternPoint> {
        let mut pivots = Vec::new();
        let lookback = self.config.pivot_lookback;

        for i in lookback..klines.len().saturating_sub(lookback) {
            let current_high = klines[i].high;
            let current_low = klines[i].low;

            // 피크 검사 (좌우 lookback 범위 내 최고가)
            let is_peak = klines[i.saturating_sub(lookback)..=i + lookback]
                .iter()
                .all(|k| k.high <= current_high);

            // 밸리 검사 (좌우 lookback 범위 내 최저가)
            let is_valley = klines[i.saturating_sub(lookback)..=i + lookback]
                .iter()
                .all(|k| k.low >= current_low);

            if is_peak {
                pivots.push(PatternPoint {
                    index: i,
                    price: current_high,
                    timestamp: klines[i].open_time,
                    point_type: "peak".to_string(),
                });
            }

            if is_valley {
                pivots.push(PatternPoint {
                    index: i,
                    price: current_low,
                    timestamp: klines[i].open_time,
                    point_type: "valley".to_string(),
                });
            }
        }

        pivots
    }

    /// Head and Shoulders 패턴 감지.
    fn detect_head_and_shoulders(
        &self,
        klines: &[Kline],
        pivots: &[PatternPoint],
    ) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 3 || valleys.len() < 2 {
            return patterns;
        }

        let tolerance = Decimal::try_from(self.config.price_tolerance).unwrap_or(dec!(0.02));

        // 연속된 3개의 피크 검사
        for i in 0..peaks.len().saturating_sub(2) {
            let left_shoulder = peaks[i];
            let head = peaks[i + 1];
            let right_shoulder = peaks[i + 2];

            // 머리가 양 어깨보다 높아야 함
            if head.price <= left_shoulder.price || head.price <= right_shoulder.price {
                continue;
            }

            // 양 어깨가 비슷한 높이
            let shoulder_diff = (left_shoulder.price - right_shoulder.price).abs();
            let shoulder_avg = (left_shoulder.price + right_shoulder.price) / dec!(2);
            if shoulder_diff / shoulder_avg > tolerance {
                continue;
            }

            // 밸리(넥라인) 찾기
            let neckline_points: Vec<&PatternPoint> = valleys
                .iter()
                .filter(|v| v.index > left_shoulder.index && v.index < right_shoulder.index)
                .cloned()
                .collect();

            if neckline_points.len() < 2 {
                continue;
            }

            // 목표가 계산 (머리 - 넥라인)
            let neckline_price = (neckline_points[0].price
                + neckline_points[neckline_points.len() - 1].price)
                / dec!(2);
            let height = head.price - neckline_price;
            let price_target = neckline_price - height;

            patterns.push(ChartPattern {
                pattern_type: ChartPatternType::HeadAndShoulders,
                start_index: left_shoulder.index,
                end_index: right_shoulder.index,
                key_points: vec![left_shoulder.clone(), head.clone(), right_shoulder.clone()],
                trendlines: vec![Trendline {
                    start: neckline_points[0].clone(),
                    end: neckline_points[neckline_points.len() - 1].clone(),
                    slope: Decimal::ZERO,
                    line_type: "neckline".to_string(),
                }],
                price_target: Some(price_target),
                confidence: 0.8,
                bullish: false,
                is_complete: true,
                timestamp: klines[right_shoulder.index].open_time,
            });
        }

        // Inverse Head and Shoulders (역머리어깨)
        for i in 0..valleys.len().saturating_sub(2) {
            let left_shoulder = valleys[i];
            let head = valleys[i + 1];
            let right_shoulder = valleys[i + 2];

            // 머리가 양 어깨보다 낮아야 함
            if head.price >= left_shoulder.price || head.price >= right_shoulder.price {
                continue;
            }

            let shoulder_diff = (left_shoulder.price - right_shoulder.price).abs();
            let shoulder_avg = (left_shoulder.price + right_shoulder.price) / dec!(2);
            if shoulder_diff / shoulder_avg > tolerance {
                continue;
            }

            let neckline_points: Vec<&PatternPoint> = peaks
                .iter()
                .filter(|p| p.index > left_shoulder.index && p.index < right_shoulder.index)
                .cloned()
                .collect();

            if neckline_points.len() < 2 {
                continue;
            }

            let neckline_price = (neckline_points[0].price
                + neckline_points[neckline_points.len() - 1].price)
                / dec!(2);
            let height = neckline_price - head.price;
            let price_target = neckline_price + height;

            patterns.push(ChartPattern {
                pattern_type: ChartPatternType::InverseHeadAndShoulders,
                start_index: left_shoulder.index,
                end_index: right_shoulder.index,
                key_points: vec![left_shoulder.clone(), head.clone(), right_shoulder.clone()],
                trendlines: vec![Trendline {
                    start: neckline_points[0].clone(),
                    end: neckline_points[neckline_points.len() - 1].clone(),
                    slope: Decimal::ZERO,
                    line_type: "neckline".to_string(),
                }],
                price_target: Some(price_target),
                confidence: 0.8,
                bullish: true,
                is_complete: true,
                timestamp: klines[right_shoulder.index].open_time,
            });
        }

        patterns
    }

    /// Double Top/Bottom 패턴 감지.
    fn detect_double_patterns(
        &self,
        klines: &[Kline],
        pivots: &[PatternPoint],
    ) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        let tolerance = Decimal::try_from(self.config.price_tolerance).unwrap_or(dec!(0.02));

        // Double Top
        for i in 0..peaks.len().saturating_sub(1) {
            let first_peak = peaks[i];
            let second_peak = peaks[i + 1];

            // 두 피크가 비슷한 높이
            let price_diff = (first_peak.price - second_peak.price).abs();
            let avg_price = (first_peak.price + second_peak.price) / dec!(2);
            if price_diff / avg_price > tolerance {
                continue;
            }

            // 중간 밸리 찾기
            let middle_valley = valleys
                .iter()
                .filter(|v| v.index > first_peak.index && v.index < second_peak.index)
                .min_by(|a, b| a.price.cmp(&b.price));

            if let Some(valley) = middle_valley {
                let height = avg_price - valley.price;
                let price_target = valley.price - height;

                patterns.push(ChartPattern {
                    pattern_type: ChartPatternType::DoubleTop,
                    start_index: first_peak.index,
                    end_index: second_peak.index,
                    key_points: vec![first_peak.clone(), (*valley).clone(), second_peak.clone()],
                    trendlines: vec![],
                    price_target: Some(price_target),
                    confidence: 0.75,
                    bullish: false,
                    is_complete: true,
                    timestamp: klines[second_peak.index].open_time,
                });
            }
        }

        // Double Bottom
        for i in 0..valleys.len().saturating_sub(1) {
            let first_valley = valleys[i];
            let second_valley = valleys[i + 1];

            let price_diff = (first_valley.price - second_valley.price).abs();
            let avg_price = (first_valley.price + second_valley.price) / dec!(2);
            if price_diff / avg_price > tolerance {
                continue;
            }

            let middle_peak = peaks
                .iter()
                .filter(|p| p.index > first_valley.index && p.index < second_valley.index)
                .max_by(|a, b| a.price.cmp(&b.price));

            if let Some(peak) = middle_peak {
                let height = peak.price - avg_price;
                let price_target = peak.price + height;

                patterns.push(ChartPattern {
                    pattern_type: ChartPatternType::DoubleBottom,
                    start_index: first_valley.index,
                    end_index: second_valley.index,
                    key_points: vec![first_valley.clone(), (*peak).clone(), second_valley.clone()],
                    trendlines: vec![],
                    price_target: Some(price_target),
                    confidence: 0.75,
                    bullish: true,
                    is_complete: true,
                    timestamp: klines[second_valley.index].open_time,
                });
            }
        }

        patterns
    }

    /// Triple Top/Bottom 패턴 감지.
    fn detect_triple_patterns(
        &self,
        klines: &[Kline],
        pivots: &[PatternPoint],
    ) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 3 || valleys.len() < 2 {
            return patterns;
        }

        let tolerance = Decimal::try_from(self.config.price_tolerance).unwrap_or(dec!(0.02));

        // Triple Top
        for i in 0..peaks.len().saturating_sub(2) {
            let p1 = peaks[i];
            let p2 = peaks[i + 1];
            let p3 = peaks[i + 2];

            let avg_price = (p1.price + p2.price + p3.price) / dec!(3);
            let max_diff = [
                (p1.price - avg_price).abs(),
                (p2.price - avg_price).abs(),
                (p3.price - avg_price).abs(),
            ]
            .into_iter()
            .max()
            .unwrap_or(Decimal::ZERO);

            if max_diff / avg_price > tolerance {
                continue;
            }

            let support = valleys
                .iter()
                .filter(|v| v.index > p1.index && v.index < p3.index)
                .map(|v| v.price)
                .min()
                .unwrap_or(Decimal::ZERO);

            let height = avg_price - support;
            let price_target = support - height;

            patterns.push(ChartPattern {
                pattern_type: ChartPatternType::TripleTop,
                start_index: p1.index,
                end_index: p3.index,
                key_points: vec![p1.clone(), p2.clone(), p3.clone()],
                trendlines: vec![],
                price_target: Some(price_target),
                confidence: 0.8,
                bullish: false,
                is_complete: true,
                timestamp: klines[p3.index].open_time,
            });
        }

        // Triple Bottom
        for i in 0..valleys.len().saturating_sub(2) {
            let v1 = valleys[i];
            let v2 = valleys[i + 1];
            let v3 = valleys[i + 2];

            let avg_price = (v1.price + v2.price + v3.price) / dec!(3);
            let max_diff = [
                (v1.price - avg_price).abs(),
                (v2.price - avg_price).abs(),
                (v3.price - avg_price).abs(),
            ]
            .into_iter()
            .max()
            .unwrap_or(Decimal::ZERO);

            if max_diff / avg_price > tolerance {
                continue;
            }

            let resistance = peaks
                .iter()
                .filter(|p| p.index > v1.index && p.index < v3.index)
                .map(|p| p.price)
                .max()
                .unwrap_or(Decimal::ZERO);

            let height = resistance - avg_price;
            let price_target = resistance + height;

            patterns.push(ChartPattern {
                pattern_type: ChartPatternType::TripleBottom,
                start_index: v1.index,
                end_index: v3.index,
                key_points: vec![v1.clone(), v2.clone(), v3.clone()],
                trendlines: vec![],
                price_target: Some(price_target),
                confidence: 0.8,
                bullish: true,
                is_complete: true,
                timestamp: klines[v3.index].open_time,
            });
        }

        patterns
    }

    /// Triangle 패턴 감지.
    fn detect_triangles(&self, klines: &[Kline], pivots: &[PatternPoint]) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 2 || valleys.len() < 2 {
            return patterns;
        }

        // 최근 피크/밸리로 추세선 그리기
        for i in 0..peaks.len().saturating_sub(1) {
            for j in 0..valleys.len().saturating_sub(1) {
                let peak1 = peaks[i];
                let peak2 = peaks[i + 1];
                let valley1 = valleys[j];
                let valley2 = valleys[j + 1];

                // 시간 순서 확인
                if peak2.index <= peak1.index || valley2.index <= valley1.index {
                    continue;
                }

                // 피크와 밸리가 겹치는 기간인지 확인
                let start = peak1.index.max(valley1.index);
                let end = peak2.index.min(valley2.index);
                if end <= start {
                    continue;
                }

                // 추세선 기울기 계산
                let peak_slope = self.calculate_slope(peak1, peak2);
                let valley_slope = self.calculate_slope(valley1, valley2);

                let slope_tolerance =
                    Decimal::try_from(self.config.slope_tolerance).unwrap_or(dec!(0.1));

                // Ascending Triangle (상단 평행, 하단 상승)
                if peak_slope.abs() < slope_tolerance && valley_slope > slope_tolerance {
                    patterns.push(ChartPattern {
                        pattern_type: ChartPatternType::AscendingTriangle,
                        start_index: start,
                        end_index: end,
                        key_points: vec![
                            peak1.clone(),
                            peak2.clone(),
                            valley1.clone(),
                            valley2.clone(),
                        ],
                        trendlines: vec![
                            Trendline {
                                start: peak1.clone(),
                                end: peak2.clone(),
                                slope: peak_slope,
                                line_type: "resistance".to_string(),
                            },
                            Trendline {
                                start: valley1.clone(),
                                end: valley2.clone(),
                                slope: valley_slope,
                                line_type: "support".to_string(),
                            },
                        ],
                        price_target: Some(peak1.price + (peak1.price - valley1.price)),
                        confidence: 0.7,
                        bullish: true,
                        is_complete: false,
                        timestamp: klines[end].open_time,
                    });
                }

                // Descending Triangle (하단 평행, 상단 하락)
                if valley_slope.abs() < slope_tolerance && peak_slope < -slope_tolerance {
                    patterns.push(ChartPattern {
                        pattern_type: ChartPatternType::DescendingTriangle,
                        start_index: start,
                        end_index: end,
                        key_points: vec![
                            peak1.clone(),
                            peak2.clone(),
                            valley1.clone(),
                            valley2.clone(),
                        ],
                        trendlines: vec![
                            Trendline {
                                start: peak1.clone(),
                                end: peak2.clone(),
                                slope: peak_slope,
                                line_type: "resistance".to_string(),
                            },
                            Trendline {
                                start: valley1.clone(),
                                end: valley2.clone(),
                                slope: valley_slope,
                                line_type: "support".to_string(),
                            },
                        ],
                        price_target: Some(valley1.price - (peak1.price - valley1.price)),
                        confidence: 0.7,
                        bullish: false,
                        is_complete: false,
                        timestamp: klines[end].open_time,
                    });
                }

                // Symmetrical Triangle (양쪽 수렴)
                if peak_slope < -slope_tolerance && valley_slope > slope_tolerance {
                    patterns.push(ChartPattern {
                        pattern_type: ChartPatternType::SymmetricalTriangle,
                        start_index: start,
                        end_index: end,
                        key_points: vec![
                            peak1.clone(),
                            peak2.clone(),
                            valley1.clone(),
                            valley2.clone(),
                        ],
                        trendlines: vec![
                            Trendline {
                                start: peak1.clone(),
                                end: peak2.clone(),
                                slope: peak_slope,
                                line_type: "resistance".to_string(),
                            },
                            Trendline {
                                start: valley1.clone(),
                                end: valley2.clone(),
                                slope: valley_slope,
                                line_type: "support".to_string(),
                            },
                        ],
                        price_target: None, // 방향 불확실
                        confidence: 0.65,
                        bullish: true, // 기존 추세 유지 가정
                        is_complete: false,
                        timestamp: klines[end].open_time,
                    });
                }
            }
        }

        patterns
    }

    /// Wedge 패턴 감지.
    fn detect_wedges(&self, klines: &[Kline], pivots: &[PatternPoint]) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 2 || valleys.len() < 2 {
            return patterns;
        }

        let slope_tolerance = Decimal::try_from(self.config.slope_tolerance).unwrap_or(dec!(0.1));

        for i in 0..peaks.len().saturating_sub(1) {
            for j in 0..valleys.len().saturating_sub(1) {
                let peak1 = peaks[i];
                let peak2 = peaks[i + 1];
                let valley1 = valleys[j];
                let valley2 = valleys[j + 1];

                if peak2.index <= peak1.index || valley2.index <= valley1.index {
                    continue;
                }

                let start = peak1.index.max(valley1.index);
                let end = peak2.index.min(valley2.index);
                if end <= start {
                    continue;
                }

                let peak_slope = self.calculate_slope(peak1, peak2);
                let valley_slope = self.calculate_slope(valley1, valley2);

                // Rising Wedge (양쪽 상승, 수렴)
                if peak_slope > slope_tolerance
                    && valley_slope > slope_tolerance
                    && peak_slope < valley_slope
                {
                    patterns.push(ChartPattern {
                        pattern_type: ChartPatternType::RisingWedge,
                        start_index: start,
                        end_index: end,
                        key_points: vec![
                            peak1.clone(),
                            peak2.clone(),
                            valley1.clone(),
                            valley2.clone(),
                        ],
                        trendlines: vec![
                            Trendline {
                                start: peak1.clone(),
                                end: peak2.clone(),
                                slope: peak_slope,
                                line_type: "resistance".to_string(),
                            },
                            Trendline {
                                start: valley1.clone(),
                                end: valley2.clone(),
                                slope: valley_slope,
                                line_type: "support".to_string(),
                            },
                        ],
                        price_target: Some(valley1.price),
                        confidence: 0.7,
                        bullish: false, // 보통 하락 반전
                        is_complete: false,
                        timestamp: klines[end].open_time,
                    });
                }

                // Falling Wedge (양쪽 하락, 수렴)
                if peak_slope < -slope_tolerance
                    && valley_slope < -slope_tolerance
                    && peak_slope > valley_slope
                {
                    patterns.push(ChartPattern {
                        pattern_type: ChartPatternType::FallingWedge,
                        start_index: start,
                        end_index: end,
                        key_points: vec![
                            peak1.clone(),
                            peak2.clone(),
                            valley1.clone(),
                            valley2.clone(),
                        ],
                        trendlines: vec![
                            Trendline {
                                start: peak1.clone(),
                                end: peak2.clone(),
                                slope: peak_slope,
                                line_type: "resistance".to_string(),
                            },
                            Trendline {
                                start: valley1.clone(),
                                end: valley2.clone(),
                                slope: valley_slope,
                                line_type: "support".to_string(),
                            },
                        ],
                        price_target: Some(peak1.price),
                        confidence: 0.7,
                        bullish: true, // 보통 상승 반전
                        is_complete: false,
                        timestamp: klines[end].open_time,
                    });
                }
            }
        }

        patterns
    }

    /// Channel 패턴 감지.
    fn detect_channels(&self, klines: &[Kline], pivots: &[PatternPoint]) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 2 || valleys.len() < 2 {
            return patterns;
        }

        let slope_tolerance = Decimal::try_from(self.config.slope_tolerance).unwrap_or(dec!(0.1));

        for i in 0..peaks.len().saturating_sub(1) {
            for j in 0..valleys.len().saturating_sub(1) {
                let peak1 = peaks[i];
                let peak2 = peaks[i + 1];
                let valley1 = valleys[j];
                let valley2 = valleys[j + 1];

                if peak2.index <= peak1.index || valley2.index <= valley1.index {
                    continue;
                }

                let start = peak1.index.max(valley1.index);
                let end = peak2.index.min(valley2.index);
                if end <= start {
                    continue;
                }

                let peak_slope = self.calculate_slope(peak1, peak2);
                let valley_slope = self.calculate_slope(valley1, valley2);

                // 채널: 양 추세선이 평행 (기울기 차이가 작음)
                let slope_diff = (peak_slope - valley_slope).abs();
                if slope_diff > slope_tolerance {
                    continue;
                }

                let pattern_type = if peak_slope > slope_tolerance {
                    ChartPatternType::AscendingChannel
                } else if peak_slope < -slope_tolerance {
                    ChartPatternType::DescendingChannel
                } else {
                    ChartPatternType::HorizontalChannel
                };

                patterns.push(ChartPattern {
                    pattern_type,
                    start_index: start,
                    end_index: end,
                    key_points: vec![
                        peak1.clone(),
                        peak2.clone(),
                        valley1.clone(),
                        valley2.clone(),
                    ],
                    trendlines: vec![
                        Trendline {
                            start: peak1.clone(),
                            end: peak2.clone(),
                            slope: peak_slope,
                            line_type: "resistance".to_string(),
                        },
                        Trendline {
                            start: valley1.clone(),
                            end: valley2.clone(),
                            slope: valley_slope,
                            line_type: "support".to_string(),
                        },
                    ],
                    price_target: None,
                    confidence: 0.65,
                    bullish: peak_slope > Decimal::ZERO,
                    is_complete: false,
                    timestamp: klines[end].open_time,
                });
            }
        }

        patterns
    }

    /// Flag/Pennant 패턴 감지.
    fn detect_flags_pennants(
        &self,
        klines: &[Kline],
        pivots: &[PatternPoint],
    ) -> Vec<ChartPattern> {
        let mut patterns = Vec::new();

        if klines.len() < 20 {
            return patterns;
        }

        // Flag/Pennant는 강한 움직임 후 짧은 조정 패턴
        // 여기서는 단순화된 감지 로직 사용

        let peaks: Vec<&PatternPoint> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<&PatternPoint> =
            pivots.iter().filter(|p| p.point_type == "valley").collect();

        if peaks.len() < 2 || valleys.len() < 2 {
            return patterns;
        }

        // 최근 강한 상승 후 조정 (Bullish Flag)
        for i in 10..klines.len() {
            let lookback = 10.min(i);
            let prior_move = klines[i].close - klines[i - lookback].close;

            // 강한 상승 (5% 이상)
            if prior_move / klines[i - lookback].close > dec!(0.05) {
                // 이후 작은 하락 채널 형성 확인
                let recent_peaks: Vec<_> = peaks
                    .iter()
                    .filter(|p| p.index > i - lookback && p.index <= i)
                    .collect();
                let recent_valleys: Vec<_> = valleys
                    .iter()
                    .filter(|v| v.index > i - lookback && v.index <= i)
                    .collect();

                if recent_peaks.len() >= 2 && recent_valleys.len() >= 2 {
                    let peak_slope =
                        self.calculate_slope(recent_peaks[0], recent_peaks[recent_peaks.len() - 1]);
                    let valley_slope = self.calculate_slope(
                        recent_valleys[0],
                        recent_valleys[recent_valleys.len() - 1],
                    );

                    // 평행 하락 채널 = Flag
                    let slope_tolerance =
                        Decimal::try_from(self.config.slope_tolerance).unwrap_or(dec!(0.1));
                    if peak_slope < Decimal::ZERO
                        && valley_slope < Decimal::ZERO
                        && (peak_slope - valley_slope).abs() < slope_tolerance
                    {
                        patterns.push(ChartPattern {
                            pattern_type: ChartPatternType::BullishFlag,
                            start_index: i - lookback,
                            end_index: i,
                            key_points: vec![],
                            trendlines: vec![],
                            price_target: Some(klines[i].close + prior_move),
                            confidence: 0.65,
                            bullish: true,
                            is_complete: false,
                            timestamp: klines[i].open_time,
                        });
                    }

                    // 수렴 = Pennant
                    if peak_slope < Decimal::ZERO && valley_slope > Decimal::ZERO {
                        patterns.push(ChartPattern {
                            pattern_type: ChartPatternType::BullishPennant,
                            start_index: i - lookback,
                            end_index: i,
                            key_points: vec![],
                            trendlines: vec![],
                            price_target: Some(klines[i].close + prior_move),
                            confidence: 0.65,
                            bullish: true,
                            is_complete: false,
                            timestamp: klines[i].open_time,
                        });
                    }
                }
            }

            // 강한 하락 후 조정 (Bearish Flag)
            if prior_move / klines[i - lookback].close < dec!(-0.05) {
                let recent_peaks: Vec<_> = peaks
                    .iter()
                    .filter(|p| p.index > i - lookback && p.index <= i)
                    .collect();
                let recent_valleys: Vec<_> = valleys
                    .iter()
                    .filter(|v| v.index > i - lookback && v.index <= i)
                    .collect();

                if recent_peaks.len() >= 2 && recent_valleys.len() >= 2 {
                    let peak_slope =
                        self.calculate_slope(recent_peaks[0], recent_peaks[recent_peaks.len() - 1]);
                    let valley_slope = self.calculate_slope(
                        recent_valleys[0],
                        recent_valleys[recent_valleys.len() - 1],
                    );

                    let slope_tolerance =
                        Decimal::try_from(self.config.slope_tolerance).unwrap_or(dec!(0.1));
                    if peak_slope > Decimal::ZERO
                        && valley_slope > Decimal::ZERO
                        && (peak_slope - valley_slope).abs() < slope_tolerance
                    {
                        patterns.push(ChartPattern {
                            pattern_type: ChartPatternType::BearishFlag,
                            start_index: i - lookback,
                            end_index: i,
                            key_points: vec![],
                            trendlines: vec![],
                            price_target: Some(klines[i].close + prior_move),
                            confidence: 0.65,
                            bullish: false,
                            is_complete: false,
                            timestamp: klines[i].open_time,
                        });
                    }

                    if peak_slope < Decimal::ZERO && valley_slope > Decimal::ZERO {
                        patterns.push(ChartPattern {
                            pattern_type: ChartPatternType::BearishPennant,
                            start_index: i - lookback,
                            end_index: i,
                            key_points: vec![],
                            trendlines: vec![],
                            price_target: Some(klines[i].close + prior_move),
                            confidence: 0.65,
                            bullish: false,
                            is_complete: false,
                            timestamp: klines[i].open_time,
                        });
                    }
                }
            }
        }

        patterns
    }

    // ==================== 헬퍼 메서드 ====================

    /// Doji 여부 확인.
    fn is_doji(&self, candle: &Kline) -> bool {
        let range = candle.range();
        if range.is_zero() {
            return true;
        }
        let body_ratio = candle.body_size() / range;
        body_ratio < Decimal::try_from(self.config.doji_body_ratio).unwrap_or(dec!(0.1))
    }

    /// Doji 신뢰도 계산.
    fn calculate_doji_confidence(&self, body_ratio: Decimal) -> f64 {
        // 몸통이 작을수록 신뢰도 증가
        let ratio_f64 = body_ratio.to_string().parse::<f64>().unwrap_or(0.1);
        (1.0 - ratio_f64 / self.config.doji_body_ratio).clamp(0.5, 0.95)
    }

    /// Shadow 패턴 신뢰도 계산.
    fn calculate_shadow_confidence(&self, shadow: Decimal, body: Decimal) -> f64 {
        if body.is_zero() {
            return 0.6;
        }
        let ratio = shadow / body;
        let ratio_f64 = ratio.to_string().parse::<f64>().unwrap_or(2.0);
        (ratio_f64 / self.config.shadow_body_ratio * 0.4 + 0.5).min(0.9)
    }

    /// Engulfing 신뢰도 계산.
    fn calculate_engulfing_confidence(&self, ratio: Decimal) -> f64 {
        let ratio_f64 = ratio.to_string().parse::<f64>().unwrap_or(1.0);
        (0.5 + ratio_f64 * 0.1).min(0.9)
    }

    /// 두 점 사이의 기울기 계산.
    fn calculate_slope(&self, p1: &PatternPoint, p2: &PatternPoint) -> Decimal {
        let x_diff = Decimal::from(p2.index as i64 - p1.index as i64);
        if x_diff.is_zero() {
            return Decimal::ZERO;
        }
        (p2.price - p1.price) / x_diff
    }
}

// ==================== 테스트 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_kline(
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
        index: i64,
    ) -> Kline {
        let time =
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::hours(index);
        Kline {
            ticker: "BTC/USDT".to_string(),
            timeframe: Timeframe::H1,
            open_time: time,
            open,
            high,
            low,
            close,
            volume: dec!(1000),
            close_time: time + chrono::Duration::hours(1),
            quote_volume: None,
            num_trades: None,
        }
    }

    #[test]
    fn test_doji_detection() {
        let recognizer = PatternRecognizer::with_defaults();

        // Doji: 시가 = 종가
        let klines = vec![create_test_kline(
            dec!(100),
            dec!(105),
            dec!(95),
            dec!(100),
            0,
        )];

        let patterns = recognizer.detect_candlestick_patterns(&klines);
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| matches!(
            p.pattern_type,
            CandlestickPatternType::Doji
                | CandlestickPatternType::DragonflyDoji
                | CandlestickPatternType::GravestoneDoji
        )));
    }

    #[test]
    fn test_hammer_detection() {
        let recognizer = PatternRecognizer::with_defaults();

        // 이전 하락 추세 + Hammer
        let klines = vec![
            create_test_kline(dec!(110), dec!(112), dec!(108), dec!(105), 0),
            create_test_kline(dec!(105), dec!(106), dec!(100), dec!(101), 1),
            create_test_kline(dec!(101), dec!(102), dec!(95), dec!(98), 2),
            // Hammer: 짧은 몸통, 긴 아래꼬리
            create_test_kline(dec!(100), dec!(101), dec!(90), dec!(100.5), 3),
        ];

        let patterns = recognizer.detect_candlestick_patterns(&klines);
        let hammer = patterns
            .iter()
            .find(|p| p.pattern_type == CandlestickPatternType::Hammer);
        assert!(hammer.is_some());
        assert!(hammer.unwrap().bullish);
    }

    #[test]
    fn test_engulfing_detection() {
        let recognizer = PatternRecognizer::with_defaults();

        // Bullish Engulfing
        let klines = vec![
            create_test_kline(dec!(105), dec!(106), dec!(100), dec!(101), 0), // 음봉
            create_test_kline(dec!(99), dec!(108), dec!(98), dec!(107), 1),   // 큰 양봉
        ];

        let patterns = recognizer.detect_candlestick_patterns(&klines);
        let engulfing = patterns
            .iter()
            .find(|p| p.pattern_type == CandlestickPatternType::BullishEngulfing);
        assert!(engulfing.is_some());
        assert!(engulfing.unwrap().bullish);
    }

    #[test]
    fn test_morning_star_detection() {
        let recognizer = PatternRecognizer::with_defaults();

        // Morning Star: 음봉 + 작은 캔들 + 양봉
        let klines = vec![
            create_test_kline(dec!(110), dec!(111), dec!(100), dec!(101), 0), // 큰 음봉
            create_test_kline(dec!(101), dec!(102), dec!(99), dec!(100), 1),  // 작은 캔들
            create_test_kline(dec!(102), dec!(115), dec!(101), dec!(112), 2), // 큰 양봉
        ];

        let patterns = recognizer.detect_candlestick_patterns(&klines);
        let morning_star = patterns
            .iter()
            .find(|p| p.pattern_type == CandlestickPatternType::MorningStar);
        assert!(morning_star.is_some());
    }

    #[test]
    fn test_three_white_soldiers() {
        let recognizer = PatternRecognizer::with_defaults();

        // Three White Soldiers: 연속 3 양봉
        let klines = vec![
            create_test_kline(dec!(100), dec!(105), dec!(99), dec!(104), 0),
            create_test_kline(dec!(103), dec!(110), dec!(102), dec!(109), 1),
            create_test_kline(dec!(108), dec!(116), dec!(107), dec!(115), 2),
        ];

        let patterns = recognizer.detect_candlestick_patterns(&klines);
        let soldiers = patterns
            .iter()
            .find(|p| p.pattern_type == CandlestickPatternType::ThreeWhiteSoldiers);
        assert!(soldiers.is_some());
        assert!(soldiers.unwrap().bullish);
    }

    #[test]
    fn test_pivot_detection() {
        let recognizer = PatternRecognizer::new(PatternConfig {
            pivot_lookback: 2,
            ..Default::default()
        });

        // 피크와 밸리가 명확한 데이터
        let klines = vec![
            create_test_kline(dec!(100), dec!(102), dec!(99), dec!(101), 0),
            create_test_kline(dec!(101), dec!(105), dec!(100), dec!(104), 1),
            create_test_kline(dec!(104), dec!(110), dec!(103), dec!(108), 2), // 피크
            create_test_kline(dec!(108), dec!(109), dec!(102), dec!(103), 3),
            create_test_kline(dec!(103), dec!(104), dec!(95), dec!(96), 4), // 밸리
            create_test_kline(dec!(96), dec!(100), dec!(95), dec!(99), 5),
            create_test_kline(dec!(99), dec!(108), dec!(98), dec!(107), 6),
        ];

        let pivots = recognizer.detect_pivots(&klines);
        assert!(!pivots.is_empty());

        let peaks: Vec<_> = pivots.iter().filter(|p| p.point_type == "peak").collect();
        let valleys: Vec<_> = pivots.iter().filter(|p| p.point_type == "valley").collect();

        assert!(!peaks.is_empty() || !valleys.is_empty());
    }

    #[test]
    fn test_pattern_config_defaults() {
        let config = PatternConfig::default();
        assert_eq!(config.doji_body_ratio, 0.1);
        assert_eq!(config.shadow_body_ratio, 2.0);
        assert_eq!(config.pivot_lookback, 5);
        assert_eq!(config.min_confidence, 0.6);
    }

    #[test]
    fn test_empty_klines() {
        let recognizer = PatternRecognizer::with_defaults();
        let patterns = recognizer.detect_candlestick_patterns(&[]);
        assert!(patterns.is_empty());

        let chart_patterns = recognizer.detect_chart_patterns(&[]);
        assert!(chart_patterns.is_empty());
    }
}
