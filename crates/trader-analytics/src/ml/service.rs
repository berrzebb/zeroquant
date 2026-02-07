//! ML 서비스 - 패턴 인식, 피처 추출, 예측을 통합하는 서비스.
//!
//! 이 모듈은 ML 기능들을 하나의 인터페이스로 통합합니다:
//! - 캔들스틱 패턴 인식
//! - 차트 패턴 인식
//! - 피처 추출
//! - 가격 예측 (ONNX 모델 사용시)

use crate::ml::{
    error::MlResult,
    features::{FeatureConfig, FeatureExtractor},
    pattern::{
        CandlestickPattern, CandlestickPatternType, ChartPattern, ChartPatternType, PatternConfig,
        PatternRecognizer,
    },
    predictor::{MockPredictor, PredictorConfig, PricePredictor},
    types::{ConfidenceLevel, FeatureVector, Prediction},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::Kline;

/// ML 서비스 설정.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlServiceConfig {
    /// 피처 추출 설정
    #[serde(default)]
    pub feature_config: FeatureConfig,
    /// 패턴 인식 설정
    #[serde(default)]
    pub pattern_config: PatternConfig,
    /// 예측기 설정 (ONNX 모델 경로 등)
    #[serde(default)]
    pub predictor_config: PredictorConfig,
    /// 캔들스틱 패턴 감지 활성화
    #[serde(default = "default_true")]
    pub enable_candlestick_patterns: bool,
    /// 차트 패턴 감지 활성화
    #[serde(default = "default_true")]
    pub enable_chart_patterns: bool,
    /// 가격 예측 활성화
    #[serde(default)]
    pub enable_prediction: bool,
    /// 예측에 필요한 최소 신뢰도
    #[serde(default = "default_min_confidence")]
    pub min_prediction_confidence: f32,
}

fn default_true() -> bool {
    true
}

fn default_min_confidence() -> f32 {
    0.6
}

impl Default for MlServiceConfig {
    fn default() -> Self {
        Self {
            feature_config: FeatureConfig::default(),
            pattern_config: PatternConfig::default(),
            predictor_config: PredictorConfig::default(),
            enable_candlestick_patterns: true,
            enable_chart_patterns: true,
            enable_prediction: false, // ONNX 모델 없으면 비활성화
            min_prediction_confidence: 0.6,
        }
    }
}

/// ML 분석 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlAnalysisResult {
    /// 심볼
    pub symbol: String,
    /// 분석 시간
    pub timestamp: DateTime<Utc>,
    /// 감지된 캔들스틱 패턴
    pub candlestick_patterns: Vec<CandlestickPattern>,
    /// 감지된 차트 패턴
    pub chart_patterns: Vec<ChartPattern>,
    /// 가격 예측 (활성화된 경우)
    pub prediction: Option<Prediction>,
    /// 추출된 피처 요약
    pub feature_summary: Option<FeatureSummary>,
}

/// 피처 요약 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSummary {
    /// 피처 개수
    pub feature_count: usize,
    /// RSI 값 (0-100)
    pub rsi: Option<f32>,
    /// MACD 히스토그램
    pub macd_histogram: Option<f32>,
    /// 볼린저 밴드 %B (0-1)
    pub bb_percent_b: Option<f32>,
    /// ATR 비율
    pub atr_ratio: Option<f32>,
}

/// 패턴 감지 결과 (API 응답용).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct PatternDetectionResult {
    /// 심볼
    pub symbol: String,
    /// 타임프레임
    pub timeframe: String,
    /// 분석 시간
    pub timestamp: DateTime<Utc>,
    /// 캔들스틱 패턴
    pub candlestick_patterns: Vec<CandlestickPatternInfo>,
    /// 차트 패턴
    pub chart_patterns: Vec<ChartPatternInfo>,
    /// 전체 신호 (상승/하락/중립)
    pub overall_signal: String,
    /// 신호 강도 (-1.0 ~ 1.0)
    pub signal_strength: f32,
}

