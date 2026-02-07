//! 통합 API 에러 응답 타입.
//!
//! 모든 API 엔드포인트에서 일관된 에러 형식을 제공합니다.
//!
//! # 마이그레이션 가이드
//!
//! 기존 타입들은 이 모듈의 `ApiErrorResponse`로 통합되었습니다:
//! - `strategies::ApiError` → `ApiErrorResponse`
//! - `backtest::BacktestApiError` → `ApiErrorResponse`
//! - `simulation::SimulationApiError` → `ApiErrorResponse`
//! - `ml::ErrorResponse` → `ApiErrorResponse` (필드명: error → code)

use axum::http::{Method, Uri};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// 통합 API 에러 응답.
///
/// 모든 API 엔드포인트에서 일관된 에러 형식을 제공합니다.
///
/// # 예시
///
/// ```json
/// {
///   "code": "STRATEGY_NOT_FOUND",
///   "message": "전략을 찾을 수 없습니다: rsi-123",
///   "details": null,
///   "timestamp": 1738300800
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorResponse {
    /// 에러 코드 (예: "DB_ERROR", "INVALID_INPUT", "NOT_FOUND")
    pub code: String,
    /// 사람이 읽을 수 있는 에러 메시지
    pub message: String,
    /// 추가 에러 상세 정보 (선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// 에러 발생 타임스탬프 (Unix timestamp, 선택적)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    /// HTTP 메서드 (GET, POST 등)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// 요청 경로
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ApiErrorResponse {
    /// 기본 에러 생성 (타임스탬프 포함).
    ///
    /// # Arguments
    ///
    /// * `code` - 에러 코드
    /// * `message` - 에러 메시지
    ///
    /// # Example
    ///
    /// ```
    /// use trader_api::error::ApiErrorResponse;
    ///
    /// let error = ApiErrorResponse::new("NOT_FOUND", "Strategy not found");
    /// ```
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            timestamp: Some(chrono::Utc::now().timestamp()),
            method: None,
            path: None,
        }
    }

    /// 상세 정보 포함 에러 생성.
    ///
    /// # Arguments
    ///
    /// * `code` - 에러 코드
    /// * `message` - 에러 메시지
    /// * `details` - 추가 상세 정보 (JSON 값)
    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
            timestamp: Some(chrono::Utc::now().timestamp()),
            method: None,
            path: None,
        }
    }

    /// 타임스탬프 없는 간단한 에러 (기존 API 호환성용).
    ///
    /// 기존 `ApiError`, `BacktestApiError`, `SimulationApiError`와
    /// 동일한 JSON 출력을 생성합니다.
    ///
    /// # Arguments
    ///
    /// * `code` - 에러 코드
    /// * `message` - 에러 메시지
    pub fn simple(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            timestamp: None,
            method: None,
            path: None,
        }
    }

    /// 요청 정보(메서드, 경로)를 추가합니다.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP 메서드 (GET, POST 등)
    /// * `uri` - 요청 URI
    ///
    /// # Example
    ///
    /// ```ignore
    /// use axum::http::{Method, Uri};
    /// use trader_api::error::ApiErrorResponse;
    ///
    /// let error = ApiErrorResponse::new("NOT_FOUND", "Resource not found")
    ///     .with_request_info(&Method::GET, &"/api/strategies/123".parse::<Uri>().unwrap());
    /// ```
    #[must_use]
    pub fn with_request_info(mut self, method: &Method, uri: &Uri) -> Self {
        self.method = Some(method.to_string());
        self.path = Some(uri.path().to_string());
        self
    }

    /// 에러 코드 반환.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// 에러 메시지 반환.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ApiErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ApiErrorResponse {}

// ==================== Type Aliases (점진적 마이그레이션용) ====================

/// 기존 `strategies::ApiError` 호환 타입 별칭.
///
/// **Deprecated**: 새 코드에서는 `ApiErrorResponse`를 직접 사용하세요.
pub type ApiError = ApiErrorResponse;

/// 기존 `backtest::BacktestApiError` 호환 타입 별칭.
///
/// **Deprecated**: 새 코드에서는 `ApiErrorResponse`를 직접 사용하세요.
pub type BacktestApiError = ApiErrorResponse;

