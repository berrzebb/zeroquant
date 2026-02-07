//! ML 모듈 에러 타입.

use thiserror::Error;

/// ML 작업에서 발생할 수 있는 에러.
#[derive(Debug, Error)]
pub enum MlError {
    /// ONNX 모델 로드 에러
    #[error("Model load error: {0}")]
    ModelLoad(String),

    /// 모델 추론 중 에러
    #[error("Inference error: {0}")]
    Inference(String),

    /// 시장 데이터에서 feature 추출 에러
    #[error("Feature extraction error: {0}")]
    FeatureExtraction(String),

    /// 패턴 인식 에러
    #[error("Pattern recognition error: {0}")]
    PatternRecognition(String),

    /// 유효하지 않은 입력 데이터
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// 분석을 위한 데이터 부족
    #[error("Insufficient data: need {required} samples, got {actual}")]
    InsufficientData { required: usize, actual: usize },

    /// ONNX Runtime 에러
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(String),
}

/// ML 작업을 위한 Result 타입.
pub type MlResult<T> = Result<T, MlError>;

impl MlError {
    /// 이 에러가 복구 가능한지 확인 (다른 데이터로 재시도 가능).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            MlError::InsufficientData { .. } | MlError::InvalidInput(_)
        )
    }

    /// 이 에러가 모델 리로드를 필요로 하는지 확인.
    pub fn requires_reload(&self) -> bool {
        matches!(self, MlError::ModelLoad(_) | MlError::OnnxRuntime(_))
    }
}

// ONNX Runtime 에러로부터 변환
impl From<ort::Error> for MlError {
    fn from(err: ort::Error) -> Self {
        MlError::OnnxRuntime(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MlError::ModelLoad("file not found".to_string());
        assert_eq!(err.to_string(), "Model load error: file not found");

        let err = MlError::InsufficientData {
            required: 100,
            actual: 50,
        };
        assert_eq!(
            err.to_string(),
            "Insufficient data: need 100 samples, got 50"
        );
    }

    #[test]
    fn test_error_recoverable() {
        let err = MlError::InsufficientData {
            required: 100,
            actual: 50,
        };
        assert!(err.is_recoverable());

        let err = MlError::ModelLoad("corrupted".to_string());
        assert!(!err.is_recoverable());
    }

    #[test]
    fn test_error_requires_reload() {
        let err = MlError::ModelLoad("missing".to_string());
        assert!(err.requires_reload());

        let err = MlError::FeatureExtraction("NaN value".to_string());
        assert!(!err.requires_reload());
    }
}