/// 캔들스틱 패턴 정보 (API 응답용).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct CandlestickPatternInfo {
    /// 패턴 타입
    pub pattern_type: String,
    /// 패턴 이름 (한글)
    pub name: String,
    /// 신호 방향
    pub signal: String,
    /// 신뢰도 (0-1)
    pub confidence: f32,
    /// 신뢰도 레벨
    pub confidence_level: String,
    /// 패턴 설명
    pub description: String,
}

/// 차트 패턴 정보 (API 응답용).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa-support", derive(utoipa::ToSchema))]
pub struct ChartPatternInfo {
    /// 패턴 타입
    pub pattern_type: String,
    /// 패턴 이름 (한글)
    pub name: String,
    /// 신호 방향
    pub signal: String,
    /// 신뢰도 (0-1)
    pub confidence: f32,
    /// 목표가
    pub target_price: Option<String>,
    /// 패턴 시작 인덱스
    pub start_index: usize,
    /// 패턴 끝 인덱스
    pub end_index: usize,
}

/// ML 서비스.
///
/// 패턴 인식, 피처 추출, 가격 예측을 통합하는 서비스입니다.
pub struct MlService {
    config: MlServiceConfig,
    feature_extractor: FeatureExtractor,
    pattern_recognizer: PatternRecognizer,
    predictor: Arc<RwLock<Box<dyn PricePredictor>>>,
}

impl MlService {
    /// 새 ML 서비스 생성.
    pub fn new(config: MlServiceConfig) -> MlResult<Self> {
        let feature_extractor = FeatureExtractor::new(config.feature_config.clone());
        let pattern_recognizer = PatternRecognizer::new(config.pattern_config.clone());

        // 기본적으로 MockPredictor 사용 (ONNX 모델 없음)
        // input_size는 feature_config에서 계산된 피처 개수 사용
        let input_size = config.feature_config.feature_count();
        let predictor: Box<dyn PricePredictor> = Box::new(MockPredictor::new(input_size));

        Ok(Self {
            config,
            feature_extractor,
            pattern_recognizer,
            predictor: Arc::new(RwLock::new(predictor)),
        })
    }

    /// 기본 설정으로 서비스 생성.
    pub fn with_defaults() -> MlResult<Self> {
        Self::new(MlServiceConfig::default())
    }

    /// ONNX 모델 경로로 서비스 생성.
    ///
    /// # Arguments
    /// * `model_path` - ONNX 모델 파일 경로
    /// * `model_name` - 모델 식별 이름
    #[cfg(feature = "ml")]
    pub fn with_onnx_model(
        config: MlServiceConfig,
        model_path: impl AsRef<std::path::Path>,
        model_name: &str,
    ) -> MlResult<Self> {
        use crate::ml::predictor::OnnxPredictor;

        let feature_extractor = FeatureExtractor::new(config.feature_config.clone());
        let pattern_recognizer = PatternRecognizer::new(config.pattern_config.clone());

        let input_size = config.feature_config.feature_count();
        let predictor_config = PredictorConfig::new(model_path.as_ref())
            .with_input_size(input_size)
            .with_model_name(model_name);

        let predictor: Box<dyn PricePredictor> = Box::new(OnnxPredictor::load(predictor_config)?);

        let mut service_config = config;
        service_config.enable_prediction = true;

        Ok(Self {
            config: service_config,
            feature_extractor,
            pattern_recognizer,
            predictor: Arc::new(RwLock::new(predictor)),
        })
    }

