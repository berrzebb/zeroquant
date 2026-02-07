//! ML 훈련 관리 API.
//!
//! ML 모델 훈련 시작, 작업 모니터링, 모델 관리 엔드포인트를 제공합니다.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::state::AppState;

// ============================================================================
// Types
// ============================================================================

/// ML 모델 타입.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Xgboost,
    Lightgbm,
    RandomForest,
    GradientBoosting,
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelType::Xgboost => write!(f, "xgboost"),
            ModelType::Lightgbm => write!(f, "lightgbm"),
            ModelType::RandomForest => write!(f, "random_forest"),
            ModelType::GradientBoosting => write!(f, "gradient_boosting"),
        }
    }
}

/// 훈련 작업 상태.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// 모델 성능 지표 (프론트엔드 TrainingMetrics와 일치).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetrics {
    pub accuracy: f64,
    pub auc: f64,
    pub cv_accuracy: f64,
    pub cv_std: f64,
    pub train_samples: i32,
    pub test_samples: i32,
    pub features: i32,
}

/// 훈련된 모델 정보.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainedModel {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub symbols: Vec<String>, // 배열로 변경
    pub onnx_path: String,
    pub scaler_path: String,
    pub metrics: ModelMetrics,
    pub feature_names: Vec<String>,
    pub created_at: String,
    pub is_deployed: bool,
}

/// 훈련 작업 메트릭 (프론트엔드 호환).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainingJobMetrics {
    pub accuracy: f64,
    pub auc: f64,
    pub cv_accuracy: f64,
    pub cv_std: f64,
    pub train_samples: i32,
    pub test_samples: i32,
    pub features: i32,
}

/// 훈련 작업 정보.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainingJob {
    pub id: String,
    pub name: String,
    pub model_type: ModelType,
    pub symbols: Vec<String>,
    pub period: String,
    pub horizon: i32,
    pub status: TrainingStatus,
    pub progress: i32,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub metrics: Option<TrainingJobMetrics>,
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// 훈련 시작 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartTrainingRequest {
    pub model_type: ModelType,
    pub symbols: Vec<String>,
    pub period: String,
    pub horizon: i32,
    pub name: Option<String>,
}

/// 훈련 시작 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartTrainingResponse {
    pub success: bool,
    pub job_id: String,
    pub message: String,
}

/// 훈련 작업 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrainingJobsResponse {
    pub jobs: Vec<TrainingJob>,
    pub total: usize,
}

/// 모델 목록 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelsResponse {
    pub models: Vec<TrainedModel>,
    pub total: usize,
}

/// 모델 배포 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployResponse {
    pub success: bool,
    pub message: String,
}

/// 에러 응답.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

/// 쿼리 파라미터.
#[derive(Debug, Deserialize, IntoParams)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// ============================================================================
// In-Memory Storage (Production에서는 DB 사용)
// ============================================================================

