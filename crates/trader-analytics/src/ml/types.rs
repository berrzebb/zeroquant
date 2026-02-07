//! ML 모듈의 공통 타입.

use serde::{Deserialize, Serialize};

/// 예측된 가격 이동 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PredictionDirection {
    /// 가격 상승 예상
    Up,
    /// 가격 하락 예상
    Down,
    /// 가격이 비교적 안정적으로 유지될 것으로 예상
    Sideways,
}

impl PredictionDirection {
    /// 연속 값(-1.0 ~ 1.0)에서 방향으로 변환.
    ///
    /// # 인자
    /// * `value` - 양수는 상승, 음수는 하락을 의미하는 prediction 값
    /// * `threshold` - Sideways가 아닌 것으로 간주할 최소 절대값 (기본값: 0.3)
    pub fn from_value(value: f32, threshold: f32) -> Self {
        if value > threshold {
            PredictionDirection::Up
        } else if value < -threshold {
            PredictionDirection::Down
        } else {
            PredictionDirection::Sideways
        }
    }

    /// 계산을 위해 방향을 숫자 값으로 변환.
    pub fn to_numeric(&self) -> f32 {
        match self {
            PredictionDirection::Up => 1.0,
            PredictionDirection::Down => -1.0,
            PredictionDirection::Sideways => 0.0,
        }
    }
}

/// ML 모델 입력을 위한 feature vector.
///
/// 시장 데이터에서 추출한 feature를 나타내는 f32 값 벡터를 감쌈
/// (예: 기술 지표, 가격 비율).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    /// feature 값들
    values: Vec<f32>,
    /// 디버깅/로깅을 위한 선택적 feature 이름
    names: Option<Vec<String>>,
}

impl FeatureVector {
    /// 값으로부터 새 feature vector 생성.
    pub fn new(values: Vec<f32>) -> Self {
        Self {
            values,
            names: None,
        }
    }

    /// 이름이 있는 feature vector 생성.
    pub fn with_names(values: Vec<f32>, names: Vec<String>) -> Self {
        debug_assert_eq!(values.len(), names.len(), "Feature count mismatch");
        Self {
            values,
            names: Some(names),
        }
    }

    /// feature 값을 슬라이스로 반환.
    pub fn as_slice(&self) -> &[f32] {
        &self.values
    }

    /// feature 값을 가변 슬라이스로 반환.
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.values
    }

    /// feature 개수 반환.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// feature vector가 비어있는지 확인.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// 사용 가능한 경우 feature 이름 반환.
    pub fn names(&self) -> Option<&[String]> {
        self.names.as_deref()
    }

    /// 소유된 Vec<f32>로 변환.
    pub fn into_vec(self) -> Vec<f32> {
        self.values
    }

    /// min-max 스케일링을 사용하여 feature를 [0, 1] 범위로 정규화.
    pub fn normalize(&mut self) {
        if self.values.is_empty() {
            return;
        }

        let min = self.values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = self
            .values
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        let range = max - min;

        if range > f32::EPSILON {
            for v in &mut self.values {
                *v = (*v - min) / range;
            }
        }
    }

    /// feature를 평균 0, 분산 1로 표준화.
    pub fn standardize(&mut self) {
        if self.values.is_empty() {
            return;
        }

        let n = self.values.len() as f32;
        let mean = self.values.iter().sum::<f32>() / n;
        let variance = self.values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        let std_dev = variance.sqrt();

        if std_dev > f32::EPSILON {
            for v in &mut self.values {
                *v = (*v - mean) / std_dev;
            }
        }
    }
}

impl From<Vec<f32>> for FeatureVector {
    fn from(values: Vec<f32>) -> Self {
        Self::new(values)
    }
}

impl AsRef<[f32]> for FeatureVector {
    fn as_ref(&self) -> &[f32] {
        &self.values
    }
}

