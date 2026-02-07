//! 주문 실행 및 포지션 관리.
//!
//! 이 crate는 다음을 제공합니다:
//! - 시그널을 주문으로 변환하는 주문 실행기
//! - 주문 상태 관리 및 추적
//! - PnL 계산을 포함한 포지션 추적
//! - 오류 복구 및 재시도 로직
//!
//! # 예제
//!
//! ```rust,ignore
//! use trader_execution::{OrderManager, PositionTracker, SignalConverter};
//!
//! // 매니저 생성
//! let mut order_manager = OrderManager::new();
//! let mut position_tracker = PositionTracker::new("binance");
//!
//! // 주문 및 포지션 처리
//! ```

pub mod executor;
pub mod live_executor;
pub mod order_manager;
pub mod position_tracker;
pub mod signal_processor;
pub mod simulated_executor;

// 주요 타입 재내보내기
pub use executor::{
    ConversionConfig, ExecutionError, ExecutionResult, OrderExecutor, SignalConverter,
};
pub use order_manager::{OrderEvent, OrderFill, OrderManager, OrderManagerError, OrderStats};
pub use position_tracker::{PositionEvent, PositionTracker, PositionTrackerError};

// Signal 처리 추상화
pub use signal_processor::{
    apply_slippage, build_add_trade, build_entry_trade, build_exit_trade,
    calculate_position_size, calculate_realized_pnl, convert_signal_metadata,
    determine_close_quantity, update_position_average, validate_funds, ProcessorConfig,
    ProcessorPosition, SignalProcessor, SignalProcessorError, TradeResult,
};
pub use live_executor::LiveExecutor;
pub use simulated_executor::SimulatedExecutor;
