//! ONNX 모델을 사용한 가격 예측.
//!
//! 이 모듈은 ONNX Runtime 기반 가격 방향 prediction을 제공합니다.
//! 모델은 별도로 학습되어야 하며 (예: Python/PyTorch 사용)
//! ONNX 형식으로 내보내야 합니다.

use crate::ml::{FeatureVector, MlError, MlResult, Prediction, PredictionDirection};
#[cfg(feature = "ml")]
use ort::session::Session;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ml")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(feature = "ml")]
use tracing::{debug, info};

/// ONNX predictor 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorConfig {
    /// ONNX 모델 파일 경로
    pub model_path: PathBuf,
    /// 예상 입력 feature 크기
    pub input_size: usize,
    /// Sideways가 아닌 prediction을 위한 최소 신뢰도 임계값
    pub confidence_threshold: f32,
    /// 로깅/식별을 위한 모델 이름
    pub model_name: String,
    /// GPU 가속 사용 여부 (가능한 경우)
    pub use_gpu: bool,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/price_predictor.onnx"),
            input_size: 20, // Default feature count from FeatureExtractor
            confidence_threshold: 0.6,
            model_name: "price_predictor".to_string(),
            use_gpu: false,
        }
    }
}

impl PredictorConfig {
    /// 주어진 모델 경로로 새 predictor 설정 생성.
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            ..Default::default()
        }
    }

    /// 입력 크기 설정.
    pub fn with_input_size(mut self, size: usize) -> Self {
        self.input_size = size;
        self
    }

    /// 신뢰도 임계값 설정.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// 모델 이름 설정.
    pub fn with_model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }
}

/// 상세 확률이 포함된 prediction 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    /// 예측된 방향
    pub direction: PredictionDirection,
    /// 신뢰도 점수 (0.0 ~ 1.0)
    pub confidence: f32,
    /// 각 클래스에 대한 원시 확률 [up, down, sideways]
    pub probabilities: [f32; 3],
    /// 이 prediction을 생성한 모델
    pub model_name: String,
}

impl PredictionResult {
    /// 공통 Prediction 타입으로 변환.
    pub fn to_prediction(&self) -> Prediction {
        Prediction::new(
            self.direction,
            self.confidence,
            self.probabilities[0] - self.probabilities[1], // 순 강세 점수
            &self.model_name,
        )
    }
}

/// ONNX 기반 가격 방향 predictor.
///
/// ONNX 모델을 로드하고 추론을 수행하여 가격 방향을 예측합니다.
/// 모델은 다음을 가져야 합니다:
/// - 입력: [batch_size, input_size] 형태의 float32 텐서
/// - 출력: [batch_size, 3] 형태의 float32 텐서 (softmax 확률)
///
/// 출력 클래스: [Up, Down, Sideways]
#[cfg(feature = "ml")]
pub struct OnnxPredictor {
    session: Session,
    config: PredictorConfig,
}

#[cfg(feature = "ml")]
impl OnnxPredictor {
    /// 지정된 경로에서 ONNX 모델 로드.
    pub fn load(config: PredictorConfig) -> MlResult<Self> {
        let path = &config.model_path;

        if !path.exists() {
            return Err(MlError::ModelLoad(format!(
                "Model file not found: {}",
                path.display()
            )));
        }

        info!("Loading ONNX model from: {}", path.display());

        let session = Session::builder()
            .map_err(|e| MlError::ModelLoad(format!("Failed to create session builder: {}", e)))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| MlError::ModelLoad(format!("Failed to set optimization level: {}", e)))?
            .commit_from_file(path)
            .map_err(|e| MlError::ModelLoad(format!("Failed to load model: {}", e)))?;

        info!("ONNX model loaded successfully: {}", config.model_name);