    /// 런타임에 ONNX 모델 로드.
    ///
    /// 현재 predictor를 새 ONNX 모델로 교체합니다.
    ///
    /// # Arguments
    /// * `model_path` - ONNX 모델 파일 경로
    /// * `model_name` - 모델 식별 이름
    #[cfg(feature = "ml")]
    pub async fn load_onnx_model(
        &self,
        model_path: impl AsRef<std::path::Path>,
        model_name: &str,
    ) -> MlResult<()> {
        use crate::ml::predictor::OnnxPredictor;

        let input_size = self.config.feature_config.feature_count();
        let predictor_config = PredictorConfig::new(model_path.as_ref())
            .with_input_size(input_size)
            .with_model_name(model_name);

        let new_predictor: Box<dyn PricePredictor> =
            Box::new(OnnxPredictor::load(predictor_config)?);

        let mut predictor = self.predictor.write().await;
        *predictor = new_predictor;

        tracing::info!("ONNX 모델 로드 완료: {}", model_name);
        Ok(())
    }

    /// ONNX 모델 로드 (ml feature 없이 빌드 시 사용 불가).
    #[cfg(not(feature = "ml"))]
    pub async fn load_onnx_model(
        &self,
        _model_path: impl AsRef<std::path::Path>,
        _model_name: &str,
    ) -> MlResult<()> {
        Err(crate::ml::error::MlError::ModelLoad(
            "ONNX Runtime not available (build with 'ml' feature)".to_string(),
        ))
    }

    /// Mock predictor로 초기화 (테스트용).
    pub async fn reset_to_mock(&self) {
        let input_size = self.config.feature_config.feature_count();
        let mock: Box<dyn PricePredictor> = Box::new(MockPredictor::new(input_size));

        let mut predictor = self.predictor.write().await;
        *predictor = mock;

        tracing::info!("Mock predictor로 초기화됨");
    }

    /// 현재 로드된 모델 이름 반환.
    pub async fn current_model_name(&self) -> String {
        let predictor = self.predictor.read().await;
        predictor.model_name().to_string()
    }

    /// 예측 기능 활성화 여부 설정.
    pub fn set_prediction_enabled(&mut self, enabled: bool) {
        self.config.enable_prediction = enabled;
    }

    /// 예측 기능이 활성화되어 있는지 확인.
    pub fn is_prediction_enabled(&self) -> bool {
        self.config.enable_prediction
    }

    /// 설정 반환.
    pub fn config(&self) -> &MlServiceConfig {
        &self.config
    }

    /// 캔들스틱 패턴 감지.
    ///
    /// # Arguments
    /// * `klines` - 분석할 Kline 데이터 (최소 3개 이상)
    ///
    /// # Returns
    /// 감지된 캔들스틱 패턴 목록
    pub fn detect_candlestick_patterns(&self, klines: &[Kline]) -> Vec<CandlestickPattern> {
        if !self.config.enable_candlestick_patterns || klines.is_empty() {
            return Vec::new();
        }

        self.pattern_recognizer.detect_candlestick_patterns(klines)
    }

    /// 차트 패턴 감지.
    ///
    /// # Arguments
    /// * `klines` - 분석할 Kline 데이터 (최소 20개 이상 권장)
    ///
    /// # Returns
    /// 감지된 차트 패턴 목록
    pub fn detect_chart_patterns(&self, klines: &[Kline]) -> Vec<ChartPattern> {
        if !self.config.enable_chart_patterns || klines.len() < 20 {
            return Vec::new();
        }

        self.pattern_recognizer.detect_chart_patterns(klines)
    }

