//! 역할 기반 접근 제어 (RBAC).
//!
//! 사용자 역할 및 권한 정의.

use serde::{Deserialize, Serialize};

/// 사용자 역할.
///
/// 시스템에서 사용자의 권한 수준을 정의합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 관리자 - 모든 권한 보유
    Admin,
    /// 트레이더 - 거래 및 전략 관리 권한
    Trader,
    /// 뷰어 - 읽기 전용 권한
    Viewer,
}

impl Role {
    /// 역할이 특정 권한을 가지는지 확인.
    pub fn has_permission(&self, permission: Permission) -> bool {
        match self {
            Role::Admin => true, // Admin은 모든 권한 보유
            Role::Trader => matches!(
                permission,
                Permission::ViewDashboard
                    | Permission::ViewPositions
                    | Permission::ViewOrders
                    | Permission::ViewStrategies
                    | Permission::ManageOrders
                    | Permission::ManageStrategies
                    | Permission::ViewAnalytics
            ),
            Role::Viewer => matches!(
                permission,
                Permission::ViewDashboard
                    | Permission::ViewPositions
                    | Permission::ViewOrders
                    | Permission::ViewStrategies
                    | Permission::ViewAnalytics
            ),
        }
    }

    /// 역할의 우선순위 레벨 반환 (높을수록 더 많은 권한).
    pub fn level(&self) -> u8 {
        match self {
            Role::Admin => 100,
            Role::Trader => 50,
            Role::Viewer => 10,
        }
    }

    /// 문자열에서 역할 파싱.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "admin" => Some(Role::Admin),
            "trader" => Some(Role::Trader),
            "viewer" => Some(Role::Viewer),
            _ => None,
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Role::Admin => "admin",
            Role::Trader => "trader",
            Role::Viewer => "viewer",
        };
        write!(f, "{}", s)
    }
}

/// 시스템 권한.
///
/// 각 작업에 필요한 권한을 정의합니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// 대시보드 조회
    ViewDashboard,
    /// 포지션 조회
    ViewPositions,
    /// 주문 조회
    ViewOrders,
    /// 전략 조회
    ViewStrategies,
    /// 분석 데이터 조회
    ViewAnalytics,
    /// 주문 생성/취소
    ManageOrders,
    /// 전략 시작/중지/설정
    ManageStrategies,
    /// 사용자 관리
    ManageUsers,
    /// 시스템 설정 관리
    ManageSystem,
    /// 리스크 한도 설정
    ManageRisk,
}

impl Permission {
    /// 권한에 대한 설명 반환.
    pub fn description(&self) -> &'static str {
        match self {
            Permission::ViewDashboard => "대시보드 조회",
            Permission::ViewPositions => "포지션 조회",
            Permission::ViewOrders => "주문 조회",
            Permission::ViewStrategies => "전략 조회",
            Permission::ViewAnalytics => "분석 데이터 조회",
            Permission::ManageOrders => "주문 관리",
            Permission::ManageStrategies => "전략 관리",
            Permission::ManageUsers => "사용자 관리",
            Permission::ManageSystem => "시스템 설정 관리",
            Permission::ManageRisk => "리스크 한도 관리",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        // Admin은 모든 권한 보유
        assert!(Role::Admin.has_permission(Permission::ManageUsers));
        assert!(Role::Admin.has_permission(Permission::ManageSystem));
        assert!(Role::Admin.has_permission(Permission::ViewDashboard));

        // Trader는 거래 관련 권한만
        assert!(Role::Trader.has_permission(Permission::ManageOrders));
        assert!(Role::Trader.has_permission(Permission::ManageStrategies));
        assert!(!Role::Trader.has_permission(Permission::ManageUsers));
        assert!(!Role::Trader.has_permission(Permission::ManageSystem));

        // Viewer는 읽기만
        assert!(Role::Viewer.has_permission(Permission::ViewDashboard));
        assert!(Role::Viewer.has_permission(Permission::ViewOrders));
        assert!(!Role::Viewer.has_permission(Permission::ManageOrders));
        assert!(!Role::Viewer.has_permission(Permission::ManageStrategies));
    }

    #[test]
    fn test_role_level() {
        assert!(Role::Admin.level() > Role::Trader.level());
        assert!(Role::Trader.level() > Role::Viewer.level());
    }

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::parse("admin"), Some(Role::Admin));
        assert_eq!(Role::parse("TRADER"), Some(Role::Trader));
        assert_eq!(Role::parse("Viewer"), Some(Role::Viewer));
        assert_eq!(Role::parse("unknown"), None);
    }

    #[test]
    fn test_role_serialization() {
        let role = Role::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"admin\"");

        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Role::Admin);
    }
}