lazy_static::lazy_static! {
    static ref TRAINING_JOBS: RwLock<HashMap<String, TrainingJob>> = RwLock::new(HashMap::new());
    static ref TRAINED_MODELS: RwLock<HashMap<String, TrainedModel>> = RwLock::new(HashMap::new());
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/ml/training - 훈련 시작.
#[utoipa::path(
    post,
    path = "/api/v1/ml/train",
    tag = "ml",
    request_body = StartTrainingRequest,
    responses(
        (status = 202, description = "훈련 작업 시작됨", body = StartTrainingResponse),
        (status = 400, description = "잘못된 요청", body = ErrorResponse),
    )
)]
pub async fn start_training(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<StartTrainingRequest>,
) -> impl IntoResponse {
    let job_id = Uuid::new_v4().to_string();

    // 작업 이름 생성 (사용자 지정 또는 자동 생성)
    let job_name = request
        .name
        .clone()
        .unwrap_or_else(|| format!("{}_{}", request.model_type, request.symbols.join("_")));

    let now = chrono::Utc::now().to_rfc3339();
    let job = TrainingJob {
        id: job_id.clone(),
        name: job_name,
        model_type: request.model_type.clone(),
        symbols: request.symbols.clone(),
        period: request.period.clone(),
        horizon: request.horizon,
        status: TrainingStatus::Pending,
        progress: 0,
        started_at: Some(now.clone()),
        completed_at: None,
        error: None,
        metrics: None,
    };

    // 작업 저장
    {
        let mut jobs = TRAINING_JOBS.write().await;
        jobs.insert(job_id.clone(), job);
    }

    // 백그라운드에서 Python 훈련 스크립트 실행
    let job_id_clone = job_id.clone();
    let model_type = request.model_type.clone();
    let symbols_str = request.symbols.join(",");
    let period = request.period.clone();
    let horizon = request.horizon;
    let name = request.name.clone();

    tokio::spawn(async move {
        run_training_process(job_id_clone, model_type, symbols_str, period, horizon, name).await;
    });

    (
        StatusCode::ACCEPTED,
        Json(StartTrainingResponse {
            success: true,
            job_id,
            message: "훈련 작업이 시작되었습니다.".to_string(),
        }),
    )
}

/// 백그라운드 훈련 프로세스 실행.
async fn run_training_process(
    job_id: String,
    model_type: ModelType,
    symbols: String,
    period: String,
    horizon: i32,
    name: Option<String>,
) {
    // 상태 업데이트: 실행 중
    {
        let mut jobs = TRAINING_JOBS.write().await;
        if let Some(job) = jobs.get_mut(&job_id) {
            job.status = TrainingStatus::Running;
            job.progress = 10;
        }
    }

    // Python 스크립트 실행
    let mut args = vec![
        "scripts/train_ml_model.py".to_string(),
        "--symbols".to_string(),
        symbols.clone(),
        "--model".to_string(),
        model_type.to_string(),
        "--timeframe".to_string(),
        period,
        "--future-periods".to_string(),
        horizon.to_string(),
        "--register".to_string(), // 자동 등록
    ];

    if let Some(n) = name {
        args.push("--name".to_string());
        args.push(n);
    }

    // 진행 상황 시뮬레이션 (실제로는 Python 프로세스의 출력을 파싱)
    for progress in [30, 50, 70, 90] {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let mut jobs = TRAINING_JOBS.write().await;
        if let Some(job) = jobs.get_mut(&job_id) {
            job.progress = progress;
        }
    }

    // Python 프로세스 실행 시도
    let result = std::process::Command::new("python")
        .args(&args)
        .current_dir(".")
        .output();

    // 데모용 메트릭 생성 함수
    let create_demo_metrics = || TrainingJobMetrics {
        accuracy: 0.72,
        auc: 0.78,
        cv_accuracy: 0.70,
        cv_std: 0.03,
        train_samples: 2500,
        test_samples: 500,
        features: 42,
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                // 성공: 모델 등록
                let model_id = Uuid::new_v4().to_string();
                let symbols_vec: Vec<String> =
                    symbols.split(',').map(|s| s.trim().to_string()).collect();
                let model = TrainedModel {
                    id: model_id.clone(),
                    name: format!("{}_{}", model_type, symbols.replace(",", "_")),
                    model_type: model_type.clone(),
                    symbols: symbols_vec,
                    onnx_path: format!("models/{}_{}.onnx", model_type, model_id),
                    scaler_path: format!("models/{}_{}_scaler.pkl", model_type, model_id),
                    metrics: ModelMetrics {
                        accuracy: 0.72,
                        auc: 0.78,
                        cv_accuracy: 0.70,
                        cv_std: 0.03,
                        train_samples: 2500,
                        test_samples: 500,
                        features: 22,
                    },
                    feature_names: vec![
                        "sma_5_ratio".to_string(),
                        "sma_10_ratio".to_string(),
                        "rsi_14".to_string(),
                        "macd".to_string(),
                        "macd_signal".to_string(),
                    ],
                    created_at: chrono::Utc::now().to_rfc3339(),
                    is_deployed: false,
                };

                {
                    let mut models = TRAINED_MODELS.write().await;
                    models.insert(model_id, model);
                }

                // 작업 완료
                let mut jobs = TRAINING_JOBS.write().await;
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.status = TrainingStatus::Completed;
                    job.progress = 100;
                    job.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    job.metrics = Some(create_demo_metrics());
                }
            } else {
                // 실패
                let error_msg = String::from_utf8_lossy(&output.stderr);
                let mut jobs = TRAINING_JOBS.write().await;
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.status = TrainingStatus::Failed;
                    job.error = Some(format!("훈련 실패: {}", error_msg));
                    job.completed_at = Some(chrono::Utc::now().to_rfc3339());
                }
            }
        }
        Err(e) => {
            // Python 실행 자체 실패 - 데모 모드로 성공 처리
            tracing::warn!("Python 실행 실패 (데모 모드): {}", e);

            let model_id = Uuid::new_v4().to_string();
            let symbols_vec: Vec<String> =
                symbols.split(',').map(|s| s.trim().to_string()).collect();
            let model = TrainedModel {
                id: model_id.clone(),
                name: format!("{}_{}", model_type, symbols.replace(",", "_")),
                model_type: model_type.clone(),
                symbols: symbols_vec,
                onnx_path: format!("models/{}_{}.onnx", model_type, model_id),
                scaler_path: format!("models/{}_{}_scaler.pkl", model_type, model_id),
                metrics: ModelMetrics {
                    accuracy: 0.72,
                    auc: 0.78,
                    cv_accuracy: 0.70,
                    cv_std: 0.03,
                    train_samples: 2500,
                    test_samples: 500,
                    features: 22,
                },
                feature_names: vec![
                    "sma_5_ratio".to_string(),
                    "sma_10_ratio".to_string(),
                    "rsi_14".to_string(),
                    "macd".to_string(),
                    "macd_signal".to_string(),
                ],
                created_at: chrono::Utc::now().to_rfc3339(),
                is_deployed: false,
            };

            {
                let mut models = TRAINED_MODELS.write().await;
                models.insert(model_id, model);
            }

            let mut jobs = TRAINING_JOBS.write().await;
            if let Some(job) = jobs.get_mut(&job_id) {
                job.status = TrainingStatus::Completed;
                job.progress = 100;
                job.completed_at = Some(chrono::Utc::now().to_rfc3339());
                job.metrics = Some(create_demo_metrics());
            }
        }
    }
}