    /// 패턴 감지 (API 응답 형식).
    ///
    /// 캔들스틱과 차트 패턴을 모두 감지하고 API 응답에 적합한 형식으로 반환합니다.
    pub fn detect_patterns(
        &self,
        symbol: &str,
        timeframe: &str,
        klines: &[Kline],
    ) -> PatternDetectionResult {
        let candlestick_patterns = self.detect_candlestick_patterns(klines);
        let chart_patterns = self.detect_chart_patterns(klines);

        // 캔들스틱 패턴 정보 변환
        let candlestick_infos: Vec<CandlestickPatternInfo> = candlestick_patterns
            .iter()
            .map(|p| CandlestickPatternInfo {
                pattern_type: format!("{:?}", p.pattern_type),
                name: get_candlestick_pattern_name(&p.pattern_type),
                signal: if p.bullish {
                    "bullish".to_string()
                } else {
                    "bearish".to_string()
                },
                confidence: p.confidence as f32,
                confidence_level: format!("{:?}", ConfidenceLevel::from_score(p.confidence as f32)),
                description: get_candlestick_pattern_description(&p.pattern_type),
            })
            .collect();

        // 차트 패턴 정보 변환
        let chart_infos: Vec<ChartPatternInfo> = chart_patterns
            .iter()
            .map(|p| ChartPatternInfo {
                pattern_type: format!("{:?}", p.pattern_type),
                name: get_chart_pattern_name(&p.pattern_type),
                signal: if p.bullish {
                    "bullish".to_string()
                } else {
                    "bearish".to_string()
                },
                confidence: p.confidence as f32,
                target_price: p.price_target.map(|d| d.to_string()),
                start_index: p.start_index,
                end_index: p.end_index,
            })
            .collect();

        // 전체 신호 계산
        let (overall_signal, signal_strength) =
            self.calculate_overall_signal(&candlestick_patterns, &chart_patterns);

        PatternDetectionResult {
            symbol: symbol.to_string(),
            timeframe: timeframe.to_string(),
            timestamp: Utc::now(),
            candlestick_patterns: candlestick_infos,
            chart_patterns: chart_infos,
            overall_signal,
            signal_strength,
        }
    }

    /// 피처 추출.
    pub fn extract_features(&self, klines: &[Kline]) -> MlResult<FeatureVector> {
        self.feature_extractor.extract(klines)
    }

    /// 피처 추출 및 요약.
    pub fn extract_features_with_summary(
        &self,
        klines: &[Kline],
    ) -> MlResult<(FeatureVector, FeatureSummary)> {
        let features = self.feature_extractor.extract(klines)?;

        // 피처 이름으로 특정 값 추출
        let names = features.names();
        let values = features.as_slice();

        let mut rsi = None;
        let mut macd_histogram = None;
        let mut bb_percent_b = None;
        let mut atr_ratio = None;

        if let Some(names) = names {
            for (i, name) in names.iter().enumerate() {
                match name.as_str() {
                    "rsi" => rsi = Some(values[i] * 100.0), // 0-1에서 0-100으로
                    "macd_histogram" => macd_histogram = Some(values[i]),
                    "bb_percent_b" => bb_percent_b = Some(values[i]),
                    "atr_ratio" => atr_ratio = Some(values[i]),
                    _ => {}
                }
            }
        }

        let summary = FeatureSummary {
            feature_count: features.len(),
            rsi,
            macd_histogram,
            bb_percent_b,
            atr_ratio,
        };

        Ok((features, summary))
    }

    /// 가격 예측 (활성화된 경우).
    pub async fn predict(&self, features: &FeatureVector) -> MlResult<Option<Prediction>> {
        if !self.config.enable_prediction {
            return Ok(None);
        }

        let mut predictor = self.predictor.write().await;
        let result = predictor.predict(features)?;

        // probabilities[0]=up, probabilities[1]=down, probabilities[2]=sideways
        // raw_value: up - down (양수면 상승, 음수면 하락)
        let raw_value = result.probabilities[0] - result.probabilities[1];

        let prediction = Prediction::new(
            result.direction,
            result.confidence,
            raw_value,
            predictor.model_name(),
        );

        // 최소 신뢰도 확인
        if prediction.confidence >= self.config.min_prediction_confidence {
            Ok(Some(prediction))
        } else {
            Ok(None)
        }
    }

