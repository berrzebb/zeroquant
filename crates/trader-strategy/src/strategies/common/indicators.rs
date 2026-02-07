//! 공용 기술적 지표 계산 함수.
//!
//! 이 모듈은 여러 전략에서 공통적으로 사용되는 기술적 지표 계산 함수를 제공합니다.
//! 모든 함수는 거래소 중립적이며, OHLCV 데이터만을 입력으로 받습니다.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// RSI (Relative Strength Index) 계산.
///
/// # Arguments
/// * `prices` - 종가 데이터 (최신 데이터가 마지막)
/// * `period` - RSI 기간 (일반적으로 14)
///
/// # Returns
/// RSI 값 (0~100), 데이터 부족 시 None
pub fn calculate_rsi(prices: &[Decimal], period: usize) -> Option<Decimal> {
    if prices.len() < period + 1 {
        return None;
    }

    let mut gains = dec!(0);
    let mut losses = dec!(0);

    // 초기 평균 계산
    for i in 1..=period {
        let change = prices[i] - prices[i - 1];
        if change > dec!(0) {
            gains += change;
        } else {
            losses += change.abs();
        }
    }

    let mut avg_gain = gains / Decimal::from(period);
    let mut avg_loss = losses / Decimal::from(period);

    // EMA 방식으로 나머지 기간 계산
    for i in (period + 1)..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > dec!(0) {
            avg_gain = (avg_gain * Decimal::from(period - 1) + change) / Decimal::from(period);
            avg_loss = (avg_loss * Decimal::from(period - 1)) / Decimal::from(period);
        } else {
            avg_gain = (avg_gain * Decimal::from(period - 1)) / Decimal::from(period);
            avg_loss =
                (avg_loss * Decimal::from(period - 1) + change.abs()) / Decimal::from(period);
        }
    }

    if avg_loss == dec!(0) {
        return Some(dec!(100));
    }

    let rs = avg_gain / avg_loss;
    let rsi = dec!(100) - (dec!(100) / (dec!(1) + rs));

    Some(rsi)
}

/// SMA (Simple Moving Average) 계산.
///
/// # Arguments
/// * `prices` - 가격 데이터
/// * `period` - 이동평균 기간
///
/// # Returns
/// SMA 값, 데이터 부족 시 None
pub fn calculate_sma(prices: &[Decimal], period: usize) -> Option<Decimal> {
    if prices.len() < period {
        return None;
    }

    let sum: Decimal = prices[prices.len() - period..].iter().sum();
    Some(sum / Decimal::from(period))
}

/// EMA (Exponential Moving Average) 계산.
///
/// # Arguments
/// * `prices` - 가격 데이터
/// * `period` - 이동평균 기간
///
/// # Returns
/// EMA 값, 데이터 부족 시 None
pub fn calculate_ema(prices: &[Decimal], period: usize) -> Option<Decimal> {
    if prices.len() < period {
        return None;
    }

    let multiplier = dec!(2) / Decimal::from(period + 1);
    let mut ema = calculate_sma(&prices[0..period], period)?;

    for &price in &prices[period..] {
        ema = (price - ema) * multiplier + ema;
    }

    Some(ema)
}

/// 볼린저 밴드 계산 결과.
#[derive(Debug, Clone)]
pub struct BollingerBands {
    /// 상단 밴드
    pub upper: Decimal,
    /// 중간선 (SMA)
    pub middle: Decimal,
    /// 하단 밴드
    pub lower: Decimal,
    /// 밴드 폭 (upper - lower)
    pub width: Decimal,
}

/// 볼린저 밴드 계산.
///
/// # Arguments
/// * `prices` - 종가 데이터
/// * `period` - 기간 (일반적으로 20)
/// * `std_dev` - 표준편차 배수 (일반적으로 2.0)
///
/// # Returns
/// 볼린저 밴드, 데이터 부족 시 None
pub fn calculate_bollinger_bands(
    prices: &[Decimal],
    period: usize,
    std_dev: Decimal,
) -> Option<BollingerBands> {
    if prices.len() < period {
        return None;
    }

    let middle = calculate_sma(prices, period)?;
    let recent_prices = &prices[prices.len() - period..];

    // 표준편차 계산
    let variance: Decimal = recent_prices
        .iter()
        .map(|&p| {
            let diff = p - middle;
            diff * diff
        })
        .sum::<Decimal>()
        / Decimal::from(period);

    // rust_decimal에는 sqrt가 없으므로 f64로 변환하여 계산
    let std = variance
        .to_f64()
        .and_then(|v: f64| Decimal::try_from(v.sqrt()).ok())
        .unwrap_or(dec!(0));
    let band_width = std * std_dev;

    Some(BollingerBands {
        upper: middle + band_width,
        middle,
        lower: middle - band_width,
        width: band_width * dec!(2),
    })
}

/// MACD 계산 결과.
#[derive(Debug, Clone)]
pub struct MacdResult {
    /// MACD 선 (Fast EMA - Slow EMA)
    pub macd: Decimal,
    /// 시그널 선 (MACD의 EMA)
    pub signal: Decimal,
    /// 히스토그램 (MACD - Signal)
    pub histogram: Decimal,
}