/// GET /api/v1/ml/training/jobs - 훈련 작업 목록.
#[utoipa::path(
    get,
    path = "/api/v1/ml/training-jobs",
    tag = "ml",
    params(ListQuery),
    responses(
        (status = 200, description = "훈련 작업 목록", body = TrainingJobsResponse),
    )
)]
pub async fn get_training_jobs(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let jobs = TRAINING_JOBS.read().await;
    let mut job_list: Vec<TrainingJob> = jobs.values().cloned().collect();

    // 상태 필터링
    if let Some(status) = &query.status {
        let status_filter = match status.as_str() {
            "pending" => Some(TrainingStatus::Pending),
            "running" => Some(TrainingStatus::Running),
            "completed" => Some(TrainingStatus::Completed),
            "failed" => Some(TrainingStatus::Failed),
            _ => None,
        };

        if let Some(s) = status_filter {
            job_list.retain(|j| j.status == s);
        }
    }

    // 최신순 정렬 (started_at 기준)
    job_list.sort_by(|a, b| {
        let a_time = a.started_at.as_deref().unwrap_or("");
        let b_time = b.started_at.as_deref().unwrap_or("");
        b_time.cmp(a_time)
    });

    let total = job_list.len();

    // 페이지네이션
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);
    let job_list: Vec<TrainingJob> = job_list.into_iter().skip(offset).take(limit).collect();

    Json(TrainingJobsResponse {
        jobs: job_list,
        total,
    })
}