    /// 전체 ML 분석 실행.
    pub async fn analyze(&self, symbol: &str, klines: &[Kline]) -> MlResult<MlAnalysisResult> {
        // 패턴 감지
        let candlestick_patterns = self.detect_candlestick_patterns(klines);
        let chart_patterns = self.detect_chart_patterns(klines);

        // 피처 추출 및 요약
        let (features, summary) = match self.extract_features_with_summary(klines) {
            Ok((f, s)) => (Some(f), Some(s)),
            Err(_) => (None, None),
        };

        // 예측 (피처가 있고 예측이 활성화된 경우)
        let prediction = if let Some(ref f) = features {
            self.predict(f).await?
        } else {
            None
        };

        Ok(MlAnalysisResult {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            candlestick_patterns,
            chart_patterns,
            prediction,
            feature_summary: summary,
        })
    }

    /// 전체 신호 계산.
    fn calculate_overall_signal(
        &self,
        candlestick_patterns: &[CandlestickPattern],
        chart_patterns: &[ChartPattern],
    ) -> (String, f32) {
        let mut bullish_score = 0.0f32;
        let mut bearish_score = 0.0f32;

        // 캔들스틱 패턴 점수 (가중치: 0.4)
        for pattern in candlestick_patterns {
            let weight = (pattern.confidence as f32) * 0.4;
            if pattern.bullish {
                bullish_score += weight;
            } else {
                bearish_score += weight;
            }
        }

        // 차트 패턴 점수 (가중치: 0.6)
        for pattern in chart_patterns {
            let weight = (pattern.confidence as f32) * 0.6;
            if pattern.bullish {
                bullish_score += weight;
            } else {
                bearish_score += weight;
            }
        }

        // 정규화
        let total = bullish_score + bearish_score;
        let strength = if total > 0.0 {
            (bullish_score - bearish_score) / total
        } else {
            0.0
        };

        let signal = if strength > 0.2 {
            "bullish".to_string()
        } else if strength < -0.2 {
            "bearish".to_string()
        } else {
            "neutral".to_string()
        };

        (signal, strength)
    }
}

// === 헬퍼 함수들 ===

/// 캔들스틱 패턴 이름 반환 (한글).
fn get_candlestick_pattern_name(pattern_type: &CandlestickPatternType) -> String {
    match pattern_type {
        CandlestickPatternType::Doji => "도지".to_string(),
        CandlestickPatternType::DragonflyDoji => "잠자리 도지".to_string(),
        CandlestickPatternType::GravestoneDoji => "비석 도지".to_string(),
        CandlestickPatternType::Hammer => "해머".to_string(),
        CandlestickPatternType::InvertedHammer => "역해머".to_string(),
        CandlestickPatternType::BullishMarubozu => "양봉 장대".to_string(),
        CandlestickPatternType::BearishMarubozu => "음봉 장대".to_string(),
        CandlestickPatternType::SpinningTop => "팽이".to_string(),
        CandlestickPatternType::ShootingStar => "유성형".to_string(),
        CandlestickPatternType::HangingMan => "교수형".to_string(),
        CandlestickPatternType::BullishEngulfing => "상승 장악형".to_string(),
        CandlestickPatternType::BearishEngulfing => "하락 장악형".to_string(),
        CandlestickPatternType::BullishHarami => "상승 잉태형".to_string(),
        CandlestickPatternType::BearishHarami => "하락 잉태형".to_string(),
        CandlestickPatternType::PiercingLine => "관통형".to_string(),
        CandlestickPatternType::DarkCloudCover => "먹구름형".to_string(),
        CandlestickPatternType::TweezerBottom => "집게 바닥".to_string(),
        CandlestickPatternType::TweezerTop => "집게 천장".to_string(),
        CandlestickPatternType::MorningStar => "샛별형".to_string(),
        CandlestickPatternType::MorningDojiStar => "샛별 도지형".to_string(),
        CandlestickPatternType::EveningStar => "석별형".to_string(),
        CandlestickPatternType::EveningDojiStar => "석별 도지형".to_string(),
        CandlestickPatternType::ThreeWhiteSoldiers => "적삼병".to_string(),
        CandlestickPatternType::ThreeBlackCrows => "흑삼병".to_string(),
        CandlestickPatternType::BullishAbandonedBaby => "상승 버림받은 아기".to_string(),
        CandlestickPatternType::BearishAbandonedBaby => "하락 버림받은 아기".to_string(),
    }
}

