//! Strategy trait 정의.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::{
    domain::MultiTimeframeConfig, Kline, MarketData, Order, Position, Signal, StrategyContext,
    Timeframe,
};

/// 트레이딩 전략 구현을 위한 Strategy trait.
///
/// 모든 전략은 전략 엔진에서 로드되기 위해 이 trait를 구현해야 합니다.
#[async_trait]
pub trait Strategy: Send + Sync {
    /// 전략 이름 반환.
    fn name(&self) -> &str;

    /// 전략 버전 반환.
    fn version(&self) -> &str;

    /// 전략 설명 반환.
    fn description(&self) -> &str;

    /// 설정으로 전략 초기화.
    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 새 시장 데이터 수신 시 호출.
    /// 트레이딩 신호가 있으면 반환.
    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>>;

    /// 주문 체결 시 호출.
    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 포지션 업데이트 시 호출.
    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 전략 종료 및 리소스 정리.
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 컨텍스트 주입 (엔진에서 호출).
    ///
    /// 전략이 실시간 거래소 정보와 분석 결과에 접근할 수 있도록 합니다.
    ///
    /// # 기본 구현
    ///
    /// Phase 1의 스코어링 작업에서 각 전략이 명시적으로 구현할 예정입니다.
    fn set_context(&mut self, _context: Arc<RwLock<StrategyContext>>) {
        // TODO: Phase 1에서 각 전략이 명시적으로 구현
    }

    // =========================================================================
    // 다중 타임프레임 지원 (Phase 1.4)
    // =========================================================================

    /// 다중 타임프레임 설정 반환.
    ///
    /// 전략이 여러 타임프레임을 사용하는 경우 이 메서드를 오버라이드하여
    /// 필요한 타임프레임과 캔들 개수를 지정합니다.
    ///
    /// # 기본 구현
    ///
    /// `None`을 반환하여 단일 타임프레임 전략임을 나타냅니다.
    /// 기존 전략은 이 메서드를 구현하지 않아도 정상 동작합니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// fn multi_timeframe_config(&self) -> Option<MultiTimeframeConfig> {
    ///     Some(
    ///         MultiTimeframeConfig::new()
    ///             .with_timeframe(Timeframe::M5, 60)   // Primary: 5분봉 60개
    ///             .with_timeframe(Timeframe::H1, 24)   // 1시간봉 24개
    ///             .with_timeframe(Timeframe::D1, 14)   // 일봉 14개
    ///             .with_primary(Timeframe::M5)
    ///     )
    /// }
    /// ```
    fn multi_timeframe_config(&self) -> Option<MultiTimeframeConfig> {
        None
    }

    /// 다중 타임프레임 데이터로 신호 생성.
    ///
    /// 여러 타임프레임의 데이터를 동시에 분석하여 매매 신호를 생성합니다.
    /// `multi_timeframe_config()`가 `Some`을 반환하는 전략에서 호출됩니다.
    ///
    /// # 인자
    ///
    /// * `primary_data` - Primary 타임프레임의 최신 시장 데이터
    /// * `secondary_data` - Secondary 타임프레임별 캔들 데이터
    ///
    /// # 기본 구현
    ///
    /// `on_market_data()`를 호출하여 기존 동작을 유지합니다.
    ///
    /// # 예시
    ///
    /// ```rust,ignore
    /// async fn on_multi_timeframe_data(
    ///     &mut self,
    ///     primary_data: &MarketData,
    ///     secondary_data: &HashMap<Timeframe, Vec<Kline>>,
    /// ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
    ///     // 일봉 RSI 확인
    ///     let d1_klines = secondary_data.get(&Timeframe::D1).unwrap();
    ///     let d1_rsi = calculate_rsi(d1_klines, 14)?;
    ///
    ///     // 1시간봉 RSI 확인
    ///     let h1_klines = secondary_data.get(&Timeframe::H1).unwrap();
    ///     let h1_rsi = calculate_rsi(h1_klines, 14)?;
    ///
    ///     // 조건 충족 시 신호 생성
    ///     if d1_rsi > dec!(50) && h1_rsi < dec!(30) {
    ///         // 5분봉 반등 확인 후 매수 신호
    ///     }
    ///
    ///     Ok(signals)
    /// }
    /// ```
    async fn on_multi_timeframe_data(
        &mut self,
        primary_data: &MarketData,
        _secondary_data: &HashMap<Timeframe, Vec<Kline>>,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        // 기본 구현: 기존 on_market_data 호출 (하위 호환성)
        self.on_market_data(primary_data).await
    }

    /// 현재 전략 상태를 JSON으로 반환 (디버깅/모니터링용).
    fn get_state(&self) -> Value;

    /// 영속성을 위해 전략 상태 저장.
    fn save_state(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }

    /// 영속성에서 전략 상태 로드.
    fn load_state(&mut self, _data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// 등록을 위한 전략 메타데이터.
#[derive(Debug, Clone)]
pub struct StrategyMetadata {
    /// 전략 이름
    pub name: String,
    /// 전략 버전
    pub version: String,
    /// 전략 설명
    pub description: String,
    /// 필수 설정 키
    pub required_config: Vec<String>,
    /// 지원 티커 (빈 값 = 전체)
    pub supported_tickers: Vec<String>,
}