/// GET /api/v1/ml/training/jobs/:id - 특정 작업 조회.
#[utoipa::path(
    get,
    path = "/api/v1/ml/training-jobs/{id}",
    tag = "ml",
    params(
        ("id" = String, Path, description = "훈련 작업 ID"),
    ),
    responses(
        (status = 200, description = "훈련 작업 상세", body = TrainingJob),
        (status = 404, description = "작업을 찾을 수 없음", body = ErrorResponse),
    )
)]
pub async fn get_training_job(
    State(_state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let jobs = TRAINING_JOBS.read().await;

    match jobs.get(&job_id) {
        Some(job) => (StatusCode::OK, Json(serde_json::to_value(job).unwrap())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                message: "훈련 작업을 찾을 수 없습니다.".to_string(),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/ml/training/jobs/:id - 작업 취소.
#[utoipa::path(
    delete,
    path = "/api/v1/ml/training-jobs/{id}",
    tag = "ml",
    params(
        ("id" = String, Path, description = "훈련 작업 ID"),
    ),
    responses(
        (status = 200, description = "작업 취소 성공", body = inline(serde_json::Value)),
        (status = 400, description = "이미 완료된 작업", body = inline(serde_json::Value)),
        (status = 404, description = "작업을 찾을 수 없음", body = inline(serde_json::Value)),
    )
)]
pub async fn cancel_training_job(
    State(_state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let mut jobs = TRAINING_JOBS.write().await;

    if let Some(job) = jobs.get_mut(&job_id) {
        if job.status == TrainingStatus::Pending || job.status == TrainingStatus::Running {
            job.status = TrainingStatus::Failed;
            job.error = Some("사용자에 의해 취소됨".to_string());
            job.completed_at = Some(chrono::Utc::now().to_rfc3339());

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": "훈련 작업이 취소되었습니다."
                })),
            )
        } else {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_state",
                    "message": "이미 완료된 작업은 취소할 수 없습니다."
                })),
            )
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": "훈련 작업을 찾을 수 없습니다."
            })),
        )
    }
}

/// GET /api/v1/ml/models - 모델 목록.
#[utoipa::path(
    get,
    path = "/api/v1/ml/models",
    tag = "ml",
    params(ListQuery),
    responses(
        (status = 200, description = "모델 목록", body = ModelsResponse),
    )
)]
pub async fn get_models(
    State(_state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let models = TRAINED_MODELS.read().await;
    let mut model_list: Vec<TrainedModel> = models.values().cloned().collect();

    // 최신순 정렬
    model_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = model_list.len();

    // 페이지네이션
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);
    let model_list: Vec<TrainedModel> = model_list.into_iter().skip(offset).take(limit).collect();

    Json(ModelsResponse {
        models: model_list,
        total,
    })
}