/// 캔들스틱 패턴 설명 반환.
fn get_candlestick_pattern_description(pattern_type: &CandlestickPatternType) -> String {
    match pattern_type {
        CandlestickPatternType::Doji => "시가와 종가가 거의 같은 중립 패턴".to_string(),
        CandlestickPatternType::DragonflyDoji => {
            "긴 아래꼬리를 가진 도지, 상승 반전 신호".to_string()
        }
        CandlestickPatternType::GravestoneDoji => {
            "긴 위꼬리를 가진 도지, 하락 반전 신호".to_string()
        }
        CandlestickPatternType::Hammer => "하락 추세에서 긴 아래꼬리, 상승 반전 신호".to_string(),
        CandlestickPatternType::InvertedHammer => {
            "하락 추세에서 긴 위꼬리, 상승 반전 가능".to_string()
        }
        CandlestickPatternType::BullishMarubozu => "꼬리 없는 강한 양봉, 매수세 강함".to_string(),
        CandlestickPatternType::BearishMarubozu => "꼬리 없는 강한 음봉, 매도세 강함".to_string(),
        CandlestickPatternType::SpinningTop => "작은 몸통과 긴 꼬리, 우유부단한 시장".to_string(),
        CandlestickPatternType::ShootingStar => {
            "상승 추세에서 긴 위꼬리, 하락 반전 신호".to_string()
        }
        CandlestickPatternType::HangingMan => {
            "상승 추세에서 긴 아래꼬리, 하락 반전 경고".to_string()
        }
        CandlestickPatternType::BullishEngulfing => {
            "음봉을 완전히 감싸는 양봉, 강한 상승 신호".to_string()
        }
        CandlestickPatternType::BearishEngulfing => {
            "양봉을 완전히 감싸는 음봉, 강한 하락 신호".to_string()
        }
        CandlestickPatternType::BullishHarami => {
            "큰 음봉 안에 작은 양봉, 상승 반전 가능".to_string()
        }
        CandlestickPatternType::BearishHarami => {
            "큰 양봉 안에 작은 음봉, 하락 반전 가능".to_string()
        }
        CandlestickPatternType::PiercingLine => {
            "음봉 후 중간 이상 상승하는 양봉, 상승 반전".to_string()
        }
        CandlestickPatternType::DarkCloudCover => {
            "양봉 후 중간 이하로 하락하는 음봉, 하락 반전".to_string()
        }
        CandlestickPatternType::TweezerBottom => "연속된 동일 저가, 지지선 형성".to_string(),
        CandlestickPatternType::TweezerTop => "연속된 동일 고가, 저항선 형성".to_string(),
        CandlestickPatternType::MorningStar => "음봉-작은봉-양봉, 강한 상승 반전".to_string(),
        CandlestickPatternType::MorningDojiStar => "음봉-도지-양봉, 강한 상승 반전".to_string(),
        CandlestickPatternType::EveningStar => "양봉-작은봉-음봉, 강한 하락 반전".to_string(),
        CandlestickPatternType::EveningDojiStar => "양봉-도지-음봉, 강한 하락 반전".to_string(),
        CandlestickPatternType::ThreeWhiteSoldiers => "연속 3개 양봉, 강한 상승 추세".to_string(),
        CandlestickPatternType::ThreeBlackCrows => "연속 3개 음봉, 강한 하락 추세".to_string(),
        CandlestickPatternType::BullishAbandonedBaby => {
            "갭을 가진 반전 패턴, 강한 상승 신호".to_string()
        }
        CandlestickPatternType::BearishAbandonedBaby => {
            "갭을 가진 반전 패턴, 강한 하락 신호".to_string()
        }
    }
}