/// 기존 `simulation::SimulationApiError` 호환 타입 별칭.
///
/// **Deprecated**: 새 코드에서는 `ApiErrorResponse`를 직접 사용하세요.
pub type SimulationApiError = ApiErrorResponse;

// ==================== Result Type Alias ====================

/// API 핸들러 Result 타입 별칭.
///
/// # Example
///
/// ```ignore
/// async fn get_strategy(
///     Path(id): Path<String>,
///     State(state): State<Arc<AppState>>,
/// ) -> ApiResult<Json<Strategy>> {
///     let strategy = state.strategy_repo
///         .find_by_id(&id)
///         .await
///         .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiErrorResponse::new("DB_ERROR", e.to_string()))))?
///         .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiErrorResponse::new("NOT_FOUND", format!("Strategy {} not found", id)))))?;
///
///     Ok(Json(strategy))
/// }
/// ```
pub type ApiResult<T> = Result<T, (axum::http::StatusCode, axum::Json<ApiErrorResponse>)>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_response_new() {
        let error = ApiErrorResponse::new("TEST_ERROR", "Test message");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "Test message");
        assert!(error.timestamp.is_some());
        assert!(error.details.is_none());
        assert!(error.method.is_none());
        assert!(error.path.is_none());
    }

    #[test]
    fn test_api_error_response_simple() {
        let error = ApiErrorResponse::simple("TEST_ERROR", "Test message");
        assert_eq!(error.code, "TEST_ERROR");
        assert_eq!(error.message, "Test message");
        assert!(error.timestamp.is_none());
        assert!(error.details.is_none());
        assert!(error.method.is_none());
        assert!(error.path.is_none());
    }

    #[test]
    fn test_api_error_response_with_details() {
        let details = serde_json::json!({"field": "symbol", "reason": "invalid format"});
        let error = ApiErrorResponse::with_details("VALIDATION_ERROR", "Invalid input", details);
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert!(error.details.is_some());
    }

    #[test]
    fn test_type_aliases() {
        // 기존 타입 별칭이 동일하게 작동하는지 확인
        let api_error: ApiError = ApiErrorResponse::simple("CODE", "msg");
        let backtest_error: BacktestApiError = ApiErrorResponse::simple("CODE", "msg");
        let simulation_error: SimulationApiError = ApiErrorResponse::simple("CODE", "msg");

        assert_eq!(api_error.code, backtest_error.code);
        assert_eq!(backtest_error.code, simulation_error.code);
    }

    #[test]
    fn test_json_serialization_simple() {
        let error = ApiErrorResponse::simple("NOT_FOUND", "Resource not found");
        let json = serde_json::to_string(&error).unwrap();

        // timestamp와 details가 없어야 함 (기존 API 호환)
        assert!(!json.contains("timestamp"));
        assert!(!json.contains("details"));
        assert!(!json.contains("method"));
        assert!(!json.contains("path"));
        assert!(json.contains(r#""code":"NOT_FOUND""#));
        assert!(json.contains(r#""message":"Resource not found""#));
    }

    #[test]
    fn test_with_request_info() {
        use axum::http::{Method, Uri};

        let uri: Uri = "/api/strategies/123".parse().unwrap();
        let error = ApiErrorResponse::new("NOT_FOUND", "Strategy not found")
            .with_request_info(&Method::GET, &uri);

        assert_eq!(error.method, Some("GET".to_string()));
        assert_eq!(error.path, Some("/api/strategies/123".to_string()));

        // JSON 직렬화 확인
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains(r#""method":"GET""#));
        assert!(json.contains(r#""path":"/api/strategies/123""#));
    }

    #[test]
    fn test_with_request_info_post() {
        use axum::http::{Method, Uri};

        let uri: Uri = "/api/backtest/run".parse().unwrap();
        let error = ApiErrorResponse::with_details(
            "VALIDATION_ERROR",
            "Invalid parameters",
            serde_json::json!({"field": "symbol"}),
        )
        .with_request_info(&Method::POST, &uri);

        assert_eq!(error.method, Some("POST".to_string()));
        assert_eq!(error.path, Some("/api/backtest/run".to_string()));
        assert!(error.details.is_some());
        assert!(error.timestamp.is_some());
    }
}