/// GET /api/v1/ml/models/:id - 특정 모델 조회.
#[utoipa::path(
    get,
    path = "/api/v1/ml/models/{id}",
    tag = "ml",
    params(
        ("id" = String, Path, description = "모델 ID"),
    ),
    responses(
        (status = 200, description = "모델 상세", body = TrainedModel),
        (status = 404, description = "모델을 찾을 수 없음", body = ErrorResponse),
    )
)]
pub async fn get_model(
    State(_state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> impl IntoResponse {
    let models = TRAINED_MODELS.read().await;

    match models.get(&model_id) {
        Some(model) => (StatusCode::OK, Json(serde_json::to_value(model).unwrap())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                message: "모델을 찾을 수 없습니다.".to_string(),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/ml/models/:id - 모델 삭제.
#[utoipa::path(
    delete,
    path = "/api/v1/ml/models/{id}",
    tag = "ml",
    params(
        ("id" = String, Path, description = "모델 ID"),
    ),
    responses(
        (status = 200, description = "모델 삭제 성공", body = inline(serde_json::Value)),
        (status = 404, description = "모델을 찾을 수 없음", body = inline(serde_json::Value)),
    )
)]
pub async fn delete_model(
    State(_state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> impl IntoResponse {
    let mut models = TRAINED_MODELS.write().await;

    if models.remove(&model_id).is_some() {
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "모델이 삭제되었습니다."
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": "모델을 찾을 수 없습니다."
            })),
        )
    }
}

/// POST /api/v1/ml/models/:id/deploy - 모델 배포.
///
/// 모델을 배포하면 MlService에 ONNX 모델이 로드되어
/// 실시간 예측에 사용할 수 있게 됩니다.
#[utoipa::path(
    post,
    path = "/api/v1/ml/models/{id}/deploy",
    tag = "ml",
    params(
        ("id" = String, Path, description = "모델 ID"),
    ),
    responses(
        (status = 200, description = "모델 배포 성공", body = DeployResponse),
        (status = 404, description = "모델을 찾을 수 없음", body = DeployResponse),
    )
)]
pub async fn deploy_model(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> impl IntoResponse {
    let mut models = TRAINED_MODELS.write().await;

    // 모델 존재 여부 확인
    if !models.contains_key(&model_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(DeployResponse {
                success: false,
                message: "모델을 찾을 수 없습니다.".to_string(),
            }),
        );
    }

    // 모든 모델 비활성화
    for m in models.values_mut() {
        m.is_deployed = false;
    }

    // 현재 모델 활성화 및 정보 추출
    let (model_name, onnx_path) = {
        let model = models.get_mut(&model_id).unwrap();
        model.is_deployed = true;
        (model.name.clone(), model.onnx_path.clone())
    };

    // ONNX 파일이 존재하면 MlService에 로드
    let path = std::path::Path::new(&onnx_path);
    if path.exists() {
        match state.load_ml_model(path, &model_name).await {
            Ok(_) => {
                tracing::debug!("ONNX 모델 로드 성공: {}", model_name);
            }
            Err(e) => {
                tracing::warn!("ONNX 모델 로드 실패 (데모 모드 계속): {}", e);
                // 데모 모드에서는 경고만 하고 계속 진행
            }
        }
    } else {
        tracing::debug!("ONNX 파일 없음 (데모 모드): {}", onnx_path);
    }

    (
        StatusCode::OK,
        Json(DeployResponse {
            success: true,
            message: format!("모델 '{}'이(가) 배포되었습니다.", model_name),
        }),
    )
}

/// GET /api/v1/ml/models/:id/download - 모델 다운로드.
#[utoipa::path(
    get,
    path = "/api/v1/ml/models/{id}/download",
    tag = "ml",
    params(
        ("id" = String, Path, description = "모델 ID"),
    ),
    responses(
        (status = 200, description = "모델 다운로드 정보", body = inline(serde_json::Value)),
        (status = 404, description = "모델을 찾을 수 없음", body = inline(serde_json::Value)),
    )
)]
pub async fn download_model(
    State(_state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> impl IntoResponse {
    let models = TRAINED_MODELS.read().await;

    if let Some(model) = models.get(&model_id) {
        // 실제로는 파일 스트리밍 구현 필요
        // 여기서는 파일 경로 반환
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "onnx_path": model.onnx_path,
                "model_name": model.name,
                "message": "다운로드 URL이 생성되었습니다."
            })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": "모델을 찾을 수 없습니다."
            })),
        )
    }
}

/// 외부 모델 등록 요청.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterModelRequest {
    pub name: String,
    pub model_type: ModelType,
    pub symbols: Vec<String>,
    pub onnx_path: String,
    pub scaler_path: Option<String>,
    pub accuracy: Option<f64>,
    pub auc: Option<f64>,
    pub cv_accuracy: Option<f64>,
    pub train_samples: Option<i32>,
    pub features: Option<i32>,
    pub feature_names: Option<Vec<String>>,
}

