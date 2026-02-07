//! 상관계수 계산 모듈.
//!
//! 종목 간 가격 움직임의 상관관계를 계산합니다.
//! 포트폴리오 분산 투자 및 리스크 관리에 활용됩니다.
//!
//! # 주요 기능
//!
//! - **Pearson 상관계수**: 두 종목 간 선형 상관관계 측정
//! - **상관행렬**: 여러 종목 간 상관관계를 N×N 행렬로 표현
//!
//! # 예시
//!
//! ```rust,ignore
//! use trader_analytics::correlation::{CorrelationMatrix, calculate_correlation};
//!
//! let returns_a = vec![0.01, -0.02, 0.015, 0.005];
//! let returns_b = vec![0.008, -0.015, 0.012, 0.003];
//!
//! let corr = calculate_correlation(&returns_a, &returns_b);
//! println!("상관계수: {:.4}", corr.unwrap_or(0.0));
//! ```

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 상관행렬 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    /// 종목 목록 (행/열 순서)
    pub symbols: Vec<String>,
    /// 상관계수 행렬 (N×N, -1.0 ~ 1.0)
    pub matrix: Vec<Vec<f64>>,
    /// 분석 기간 (일수)
    pub period: usize,
}

/// Pearson 상관계수 계산.
///
/// 두 수익률 시계열 간의 상관계수를 계산합니다.
///
/// # 인자
///
/// * `x` - 첫 번째 수익률 시계열
/// * `y` - 두 번째 수익률 시계열
///
/// # 반환
///
/// 상관계수 (-1.0 ~ 1.0), 데이터 부족 시 None
pub fn calculate_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let n = x.len() as f64;

    // 평균 계산
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    // 공분산 및 표준편차 계산
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    // 표준편차가 0인 경우 (변동 없음)
    if var_x == 0.0 || var_y == 0.0 {
        return None;
    }

    let std_x = var_x.sqrt();
    let std_y = var_y.sqrt();

    Some(cov / (std_x * std_y))
}

/// Decimal 수익률로 상관계수 계산.
pub fn calculate_correlation_decimal(x: &[Decimal], y: &[Decimal]) -> Option<f64> {
    let x_f64: Vec<f64> = x.iter().filter_map(|d| d.to_f64()).collect();
    let y_f64: Vec<f64> = y.iter().filter_map(|d| d.to_f64()).collect();
    calculate_correlation(&x_f64, &y_f64)
}

/// 가격 시계열을 수익률로 변환.
///
/// # 인자
///
/// * `prices` - 가격 시계열 (시간순)
///
/// # 반환
///
/// 일간 수익률 벡터 (길이: prices.len() - 1)
pub fn prices_to_returns(prices: &[f64]) -> Vec<f64> {
    if prices.len() < 2 {
        return Vec::new();
    }

    prices
        .windows(2)
        .map(|w| {
            if w[0] == 0.0 {
                0.0
            } else {
                (w[1] - w[0]) / w[0]
            }
        })
        .collect()
}

/// Decimal 가격을 수익률로 변환.
pub fn prices_to_returns_decimal(prices: &[Decimal]) -> Vec<f64> {
    let prices_f64: Vec<f64> = prices.iter().filter_map(|d| d.to_f64()).collect();
    prices_to_returns(&prices_f64)
}

/// 상관행렬 계산.
///
/// 여러 종목의 가격 데이터를 받아 상관행렬을 계산합니다.
///
/// # 인자
///
/// * `prices` - 종목별 가격 데이터 (HashMap<종목코드, 가격벡터>)
/// * `symbols` - 행렬에 포함할 종목 순서 (지정하지 않으면 HashMap 키 순서)
///
/// # 반환
///
/// 상관행렬 결과
pub fn calculate_correlation_matrix(
    prices: &HashMap<String, Vec<f64>>,
    symbols: Option<Vec<String>>,
) -> Option<CorrelationMatrix> {
    if prices.is_empty() {
        return None;
    }

    // 종목 순서 결정
    let symbol_list: Vec<String> = symbols.unwrap_or_else(|| {
        let mut keys: Vec<String> = prices.keys().cloned().collect();
        keys.sort();
        keys
    });

    let n = symbol_list.len();
    if n == 0 {
        return None;
    }

    // 수익률 변환
    let returns: HashMap<String, Vec<f64>> = symbol_list
        .iter()
        .filter_map(|s| prices.get(s).map(|p| (s.clone(), prices_to_returns(p))))
        .collect();

    // 최소 데이터 길이 확인
    let min_len = returns.values().map(|r| r.len()).min().unwrap_or(0);
    if min_len < 5 {
        return None;
    }

    // 상관행렬 계산
    let mut matrix = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in 0..n {
            if i == j {
                // 자기 자신과의 상관계수는 1.0
                matrix[i][j] = 1.0;
            } else if i < j {
                // 상삼각 행렬만 계산
                let corr =
                    calculate_correlation(&returns[&symbol_list[i]], &returns[&symbol_list[j]])
                        .unwrap_or(0.0);
                matrix[i][j] = corr;
                matrix[j][i] = corr; // 대칭
            }
        }
    }

    Some(CorrelationMatrix {
        symbols: symbol_list,
        matrix,
        period: min_len + 1, // 수익률 길이 + 1 = 가격 데이터 길이
    })
}