        Ok(Self { session, config })
    }

    /// 기본 설정으로 파일 경로에서 모델 로드.
    pub fn from_file(path: impl AsRef<Path>) -> MlResult<Self> {
        let config = PredictorConfig::new(path.as_ref());
        Self::load(config)
    }

    /// predictor 설정 반환.
    pub fn config(&self) -> &PredictorConfig {
        &self.config
    }

    /// feature vector에서 가격 방향 예측.
    pub fn predict(&mut self, features: &FeatureVector) -> MlResult<PredictionResult> {
        // 입력 크기 검증
        if features.len() != self.config.input_size {
            return Err(MlError::InvalidInput(format!(
                "Expected {} features, got {}",
                self.config.input_size,
                features.len()
            )));
        }

        // 입력 텐서 생성 [1, input_size]
        let input_data: Vec<f32> = features.as_slice().to_vec();
        let input_shape = [1i64, self.config.input_size as i64];

        // 텐서 값 생성
        let input_tensor =
            ort::value::Tensor::from_array((input_shape, input_data.into_boxed_slice()))
                .map_err(|e| MlError::Inference(format!("Failed to create input tensor: {}", e)))?;

        // 입력으로 추론 실행
        let outputs = self
            .session
            .run(ort::inputs!["input" => input_tensor])
            .map_err(|e| MlError::Inference(format!("Inference failed: {}", e)))?;

        // 첫 번째 출력 가져오기 ("output" 이름 또는 첫 번째 사용 가능한 것)
        let output_name = outputs
            .iter()
            .next()
            .map(|(name, _)| name.to_string())
            .ok_or_else(|| MlError::Inference("No output tensor found".to_string()))?;

        let output = outputs
            .get(&output_name)
            .ok_or_else(|| MlError::Inference("Failed to get output by name".to_string()))?;

        // 텐서 데이터 추출 - (&Shape, &[f32]) 반환
        let (_, output_slice) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Inference(format!("Failed to extract output tensor: {}", e)))?;

        // 확률 파싱 ([up, down, sideways] 예상)
        if output_slice.len() < 3 {
            return Err(MlError::Inference(format!(
                "Expected 3 output values, got {}",
                output_slice.len()
            )));
        }

        // 출력을 드롭하기 전에 확률을 소유 데이터로 복사
        let probabilities = [output_slice[0], output_slice[1], output_slice[2]];

        // session에 대한 가변 차용을 해제하기 위해 출력 드롭
        drop(outputs);

        // 아직 적용되지 않은 경우 softmax 적용 (합계 ≈ 1 확인)
        let sum: f32 = probabilities.iter().sum();
        let probs = if (sum - 1.0).abs() > 0.01 {
            // softmax 적용
            let max_val = probabilities
                .iter()
                .cloned()
                .fold(f32::NEG_INFINITY, f32::max);
            let exp_vals: Vec<f32> = probabilities.iter().map(|x| (x - max_val).exp()).collect();
            let exp_sum: f32 = exp_vals.iter().sum();
            [
                exp_vals[0] / exp_sum,
                exp_vals[1] / exp_sum,
                exp_vals[2] / exp_sum,
            ]
        } else {
            probabilities
        };

        // 방향과 신뢰도 결정
        let (direction, confidence) = self.interpret_probabilities(&probs);

        debug!(
            "Prediction: {:?} (confidence: {:.2}%, probs: [{:.2}, {:.2}, {:.2}])",
            direction,
            confidence * 100.0,
            probs[0],
            probs[1],
            probs[2]
        );

        Ok(PredictionResult {
            direction,
            confidence,
            probabilities: probs,
            model_name: self.config.model_name.clone(),
        })
    }

    /// feature vector 배치에 대해 방향 예측.
    pub fn predict_batch(
        &mut self,
        features_batch: &[FeatureVector],
    ) -> MlResult<Vec<PredictionResult>> {
        // 단순함을 위해 한 번에 하나씩 처리
        // 더 최적화된 버전은 단일 텐서로 배치 처리
        features_batch.iter().map(|f| self.predict(f)).collect()
    }

    /// 확률 분포를 해석하여 방향과 신뢰도 반환.
    fn interpret_probabilities(&self, probs: &[f32; 3]) -> (PredictionDirection, f32) {
        let up_prob = probs[0];
        let down_prob = probs[1];
        let sideways_prob = probs[2];

        // 가장 가능성 높은 방향 찾기
        let max_prob = up_prob.max(down_prob).max(sideways_prob);

        let direction = if max_prob == up_prob && up_prob >= self.config.confidence_threshold {
            PredictionDirection::Up
        } else if max_prob == down_prob && down_prob >= self.config.confidence_threshold {
            PredictionDirection::Down
        } else {
            PredictionDirection::Sideways
        };

        // 신뢰도는 선택된 방향의 확률
        let confidence = match direction {
            PredictionDirection::Up => up_prob,
            PredictionDirection::Down => down_prob,
            PredictionDirection::Sideways => sideways_prob,
        };

        (direction, confidence)
    }
}

/// 실제 모델 파일 없이 테스트하기 위한 mock predictor.
pub struct MockPredictor {
    #[allow(dead_code)]
    config: PredictorConfig,
    /// 반환할 고정 prediction
    pub fixed_prediction: Option<PredictionResult>,
}

impl MockPredictor {
    /// 새 mock predictor 생성.
    pub fn new(input_size: usize) -> Self {
        Self {
            config: PredictorConfig::default().with_input_size(input_size),
            fixed_prediction: None,
        }
    }

    /// 항상 반환할 고정 prediction 설정.
    pub fn with_fixed_prediction(mut self, prediction: PredictionResult) -> Self {
        self.fixed_prediction = Some(prediction);
        self
    }

    /// mock 로직으로 예측.
    pub fn predict(&mut self, features: &FeatureVector) -> MlResult<PredictionResult> {
        if let Some(ref pred) = self.fixed_prediction {
            return Ok(pred.clone());
        }

        // feature 값 기반으로 prediction 생성
        let values = features.as_slice();
        if values.is_empty() {
            return Err(MlError::InvalidInput("Empty feature vector".to_string()));
        }

        // 간단한 휴리스틱: feature 평균으로 방향 결정
        let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;

        let (direction, probs): (PredictionDirection, [f32; 3]) = if mean > 0.1 {
            (PredictionDirection::Up, [0.7_f32, 0.15_f32, 0.15_f32])
        } else if mean < -0.1 {
            (PredictionDirection::Down, [0.15_f32, 0.7_f32, 0.15_f32])
        } else {
            (PredictionDirection::Sideways, [0.2_f32, 0.2_f32, 0.6_f32])
        };

        Ok(PredictionResult {
            direction,
            confidence: probs[0].max(probs[1]).max(probs[2]),
            probabilities: probs,
            model_name: "mock_predictor".to_string(),
        })
    }
}