/// ML 모델의 prediction 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// 예측된 방향
    pub direction: PredictionDirection,
    /// 신뢰도 점수 (0.0 ~ 1.0)
    pub confidence: f32,
    /// 원시 prediction 값
    pub raw_value: f32,
    /// 예측된 가격 변동 퍼센트 (선택적)
    pub price_change_pct: Option<f32>,
    /// prediction에 사용된 모델
    pub model_name: String,
}

impl Prediction {
    /// 새 prediction 생성.
    pub fn new(
        direction: PredictionDirection,
        confidence: f32,
        raw_value: f32,
        model_name: impl Into<String>,
    ) -> Self {
        Self {
            direction,
            confidence: confidence.clamp(0.0, 1.0),
            raw_value,
            price_change_pct: None,
            model_name: model_name.into(),
        }
    }

    /// 예측된 가격 변동 퍼센트 추가.
    pub fn with_price_change(mut self, pct: f32) -> Self {
        self.price_change_pct = Some(pct);
        self
    }

    /// prediction이 유의미한지 확인 (높은 신뢰도).
    pub fn is_significant(&self, threshold: f32) -> bool {
        self.confidence >= threshold && self.direction != PredictionDirection::Sideways
    }
}

/// prediction과 패턴에 대한 신뢰도 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// 낮은 신뢰도 (< 50%)
    Low,
    /// 중간 신뢰도 (50% - 75%)
    Medium,
    /// 높은 신뢰도 (75% - 90%)
    High,
    /// 매우 높은 신뢰도 (> 90%)
    VeryHigh,
}

impl ConfidenceLevel {
    /// 신뢰도 점수(0.0 ~ 1.0)에서 변환.
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s >= 0.9 => ConfidenceLevel::VeryHigh,
            s if s >= 0.75 => ConfidenceLevel::High,
            s if s >= 0.5 => ConfidenceLevel::Medium,
            _ => ConfidenceLevel::Low,
        }
    }

    /// 이 신뢰도 수준의 최소 임계값 반환.
    pub fn min_threshold(&self) -> f32 {
        match self {
            ConfidenceLevel::Low => 0.0,
            ConfidenceLevel::Medium => 0.5,
            ConfidenceLevel::High => 0.75,
            ConfidenceLevel::VeryHigh => 0.9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_direction_from_value() {
        assert_eq!(
            PredictionDirection::from_value(0.5, 0.3),
            PredictionDirection::Up
        );
        assert_eq!(
            PredictionDirection::from_value(-0.5, 0.3),
            PredictionDirection::Down
        );
        assert_eq!(
            PredictionDirection::from_value(0.1, 0.3),
            PredictionDirection::Sideways
        );
    }

    #[test]
    fn test_feature_vector() {
        let mut features = FeatureVector::new(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(features.len(), 5);
        assert!(!features.is_empty());

        features.normalize();
        assert!((features.as_slice()[0] - 0.0).abs() < f32::EPSILON);
        assert!((features.as_slice()[4] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_feature_vector_standardize() {
        let mut features = FeatureVector::new(vec![2.0, 4.0, 6.0, 8.0, 10.0]);
        features.standardize();

        // Mean should be approximately 0
        let mean: f32 = features.as_slice().iter().sum::<f32>() / 5.0;
        assert!(mean.abs() < 0.001);
    }

    #[test]
    fn test_prediction() {
        let pred = Prediction::new(PredictionDirection::Up, 0.85, 0.7, "test_model")
            .with_price_change(2.5);

        assert!(pred.is_significant(0.8));
        assert!(!pred.is_significant(0.9));
        assert_eq!(pred.price_change_pct, Some(2.5));
    }

    #[test]
    fn test_confidence_level() {
        assert_eq!(ConfidenceLevel::from_score(0.95), ConfidenceLevel::VeryHigh);
        assert_eq!(ConfidenceLevel::from_score(0.8), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.6), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.3), ConfidenceLevel::Low);
    }
}
