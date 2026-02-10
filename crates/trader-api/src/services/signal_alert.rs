//! SignalMarker 기반 알림 서비스.
//!
//! 백테스트 및 실거래에서 발생한 신호 마커를 필터링하고
//! 텔레그램 등 알림 채널로 전송합니다.

use serde::{Deserialize, Serialize};
use trader_core::SignalMarker;
use trader_notification::{NotificationManager, NotificationResult};

/// 신호 알림 필터 조건.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SignalAlertFilter {
    /// 최소 신호 강도 (0.0 ~ 1.0)
    pub min_strength: Option<f64>,
    /// 전략 ID 필터 (Some이면 특정 전략만, None이면 모든 전략)
    pub strategy_ids: Option<Vec<String>>,
    /// 심볼 필터 (Some이면 특정 심볼만, None이면 모든 심볼)
    pub symbols: Option<Vec<String>>,
    /// 진입 신호만 (true면 Entry만, false면 모든 신호)
    pub entry_only: bool,
}

impl Default for SignalAlertFilter {
    fn default() -> Self {
        Self {
            min_strength: Some(0.7), // 기본: 강도 70% 이상만 알림
            strategy_ids: None,
            symbols: None,
            entry_only: false,
        }
    }
}

impl SignalAlertFilter {
    /// 새 필터 생성 (기본값).
    pub fn new() -> Self {
        Self::default()
    }

    /// 최소 강도 설정.
    pub fn with_min_strength(mut self, min_strength: f64) -> Self {
        self.min_strength = Some(min_strength);
        self
    }

    /// 전략 필터 설정.
    pub fn with_strategies(mut self, strategy_ids: Vec<String>) -> Self {
        self.strategy_ids = Some(strategy_ids);
        self
    }

    /// 심볼 필터 설정.
    pub fn with_symbols(mut self, symbols: Vec<String>) -> Self {
        self.symbols = Some(symbols);
        self
    }

    /// 진입 신호만 허용.
    pub fn entry_only(mut self) -> Self {
        self.entry_only = true;
        self
    }

    /// 신호 마커가 필터 조건을 만족하는지 확인.
    pub fn matches(&self, marker: &SignalMarker) -> bool {
        // 최소 강도 확인
        if let Some(min_strength) = self.min_strength {
            if marker.strength < min_strength {
                return false;
            }
        }

        // 전략 필터 확인
        if let Some(ref strategy_ids) = self.strategy_ids {
            if !strategy_ids.contains(&marker.strategy_id) {
                return false;
            }
        }

        // 심볼 필터 확인
        if let Some(ref symbols) = self.symbols {
            let symbol_str = marker.ticker.to_string();
            if !symbols.iter().any(|s| symbol_str.contains(s)) {
                return false;
            }
        }

        // 진입 신호만 허용
        if self.entry_only && !marker.is_entry() {
            return false;
        }

        true
    }
}

/// 신호 알림 서비스.
///
/// SignalMarker를 받아서 필터링하고 알림을 전송합니다.
pub struct SignalAlertService {
    notification_manager: NotificationManager,
    filter: SignalAlertFilter,
}

impl SignalAlertService {
    /// 새 알림 서비스 생성.
    pub fn new(notification_manager: NotificationManager) -> Self {
        Self {
            notification_manager,
            filter: SignalAlertFilter::default(),
        }
    }

    /// 필터 조건 설정.
    pub fn with_filter(mut self, filter: SignalAlertFilter) -> Self {
        self.filter = filter;
        self
    }

    /// 신호 마커 알림 전송.
    ///
    /// 필터 조건을 만족하는 경우에만 알림을 전송합니다.
    ///
    /// # 인자
    /// - `marker`: 신호 마커
    ///
    /// # 반환
    /// - `Ok(true)`: 알림 전송 성공
    /// - `Ok(false)`: 필터 조건 불만족으로 알림 미전송
    /// - `Err`: 알림 전송 실패
    pub async fn notify_signal(&self, marker: &SignalMarker) -> NotificationResult<bool> {
        // 필터 확인
        if !self.filter.matches(marker) {
            return Ok(false);
        }

        // 지표 정보를 JSON으로 변환
        let indicators =
            serde_json::to_value(&marker.indicators).unwrap_or(serde_json::Value::Null);

        // side를 String으로 변환 (라이프타임 문제 해결)
        let side_str = marker.side.as_ref().map(|s| s.to_string());

        // 알림 전송
        self.notification_manager
            .notify_signal_alert(
                &marker.signal_type.to_string(),
                &marker.ticker.to_string(),
                side_str.as_deref(),
                marker.price,
                marker.strength,
                &marker.reason,
                &marker.strategy_name,
                indicators,
            )
            .await?;

        Ok(true)
    }

    /// 여러 신호 마커 일괄 알림.
    ///
    /// # 인자
    /// - `markers`: 신호 마커 목록
    ///
    /// # 반환
    /// 전송된 알림 수
    pub async fn notify_signals(&self, markers: &[SignalMarker]) -> NotificationResult<usize> {
        let mut sent_count = 0;

        for marker in markers {
            if self.notify_signal(marker).await? {
                sent_count += 1;
            }
        }

        Ok(sent_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use trader_core::SignalType;

    #[test]
    fn test_filter_min_strength() {
        let filter = SignalAlertFilter::new().with_min_strength(0.8);

        let marker_strong = SignalMarker::new(
            "BTC/USDT".to_string(),
            Utc::now(),
            SignalType::Entry,
            dec!(50000),
            "test_strategy",
            "Test Strategy",
        )
        .with_strength(0.9);

        let marker_weak = SignalMarker::new(
            "ETH/USDT".to_string(),
            Utc::now(),
            SignalType::Entry,
            dec!(3000),
            "test_strategy",
            "Test Strategy",
        )
        .with_strength(0.5);

        assert!(filter.matches(&marker_strong));
        assert!(!filter.matches(&marker_weak));
    }

    #[test]
    fn test_filter_entry_only() {
        // min_strength 제거하여 모든 강도 허용
        let filter = SignalAlertFilter::new()
            .entry_only()
            .with_min_strength(0.0);

        let entry_marker = SignalMarker::new(
            "BTC/USDT".to_string(),
            Utc::now(),
            SignalType::Entry,
            dec!(50000),
            "test_strategy",
            "Test Strategy",
        );

        let exit_marker = SignalMarker::new(
            "BTC/USDT".to_string(),
            Utc::now(),
            SignalType::Exit,
            dec!(51000),
            "test_strategy",
            "Test Strategy",
        );

        assert!(filter.matches(&entry_marker));
        assert!(!filter.matches(&exit_marker));
    }

    #[test]
    fn test_filter_strategy_ids() {
        // min_strength 제거하여 모든 강도 허용
        let filter = SignalAlertFilter::new()
            .with_strategies(vec!["rsi_strategy".to_string()])
            .with_min_strength(0.0);

        let matching_marker = SignalMarker::new(
            "BTC/USDT".to_string(),
            Utc::now(),
            SignalType::Entry,
            dec!(50000),
            "rsi_strategy",
            "RSI Strategy",
        );

        let non_matching_marker = SignalMarker::new(
            "ETH/USDT".to_string(),
            Utc::now(),
            SignalType::Entry,
            dec!(3000),
            "macd_strategy",
            "MACD Strategy",
        );

        assert!(filter.matches(&matching_marker));
        assert!(!filter.matches(&non_matching_marker));
    }
}