/// 다형성을 가능하게 하는 predictor trait.
pub trait PricePredictor: Send + Sync {
    /// feature에서 가격 방향 예측.
    fn predict(&mut self, features: &FeatureVector) -> MlResult<PredictionResult>;

    /// 모델 이름 반환.
    fn model_name(&self) -> &str;
}

#[cfg(feature = "ml")]
impl PricePredictor for OnnxPredictor {
    fn predict(&mut self, features: &FeatureVector) -> MlResult<PredictionResult> {
        OnnxPredictor::predict(self, features)
    }

    fn model_name(&self) -> &str {
        &self.config.model_name
    }
}

impl PricePredictor for MockPredictor {
    fn predict(&mut self, features: &FeatureVector) -> MlResult<PredictionResult> {
        MockPredictor::predict(self, features)
    }

    fn model_name(&self) -> &str {
        "mock_predictor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictor_config_default() {
        let config = PredictorConfig::default();
        assert_eq!(config.input_size, 20);
        assert_eq!(config.confidence_threshold, 0.6);
    }

    #[test]
    fn test_predictor_config_builder() {
        let config = PredictorConfig::new("models/test.onnx")
            .with_input_size(30)
            .with_confidence_threshold(0.7)
            .with_model_name("test_model");

        assert_eq!(config.input_size, 30);
        assert_eq!(config.confidence_threshold, 0.7);
        assert_eq!(config.model_name, "test_model");
    }

    #[cfg(feature = "ml")]
    #[test]
    fn test_model_not_found() {
        let config = PredictorConfig::new("nonexistent/model.onnx");
        let result = OnnxPredictor::load(config);

        assert!(result.is_err());
        match result {
            Err(MlError::ModelLoad(msg)) => {
                assert!(msg.contains("not found"));
            }
            _ => panic!("Expected ModelLoad error"),
        }
    }

    #[test]
    fn test_mock_predictor() {
        let mut predictor = MockPredictor::new(20);
        let features = FeatureVector::new(vec![0.2; 20]);

        let result = predictor.predict(&features).unwrap();
        assert_eq!(result.direction, PredictionDirection::Up);
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_mock_predictor_down() {
        let mut predictor = MockPredictor::new(20);
        let features = FeatureVector::new(vec![-0.2; 20]);

        let result = predictor.predict(&features).unwrap();
        assert_eq!(result.direction, PredictionDirection::Down);
    }

    #[test]
    fn test_mock_predictor_sideways() {
        let mut predictor = MockPredictor::new(20);
        let features = FeatureVector::new(vec![0.0; 20]);

        let result = predictor.predict(&features).unwrap();
        assert_eq!(result.direction, PredictionDirection::Sideways);
    }

    #[test]
    fn test_prediction_result_to_prediction() {
        let result = PredictionResult {
            direction: PredictionDirection::Up,
            confidence: 0.85,
            probabilities: [0.85, 0.10, 0.05],
            model_name: "test".to_string(),
        };

        let pred = result.to_prediction();
        assert_eq!(pred.direction, PredictionDirection::Up);
        assert_eq!(pred.confidence, 0.85);
        assert!((pred.raw_value - 0.75).abs() < 0.01); // 0.85 - 0.10
    }

    #[test]
    fn test_interpret_probabilities() {
        // 높은 신뢰도 상승
        let (dir, conf) = interpret_probs(&[0.8_f32, 0.1_f32, 0.1_f32], 0.6);
        assert_eq!(dir, PredictionDirection::Up);
        assert!((conf - 0.8).abs() < 0.01);

        // 높은 신뢰도 하락
        let (dir, _conf) = interpret_probs(&[0.1_f32, 0.8_f32, 0.1_f32], 0.6);
        assert_eq!(dir, PredictionDirection::Down);

        // 임계값 미만 - sideways여야 함
        let (dir, _) = interpret_probs(&[0.5_f32, 0.3_f32, 0.2_f32], 0.6);
        assert_eq!(dir, PredictionDirection::Sideways);
    }

    // 확률 해석 테스트를 위한 헬퍼
    fn interpret_probs(probs: &[f32; 3], threshold: f32) -> (PredictionDirection, f32) {
        let up_prob = probs[0];
        let down_prob = probs[1];
        let sideways_prob = probs[2];
        let max_prob = up_prob.max(down_prob).max(sideways_prob);

        let direction = if max_prob == up_prob && up_prob >= threshold {
            PredictionDirection::Up
        } else if max_prob == down_prob && down_prob >= threshold {
            PredictionDirection::Down
        } else {
            PredictionDirection::Sideways
        };

        let confidence = match direction {
            PredictionDirection::Up => up_prob,
            PredictionDirection::Down => down_prob,
            PredictionDirection::Sideways => sideways_prob,
        };

        (direction, confidence)
    }
}