/// MACD (Moving Average Convergence Divergence) 계산.
///
/// # Arguments
/// * `prices` - 종가 데이터
/// * `fast_period` - 단기 EMA 기간 (일반적으로 12)
/// * `slow_period` - 장기 EMA 기간 (일반적으로 26)
/// * `signal_period` - 시그널 EMA 기간 (일반적으로 9)
///
/// # Returns
/// MACD 결과, 데이터 부족 시 None
pub fn calculate_macd(
    prices: &[Decimal],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> Option<MacdResult> {
    if prices.len() < slow_period + signal_period {
        return None;
    }

    let fast_ema = calculate_ema(prices, fast_period)?;
    let slow_ema = calculate_ema(prices, slow_period)?;
    let macd = fast_ema - slow_ema;

    // MACD 라인의 히스토리 계산 (시그널 EMA를 위해)
    let mut macd_values = Vec::new();
    for i in slow_period..prices.len() {
        let window = &prices[0..=i];
        if let (Some(f), Some(s)) = (
            calculate_ema(window, fast_period),
            calculate_ema(window, slow_period),
        ) {
            macd_values.push(f - s);
        }
    }

    let signal = calculate_ema(&macd_values, signal_period)?;
    let histogram = macd - signal;

    Some(MacdResult {
        macd,
        signal,
        histogram,
    })
}

/// ATR (Average True Range) 계산.
///
/// # Arguments
/// * `highs` - 고가 데이터
/// * `lows` - 저가 데이터
/// * `closes` - 종가 데이터
/// * `period` - ATR 기간 (일반적으로 14)
///
/// # Returns
/// ATR 값, 데이터 부족 시 None
pub fn calculate_atr(
    highs: &[Decimal],
    lows: &[Decimal],
    closes: &[Decimal],
    period: usize,
) -> Option<Decimal> {
    if highs.len() < period + 1 || lows.len() < period + 1 || closes.len() < period + 1 {
        return None;
    }

    let mut true_ranges = Vec::new();

    for i in 1..=period {
        let tr1 = highs[i] - lows[i];
        let tr2 = (highs[i] - closes[i - 1]).abs();
        let tr3 = (lows[i] - closes[i - 1]).abs();
        let tr = tr1.max(tr2).max(tr3);
        true_ranges.push(tr);
    }

    let mut atr = true_ranges.iter().sum::<Decimal>() / Decimal::from(period);

    // EMA 방식으로 나머지 계산
    for i in (period + 1)..highs.len() {
        let tr1 = highs[i] - lows[i];
        let tr2 = (highs[i] - closes[i - 1]).abs();
        let tr3 = (lows[i] - closes[i - 1]).abs();
        let tr = tr1.max(tr2).max(tr3);

        atr = (atr * Decimal::from(period - 1) + tr) / Decimal::from(period);
    }

    Some(atr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_sma() {
        let prices = vec![dec!(10), dec!(11), dec!(12), dec!(13), dec!(14)];
        let sma = calculate_sma(&prices, 3).unwrap();
        assert_eq!(sma, dec!(13)); // (12 + 13 + 14) / 3
    }

    #[test]
    fn test_calculate_rsi() {
        let prices = vec![
            dec!(44.34),
            dec!(44.09),
            dec!(44.15),
            dec!(43.61),
            dec!(44.33),
            dec!(44.83),
            dec!(45.10),
            dec!(45.42),
            dec!(45.84),
            dec!(46.08),
            dec!(45.89),
            dec!(46.03),
            dec!(45.61),
            dec!(46.28),
            dec!(46.28),
        ];

        let rsi = calculate_rsi(&prices, 14);
        assert!(rsi.is_some());
        let rsi_value = rsi.unwrap();
        assert!(rsi_value >= dec!(0) && rsi_value <= dec!(100));
    }

    #[test]
    fn test_calculate_bollinger_bands() {
        let prices = vec![
            dec!(20),
            dec!(21),
            dec!(22),
            dec!(21),
            dec!(20),
            dec!(19),
            dec!(20),
            dec!(21),
            dec!(22),
            dec!(23),
            dec!(22),
            dec!(21),
            dec!(20),
            dec!(21),
            dec!(22),
            dec!(23),
            dec!(24),
            dec!(23),
            dec!(22),
            dec!(21),
        ];

        let bb = calculate_bollinger_bands(&prices, 20, dec!(2));
        assert!(bb.is_some());
        let bands = bb.unwrap();
        assert!(bands.upper > bands.middle);
        assert!(bands.middle > bands.lower);
    }

    #[test]
    fn test_calculate_macd() {
        let prices: Vec<Decimal> = (0..50).map(|i| Decimal::from(100 + i)).collect();

        let macd = calculate_macd(&prices, 12, 26, 9);
        assert!(macd.is_some());
    }

    #[test]
    fn test_calculate_atr() {
        let highs = vec![dec!(50), dec!(51), dec!(52), dec!(53), dec!(54), dec!(55)];
        let lows = vec![dec!(48), dec!(49), dec!(50), dec!(51), dec!(52), dec!(53)];
        let closes = vec![dec!(49), dec!(50), dec!(51), dec!(52), dec!(53), dec!(54)];

        let atr = calculate_atr(&highs, &lows, &closes, 5);
        assert!(atr.is_some());
        assert!(atr.unwrap() > dec!(0));
    }
}