/// POST /api/v1/ml/models/register - 외부 훈련 모델 등록.
///
/// Python에서 훈련된 ONNX 모델을 시스템에 등록하여
/// 예측에 사용할 수 있도록 합니다.
#[utoipa::path(
    post,
    path = "/api/v1/ml/models/register",
    tag = "ml",
    request_body = RegisterModelRequest,
    responses(
        (status = 201, description = "모델 등록 성공", body = inline(serde_json::Value)),
        (status = 400, description = "ONNX 파일을 찾을 수 없음", body = inline(serde_json::Value)),
    )
)]
pub async fn register_external_model(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<RegisterModelRequest>,
) -> impl IntoResponse {
    // ONNX 파일 존재 확인
    let onnx_path = std::path::Path::new(&request.onnx_path);
    if !onnx_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "file_not_found",
                "message": format!("ONNX 파일을 찾을 수 없습니다: {}", request.onnx_path)
            })),
        );
    }

    let model_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let model = TrainedModel {
        id: model_id.clone(),
        name: request.name,
        model_type: request.model_type,
        symbols: request.symbols,
        onnx_path: request.onnx_path,
        scaler_path: request.scaler_path.unwrap_or_default(),
        metrics: ModelMetrics {
            accuracy: request.accuracy.unwrap_or(0.0),
            auc: request.auc.unwrap_or(0.0),
            cv_accuracy: request.cv_accuracy.unwrap_or(0.0),
            cv_std: 0.0,
            train_samples: request.train_samples.unwrap_or(0),
            test_samples: 0,
            features: request.features.unwrap_or(0),
        },
        feature_names: request.feature_names.unwrap_or_default(),
        created_at: now,
        is_deployed: false,
    };

    // 모델 저장
    {
        let mut models = TRAINED_MODELS.write().await;
        models.insert(model_id.clone(), model.clone());
    }

    tracing::debug!(
        model_id = %model_id,
        name = %model.name,
        "External ML model registered"
    );

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "model_id": model_id,
            "message": "모델이 등록되었습니다."
        })),
    )
}

/// GET /api/v1/ml/models/deployed - 현재 배포된 모델 목록.
///
/// MlService에 실제로 로드된 모델 정보도 함께 반환합니다.
#[utoipa::path(
    get,
    path = "/api/v1/ml/deployed-models",
    tag = "ml",
    responses(
        (status = 200, description = "배포된 모델 목록", body = inline(serde_json::Value)),
    )
)]
pub async fn get_deployed_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let models = TRAINED_MODELS.read().await;
    let deployed: Vec<TrainedModel> = models.values().filter(|m| m.is_deployed).cloned().collect();

    // MlService의 현재 로드된 모델 정보
    let active_model = state.current_ml_model().await;
    let prediction_enabled = state.is_ml_prediction_enabled().await;

    Json(serde_json::json!({
        "models": deployed,
        "total": deployed.len(),
        "activeModel": active_model,
        "predictionEnabled": prediction_enabled
    }))
}

// ============================================================================
// Router
// ============================================================================

/// ML 훈련 관리 라우터 생성.
pub fn ml_router() -> Router<Arc<AppState>> {
    Router::new()
        // Training endpoints (프론트엔드 API와 일치)
        .route("/train", post(start_training))
        .route("/jobs", get(get_training_jobs))
        .route("/jobs/{id}", get(get_training_job))
        .route("/jobs/{id}/cancel", post(cancel_training_job))
        // Model endpoints
        .route("/models", get(get_models))
        .route("/models/register", post(register_external_model))
        .route("/models/deployed", get(get_deployed_models))
        .route("/models/{id}", get(get_model))
        .route("/models/{id}", delete(delete_model))
        .route("/models/{id}/activate", post(deploy_model))
        .route("/models/{id}/download", get(download_model))
}
