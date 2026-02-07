//! API 응답 래퍼 타입들.
//!
//! 이 모듈은 일관된 API 응답 형식을 위한 제네릭 래퍼 타입들을 제공합니다.

use serde::Serialize;

/// 리스트 응답을 위한 제네릭 래퍼.
///
/// 페이지네이션 정보와 함께 아이템 목록을 반환합니다.
///
/// # Example
///
/// ```
/// use trader_api::utils::response::ListResponse;
///
/// let response = ListResponse {
///     items: vec!["item1", "item2"],
///     total: 100,
///     page: Some(1),
///     per_page: Some(10),
/// };
/// ```
#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    /// 아이템 목록
    pub items: Vec<T>,
    /// 전체 아이템 수
    pub total: usize,
    /// 현재 페이지 번호 (1-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    /// 페이지당 아이템 수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<usize>,
}

impl<T> ListResponse<T> {
    /// 새로운 ListResponse를 생성합니다.
    pub fn new(items: Vec<T>, total: usize) -> Self {
        Self {
            items,
            total,
            page: None,
            per_page: None,
        }
    }

    /// 페이지네이션 정보를 포함한 ListResponse를 생성합니다.
    pub fn with_pagination(items: Vec<T>, total: usize, page: usize, per_page: usize) -> Self {
        Self {
            items,
            total,
            page: Some(page),
            per_page: Some(per_page),
        }
    }
}

/// 응답 메타데이터.
///
/// 응답에 대한 추가 정보를 포함합니다.
#[derive(Debug, Serialize)]
pub struct ResponseMetadata {
    /// 응답 생성 시각 (ISO 8601 형식)
    pub timestamp: String,
    /// 요청 추적을 위한 고유 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ResponseMetadata {
    /// 현재 시각으로 새로운 메타데이터를 생성합니다.
    pub fn now() -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: None,
        }
    }

    /// 요청 ID를 포함한 메타데이터를 생성합니다.
    pub fn with_request_id(request_id: String) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: Some(request_id),
        }
    }
}

impl Default for ResponseMetadata {
    fn default() -> Self {
        Self::now()
    }
}

/// 단일 엔티티 응답을 위한 제네릭 래퍼.
///
/// 메타데이터와 함께 단일 데이터 객체를 반환합니다.
///
/// # Example
///
/// ```
/// use trader_api::utils::response::{EntityResponse, ResponseMetadata};
///
/// #[derive(serde::Serialize)]
/// struct User {
///     id: u64,
///     name: String,
/// }
///
/// let user = User { id: 1, name: "Alice".to_string() };
/// let response = EntityResponse::new(user);
/// ```
#[derive(Debug, Serialize)]
pub struct EntityResponse<T> {
    /// 응답 데이터
    pub data: T,
    /// 응답 메타데이터
    pub metadata: ResponseMetadata,
}

impl<T> EntityResponse<T> {
    /// 새로운 EntityResponse를 생성합니다.
    pub fn new(data: T) -> Self {
        Self {
            data,
            metadata: ResponseMetadata::now(),
        }
    }

    /// 요청 ID를 포함한 EntityResponse를 생성합니다.
    pub fn with_request_id(data: T, request_id: String) -> Self {
        Self {
            data,
            metadata: ResponseMetadata::with_request_id(request_id),
        }
    }
}

/// 데이터 없는 성공 응답.
///
/// 작업 성공을 나타내는 메시지만 반환합니다.
///
/// # Example
///
/// ```
/// use trader_api::utils::response::SuccessResponse;
///
/// let response = SuccessResponse::new("Operation completed successfully");
/// let detailed = SuccessResponse::with_details(
///     "Resource deleted",
///     "Resource ID: 123 has been permanently removed"
/// );
/// ```
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    /// 성공 메시지
    pub message: String,
    /// 추가 상세 정보
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl SuccessResponse {
    /// 새로운 SuccessResponse를 생성합니다.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    /// 상세 정보를 포함한 SuccessResponse를 생성합니다.
    pub fn with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_list_response_serialization() {
        let response = ListResponse::new(vec![1, 2, 3], 100);
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"items\":[1,2,3]"));
        assert!(json.contains("\"total\":100"));
        // page와 per_page는 None이므로 직렬화되지 않음
        assert!(!json.contains("\"page\""));
        assert!(!json.contains("\"per_page\""));
    }

    #[test]
    fn test_list_response_with_pagination() {
        let response = ListResponse::with_pagination(vec!["a", "b"], 50, 2, 10);
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"page\":2"));
        assert!(json.contains("\"per_page\":10"));
    }

    #[test]
    fn test_entity_response_serialization() {
        #[derive(Debug, Serialize)]
        struct TestData {
            id: u64,
        }

        let response = EntityResponse::new(TestData { id: 42 });
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"timestamp\""));
        // request_id는 None이므로 직렬화되지 않음
        assert!(!json.contains("\"request_id\""));
    }

    #[test]
    fn test_entity_response_with_request_id() {
        let response = EntityResponse::with_request_id("test", "req-123".to_string());
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"request_id\":\"req-123\""));
    }

    #[test]
    fn test_success_response_serialization() {
        let response = SuccessResponse::new("Success");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"message\":\"Success\""));
        assert!(!json.contains("\"details\""));
    }

    #[test]
    fn test_success_response_with_details() {
        let response = SuccessResponse::with_details("Deleted", "ID: 123");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"message\":\"Deleted\""));
        assert!(json.contains("\"details\":\"ID: 123\""));
    }

    #[test]
    fn test_response_metadata_default() {
        let metadata = ResponseMetadata::default();
        assert!(metadata.request_id.is_none());
        assert!(!metadata.timestamp.is_empty());
    }
}