/// Decimal 가격으로 상관행렬 계산.
pub fn calculate_correlation_matrix_decimal(
    prices: &HashMap<String, Vec<Decimal>>,
    symbols: Option<Vec<String>>,
) -> Option<CorrelationMatrix> {
    let prices_f64: HashMap<String, Vec<f64>> = prices
        .iter()
        .map(|(k, v)| {
            let f64_vec: Vec<f64> = v.iter().filter_map(|d| d.to_f64()).collect();
            (k.clone(), f64_vec)
        })
        .collect();
    calculate_correlation_matrix(&prices_f64, symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_correlation_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let corr = calculate_correlation(&x, &y);
        assert!(corr.is_some());
        assert!((corr.unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_correlation_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let corr = calculate_correlation(&x, &y);
        assert!(corr.is_some());
        assert!((corr.unwrap() + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_correlation_zero() {
        // 상관관계 없는 데이터
        let x = vec![1.0, 2.0, 3.0, 2.0, 1.0];
        let y = vec![3.0, 1.0, 3.0, 1.0, 3.0];
        let corr = calculate_correlation(&x, &y);
        assert!(corr.is_some());
        // 정확히 0은 아니지만 0에 가까움
        assert!(corr.unwrap().abs() < 0.5);
    }

    #[test]
    fn test_correlation_insufficient_data() {
        let x = vec![1.0];
        let y = vec![2.0];
        assert!(calculate_correlation(&x, &y).is_none());
    }

    #[test]
    fn test_correlation_length_mismatch() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![1.0, 2.0];
        assert!(calculate_correlation(&x, &y).is_none());
    }

    #[test]
    fn test_prices_to_returns() {
        let prices = vec![100.0, 101.0, 99.0, 102.0];
        let returns = prices_to_returns(&prices);
        assert_eq!(returns.len(), 3);
        assert!((returns[0] - 0.01).abs() < 0.001); // (101-100)/100
    }

    #[test]
    fn test_correlation_matrix() {
        let mut prices = HashMap::new();
        // A: 변동 있는 상승 추세
        prices.insert(
            "A".to_string(),
            vec![100.0, 105.0, 102.0, 110.0, 108.0, 115.0, 120.0],
        );
        // B: A와 비례하여 움직임 (A * 0.5 + 약간의 차이)
        prices.insert(
            "B".to_string(),
            vec![50.0, 52.5, 51.0, 55.0, 54.0, 57.5, 60.0],
        );
        // C: A와 반대로 움직임
        prices.insert(
            "C".to_string(),
            vec![120.0, 115.0, 118.0, 110.0, 112.0, 105.0, 100.0],
        );

        let matrix = calculate_correlation_matrix(
            &prices,
            Some(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
        );
        assert!(matrix.is_some(), "Matrix should be Some");
        let m = matrix.unwrap();
        assert_eq!(m.symbols.len(), 3);
        assert_eq!(m.matrix.len(), 3);

        // 대각선은 1.0
        assert!(
            (m.matrix[0][0] - 1.0).abs() < 0.001,
            "Diagonal A should be 1.0"
        );
        assert!(
            (m.matrix[1][1] - 1.0).abs() < 0.001,
            "Diagonal B should be 1.0"
        );
        assert!(
            (m.matrix[2][2] - 1.0).abs() < 0.001,
            "Diagonal C should be 1.0"
        );

        // A와 B는 양의 상관 (둘 다 비슷하게 움직임)
        assert!(
            m.matrix[0][1] > 0.8,
            "A-B correlation should be high positive, got: {}",
            m.matrix[0][1]
        );

        // A와 C는 음의 상관 (반대로 움직임)
        assert!(
            m.matrix[0][2] < -0.5,
            "A-C correlation should be negative, got: {}",
            m.matrix[0][2]
        );

        // 대칭 확인
        assert!(
            (m.matrix[0][1] - m.matrix[1][0]).abs() < 0.001,
            "Matrix should be symmetric"
        );
        assert!(
            (m.matrix[0][2] - m.matrix[2][0]).abs() < 0.001,
            "Matrix should be symmetric"
        );
    }

    #[test]
    fn test_correlation_decimal() {
        let x = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0)];
        let y = vec![dec!(2.0), dec!(4.0), dec!(6.0), dec!(8.0)];
        let corr = calculate_correlation_decimal(&x, &y);
        assert!(corr.is_some());
        assert!((corr.unwrap() - 1.0).abs() < 0.001);
    }
}