/// 차트 패턴 이름 반환 (한글).
fn get_chart_pattern_name(pattern_type: &ChartPatternType) -> String {
    match pattern_type {
        ChartPatternType::HeadAndShoulders => "머리어깨형".to_string(),
        ChartPatternType::InverseHeadAndShoulders => "역머리어깨형".to_string(),
        ChartPatternType::DoubleTop => "이중 천장".to_string(),
        ChartPatternType::DoubleBottom => "이중 바닥".to_string(),
        ChartPatternType::TripleTop => "삼중 천장".to_string(),
        ChartPatternType::TripleBottom => "삼중 바닥".to_string(),
        ChartPatternType::RoundingTop => "원형 천장".to_string(),
        ChartPatternType::RoundingBottom => "원형 바닥".to_string(),
        ChartPatternType::AscendingTriangle => "상승 삼각형".to_string(),
        ChartPatternType::DescendingTriangle => "하락 삼각형".to_string(),
        ChartPatternType::SymmetricalTriangle => "대칭 삼각형".to_string(),
        ChartPatternType::RisingWedge => "상승 쐐기형".to_string(),
        ChartPatternType::FallingWedge => "하락 쐐기형".to_string(),
        ChartPatternType::BullishFlag => "상승 깃발형".to_string(),
        ChartPatternType::BearishFlag => "하락 깃발형".to_string(),
        ChartPatternType::BullishPennant => "상승 페넌트".to_string(),
        ChartPatternType::BearishPennant => "하락 페넌트".to_string(),
        ChartPatternType::AscendingChannel => "상승 채널".to_string(),
        ChartPatternType::DescendingChannel => "하락 채널".to_string(),
        ChartPatternType::HorizontalChannel => "수평 채널 (박스권)".to_string(),
        ChartPatternType::CupAndHandle => "컵앤핸들".to_string(),
        ChartPatternType::InverseCupAndHandle => "역컵앤핸들".to_string(),
        ChartPatternType::BroadeningFormation => "확대형".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::Timeframe;

    fn create_test_klines(count: usize) -> Vec<Kline> {
        let ticker = "BTC/USDT".to_string();
        let base_price = 50000.0;

        (0..count)
            .map(|i| {
                let variation = (i as f64 * 0.1).sin() * 1000.0;
                let open = base_price + variation;
                let close = open + (i as f64 % 3.0 - 1.0) * 100.0;
                let high = open.max(close) + 50.0;
                let low = open.min(close) - 50.0;

                Kline::new(
                    ticker.clone(),
                    Timeframe::H1,
                    Utc::now(),
                    rust_decimal::Decimal::from_f64_retain(open).unwrap_or(dec!(50000)),
                    rust_decimal::Decimal::from_f64_retain(high).unwrap_or(dec!(50050)),
                    rust_decimal::Decimal::from_f64_retain(low).unwrap_or(dec!(49950)),
                    rust_decimal::Decimal::from_f64_retain(close).unwrap_or(dec!(50000)),
                    dec!(100) + rust_decimal::Decimal::from(i as u32),
                    Utc::now(),
                )
            })
            .collect()
    }

    #[test]
    fn test_ml_service_creation() {
        let service = MlService::with_defaults().unwrap();
        assert!(service.config().enable_candlestick_patterns);
        assert!(service.config().enable_chart_patterns);
    }

    #[test]
    fn test_pattern_detection() {
        let service = MlService::with_defaults().unwrap();
        let klines = create_test_klines(50);

        let result = service.detect_patterns("BTC/USDT", "1h", &klines);

        assert_eq!(result.symbol, "BTC/USDT");
        assert_eq!(result.timeframe, "1h");
        assert!(!result.overall_signal.is_empty());
    }

    #[tokio::test]
    async fn test_full_analysis() {
        let service = MlService::with_defaults().unwrap();
        let klines = create_test_klines(100);

        let result = service.analyze("BTC/USDT", &klines).await.unwrap();

        assert_eq!(result.symbol, "BTC/USDT");
        assert!(result.feature_summary.is_some());
    }
}
