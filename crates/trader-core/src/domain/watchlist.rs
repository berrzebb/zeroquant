//! 관심종목 도메인 모델.
//!
//! 사용자가 주시하고 싶은 종목을 그룹별로 관리합니다.
//!
//! # 구조
//!
//! - `Watchlist`: 관심종목 그룹 (예: "모멘텀 종목", "저평가 주")
//! - `WatchlistItem`: 그룹 내 개별 종목
//!
//! # 사용 예시
//!
//! ```rust,ignore
//! use trader_core::domain::{Watchlist, WatchlistItem};
//!
//! let watchlist = Watchlist::new("모멘텀 종목")
//!     .with_description("모멘텀 상위 10개 종목")
//!     .with_color("#10B981");
//!
//! let item = WatchlistItem::new(watchlist.id, "005930", "KR")
//!     .with_target_price(Decimal::from(75000))
//!     .with_memo("삼성전자 - 실적 개선 기대");
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 관심종목 그룹.
///
/// 관련 종목들을 하나의 그룹으로 묶어 관리합니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Watchlist {
    /// 고유 ID
    pub id: Uuid,

    /// 그룹 이름
    pub name: String,

    /// 설명
    #[serde(default)]
    pub description: Option<String>,

    /// 표시 순서 (낮을수록 먼저)
    #[serde(default)]
    pub sort_order: i32,

    /// 색상 코드 (예: "#FFD700")
    #[serde(default)]
    pub color: Option<String>,

    /// 아이콘 이름 (예: "star", "chart")
    #[serde(default)]
    pub icon: Option<String>,

    /// 생성 시각
    pub created_at: DateTime<Utc>,

    /// 수정 시각
    pub updated_at: DateTime<Utc>,
}

impl Watchlist {
    /// 새 관심종목 그룹 생성.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            sort_order: 0,
            color: None,
            icon: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 설명 설정.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 색상 설정.
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// 아이콘 설정.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 정렬 순서 설정.
    pub fn with_sort_order(mut self, order: i32) -> Self {
        self.sort_order = order;
        self
    }
}

/// 관심종목 아이템.
///
/// 관심종목 그룹 내 개별 종목입니다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WatchlistItem {
    /// 고유 ID
    pub id: Uuid,

    /// 그룹 ID
    pub watchlist_id: Uuid,

    /// 종목 코드
    pub symbol: String,

    /// 시장 (KR, US)
    pub market: String,

    /// 사용자 메모
    #[serde(default)]
    pub memo: Option<String>,

    /// 목표가
    #[serde(default)]
    pub target_price: Option<Decimal>,

    /// 손절가
    #[serde(default)]
    pub stop_price: Option<Decimal>,

    /// 알림 활성화 여부
    #[serde(default)]
    pub alert_enabled: bool,

    /// 표시 순서
    #[serde(default)]
    pub sort_order: i32,

    /// 추가 시점 가격
    #[serde(default)]
    pub added_price: Option<Decimal>,

    /// 생성 시각
    pub created_at: DateTime<Utc>,

    /// 수정 시각
    pub updated_at: DateTime<Utc>,
}

impl WatchlistItem {
    /// 새 관심종목 아이템 생성.
    pub fn new(watchlist_id: Uuid, symbol: impl Into<String>, market: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            watchlist_id,
            symbol: symbol.into(),
            market: market.into(),
            memo: None,
            target_price: None,
            stop_price: None,
            alert_enabled: false,
            sort_order: 0,
            added_price: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 메모 설정.
    pub fn with_memo(mut self, memo: impl Into<String>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// 목표가 설정.
    pub fn with_target_price(mut self, price: Decimal) -> Self {
        self.target_price = Some(price);
        self
    }

    /// 손절가 설정.
    pub fn with_stop_price(mut self, price: Decimal) -> Self {
        self.stop_price = Some(price);
        self
    }

    /// 추가 시점 가격 설정.
    pub fn with_added_price(mut self, price: Decimal) -> Self {
        self.added_price = Some(price);
        self
    }

    /// 알림 활성화 설정.
    pub fn with_alert_enabled(mut self, enabled: bool) -> Self {
        self.alert_enabled = enabled;
        self
    }

    /// 정렬 순서 설정.
    pub fn with_sort_order(mut self, order: i32) -> Self {
        self.sort_order = order;
        self
    }
}

/// 관심종목 그룹과 아이템을 함께 담은 응답.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct WatchlistWithItems {
    /// 그룹 정보
    #[serde(flatten)]
    pub watchlist: Watchlist,

    /// 아이템 목록
    pub items: Vec<WatchlistItem>,

    /// 아이템 수
    pub item_count: usize,
}

impl WatchlistWithItems {
    /// 새 WatchlistWithItems 생성.
    pub fn new(watchlist: Watchlist, items: Vec<WatchlistItem>) -> Self {
        let item_count = items.len();
        Self {
            watchlist,
            items,
            item_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchlist_creation() {
        let watchlist = Watchlist::new("모멘텀 종목")
            .with_description("모멘텀 상위 10개")
            .with_color("#10B981")
            .with_icon("trending-up");

        assert_eq!(watchlist.name, "모멘텀 종목");
        assert_eq!(watchlist.description, Some("모멘텀 상위 10개".to_string()));
        assert_eq!(watchlist.color, Some("#10B981".to_string()));
        assert_eq!(watchlist.icon, Some("trending-up".to_string()));
    }

    #[test]
    fn test_watchlist_item_creation() {
        let watchlist_id = Uuid::new_v4();
        let item = WatchlistItem::new(watchlist_id, "005930", "KR")
            .with_memo("삼성전자")
            .with_target_price(Decimal::from(75000))
            .with_stop_price(Decimal::from(60000))
            .with_alert_enabled(true);

        assert_eq!(item.watchlist_id, watchlist_id);
        assert_eq!(item.symbol, "005930");
        assert_eq!(item.market, "KR");
        assert_eq!(item.memo, Some("삼성전자".to_string()));
        assert!(item.alert_enabled);
    }

    #[test]
    fn test_watchlist_with_items() {
        let watchlist = Watchlist::new("테스트");
        let items = vec![
            WatchlistItem::new(watchlist.id, "005930", "KR"),
            WatchlistItem::new(watchlist.id, "AAPL", "US"),
        ];

        let with_items = WatchlistWithItems::new(watchlist, items);
        assert_eq!(with_items.item_count, 2);
    }
}
