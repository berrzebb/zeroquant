//! ExitConfig 섹션별 리스크 관리 회귀 테스트.
//!
//! 검증 범위:
//! 1. 섹션 구조 (Default, enabled 토글, 각 섹션 독립 동작)
//! 2. 6개 프리셋 (for_day_trading, for_mean_reversion 등) 값 검증
//! 3. 하위 호환 헬퍼 (stop_loss(), take_profit(), trailing_stop())
//! 4. Signal 인리치먼트 (SL/TP 가격 계산, metadata, Long/Short, 기존값 보존)

use rust_decimal_macros::dec;
use std::collections::HashMap;
use trader_core::{Side, Signal, SignalType};
use trader_strategy::strategies::common::{
    DailyLossLimitConfig, ExitConfig, ProfitLockConfig, StepLevel, StopLossConfig, StopLossMode,
    TakeProfitConfig, TrailingMode, TrailingStopConfig,
};

// ============================================================================
// 헬퍼 함수
// ============================================================================

/// 테스트용 Entry Signal 생성.
fn create_entry_signal(side: Side) -> Signal {
    Signal {
        id: uuid::Uuid::new_v4(),
        strategy_id: "test_strategy".to_string(),
        ticker: "005930".to_string(),
        side,
        signal_type: SignalType::Entry,
        strength: 0.8,
        suggested_price: Some(dec!(50000)),
        stop_loss: None,
        take_profit: None,
        timestamp: chrono::Utc::now(),
        metadata: HashMap::new(),
        position_id: None,
        group_id: None,
    }
}

/// 테스트용 Exit Signal 생성.
fn create_exit_signal() -> Signal {
    Signal {
        id: uuid::Uuid::new_v4(),
        strategy_id: "test_strategy".to_string(),
        ticker: "005930".to_string(),
        side: Side::Sell,
        signal_type: SignalType::Exit,
        strength: 0.5,
        suggested_price: None,
        stop_loss: None,
        take_profit: None,
        timestamp: chrono::Utc::now(),
        metadata: HashMap::new(),
        position_id: None,
        group_id: None,
    }
}

// ============================================================================
// 1. 섹션 구조 테스트
// ============================================================================

#[test]
fn default_exit_config_has_stop_loss_and_take_profit_enabled() {
    let config = ExitConfig::default();

    // 기본값: SL/TP 활성화
    assert!(config.stop_loss.enabled);
    assert!(config.take_profit.enabled);
    assert_eq!(config.stop_loss.pct, dec!(2.0));
    assert_eq!(config.take_profit.pct, dec!(4.0));
    assert_eq!(config.stop_loss.mode, StopLossMode::Fixed);
}

#[test]
fn default_exit_config_has_trailing_and_extras_disabled() {
    let config = ExitConfig::default();

    // 기본값: 트레일링/수익잠금/일일한도 비활성화
    assert!(!config.trailing_stop.enabled);
    assert!(!config.profit_lock.enabled);
    assert!(!config.daily_loss_limit.enabled);
    assert!(config.exit_on_opposite_signal);
}

#[test]
fn stop_loss_config_default_values() {
    let sl = StopLossConfig::default();
    assert!(sl.enabled);
    assert_eq!(sl.mode, StopLossMode::Fixed);
    assert_eq!(sl.pct, dec!(2.0));
    assert_eq!(sl.atr_multiplier, dec!(2.0));
    assert_eq!(sl.atr_period, 14);
}

#[test]
fn take_profit_config_default_values() {
    let tp = TakeProfitConfig::default();
    assert!(tp.enabled);
    assert_eq!(tp.pct, dec!(4.0));
}

#[test]
fn trailing_stop_config_default_values() {
    let ts = TrailingStopConfig::default();
    assert!(!ts.enabled);
    assert_eq!(ts.mode, TrailingMode::FixedPercentage);
    assert_eq!(ts.trigger_pct, dec!(2.0));
    assert_eq!(ts.stop_pct, dec!(1.0));
    assert_eq!(ts.atr_multiplier, dec!(2.0));
    assert!(ts.step_levels.is_empty());
}

#[test]
fn profit_lock_config_default_values() {
    let pl = ProfitLockConfig::default();
    assert!(!pl.enabled);
    assert_eq!(pl.threshold_pct, dec!(5.0));
    assert_eq!(pl.lock_pct, dec!(80.0));
}

#[test]
fn daily_loss_limit_config_default_values() {
    let dl = DailyLossLimitConfig::default();
    assert!(!dl.enabled);
    assert_eq!(dl.max_loss_pct, dec!(3.0));
}

#[test]
fn sections_are_independent() {
    // 각 섹션을 독립적으로 활성화/비활성화할 수 있는지 검증
    let config = ExitConfig {
        stop_loss: StopLossConfig {
            enabled: false,
            ..Default::default()
        },
        take_profit: TakeProfitConfig {
            enabled: true,
            pct: dec!(10.0),
        },
        trailing_stop: TrailingStopConfig {
            enabled: true,
            mode: TrailingMode::Step,
            step_levels: vec![
                StepLevel {
                    profit_pct: dec!(3.0),
                    trail_pct: dec!(1.0),
                },
                StepLevel {
                    profit_pct: dec!(5.0),
                    trail_pct: dec!(2.0),
                },
            ],
            ..Default::default()
        },
        profit_lock: ProfitLockConfig {
            enabled: true,
            threshold_pct: dec!(10.0),
            lock_pct: dec!(70.0),
        },
        daily_loss_limit: DailyLossLimitConfig {
            enabled: true,
            max_loss_pct: dec!(5.0),
        },
        exit_on_opposite_signal: false,
    };

    assert!(!config.stop_loss.enabled);
    assert!(config.take_profit.enabled);
    assert!(config.trailing_stop.enabled);
    assert_eq!(config.trailing_stop.mode, TrailingMode::Step);
    assert_eq!(config.trailing_stop.step_levels.len(), 2);
    assert!(config.profit_lock.enabled);
    assert_eq!(config.profit_lock.threshold_pct, dec!(10.0));
    assert!(config.daily_loss_limit.enabled);
    assert_eq!(config.daily_loss_limit.max_loss_pct, dec!(5.0));
    assert!(!config.exit_on_opposite_signal);
}

// ============================================================================
// 2. 프리셋 테스트
// ============================================================================

#[test]
fn preset_day_trading() {
    let config = ExitConfig::for_day_trading();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(2.0));
    assert_eq!(config.stop_loss.mode, StopLossMode::Fixed);

    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(4.0));

    assert!(!config.trailing_stop.enabled);
    assert!(!config.profit_lock.enabled);
    assert!(!config.daily_loss_limit.enabled);
    assert!(config.exit_on_opposite_signal);
}

#[test]
fn preset_mean_reversion() {
    let config = ExitConfig::for_mean_reversion();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(3.0));

    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(6.0));

    // 트레일링 설정은 있지만 비활성화
    assert!(!config.trailing_stop.enabled);
    assert_eq!(config.trailing_stop.trigger_pct, dec!(3.0));
    assert_eq!(config.trailing_stop.stop_pct, dec!(1.5));

    assert!(config.exit_on_opposite_signal);
}

#[test]
fn preset_grid_trading() {
    let config = ExitConfig::for_grid_trading();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(15.0));

    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(3.0));

    // 그리드: 반대 신호 청산 비활성화 (독자적 레벨 관리)
    assert!(!config.exit_on_opposite_signal);
}

#[test]
fn preset_rebalancing() {
    let config = ExitConfig::for_rebalancing();

    // 리밸런싱: SL/TP 비활성화
    assert!(!config.stop_loss.enabled);
    assert!(!config.take_profit.enabled);
    assert!(!config.trailing_stop.enabled);
    assert!(!config.exit_on_opposite_signal);
}

#[test]
fn preset_leverage() {
    let config = ExitConfig::for_leverage();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(5.0));

    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(10.0));

    // 레버리지: 트레일링 활성화
    assert!(config.trailing_stop.enabled);
    assert_eq!(config.trailing_stop.mode, TrailingMode::FixedPercentage);
    assert_eq!(config.trailing_stop.trigger_pct, dec!(5.0));
    assert_eq!(config.trailing_stop.stop_pct, dec!(2.0));

    assert!(config.exit_on_opposite_signal);
}

#[test]
fn preset_momentum() {
    let config = ExitConfig::for_momentum();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(5.0));

    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(15.0));

    // 모멘텀: 넓은 트레일링 (8%/3%)
    assert!(config.trailing_stop.enabled);
    assert_eq!(config.trailing_stop.trigger_pct, dec!(8.0));
    assert_eq!(config.trailing_stop.stop_pct, dec!(3.0));

    assert!(config.exit_on_opposite_signal);
}

// ============================================================================
// 3. 하위 호환 헬퍼 테스트
// ============================================================================

#[test]
fn helper_stop_loss_returns_pct_when_enabled() {
    let config = ExitConfig::for_day_trading();
    assert_eq!(config.stop_loss(), Some(dec!(2.0)));
}

#[test]
fn helper_stop_loss_returns_none_when_disabled() {
    let config = ExitConfig::for_rebalancing();
    assert_eq!(config.stop_loss(), None);
}

#[test]
fn helper_take_profit_returns_pct_when_enabled() {
    let config = ExitConfig::for_day_trading();
    assert_eq!(config.take_profit(), Some(dec!(4.0)));
}

#[test]
fn helper_take_profit_returns_none_when_disabled() {
    let config = ExitConfig::for_rebalancing();
    assert_eq!(config.take_profit(), None);
}

#[test]
fn helper_trailing_stop_returns_tuple_when_enabled() {
    let config = ExitConfig::for_leverage();
    assert_eq!(config.trailing_stop(), Some((dec!(5.0), dec!(2.0))));
}

#[test]
fn helper_trailing_stop_returns_none_when_disabled() {
    let config = ExitConfig::for_day_trading();
    assert_eq!(config.trailing_stop(), None);
}

// ============================================================================
// 4. Signal 인리치먼트 테스트
// ============================================================================

#[test]
fn enrich_signal_sets_stop_loss_for_long_entry() {
    let config = ExitConfig::for_day_trading(); // SL 2%
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 매수 SL: 50000 * (1 - 0.02) = 49000
    assert_eq!(signal.stop_loss, Some(dec!(49000)));
}

#[test]
fn enrich_signal_sets_stop_loss_for_short_entry() {
    let config = ExitConfig::for_day_trading(); // SL 2%
    let mut signal = create_entry_signal(Side::Sell);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 매도 SL: 50000 * (1 + 0.02) = 51000
    assert_eq!(signal.stop_loss, Some(dec!(51000)));
}

#[test]
fn enrich_signal_sets_take_profit_for_long_entry() {
    let config = ExitConfig::for_day_trading(); // TP 4%
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 매수 TP: 50000 * (1 + 0.04) = 52000
    assert_eq!(signal.take_profit, Some(dec!(52000)));
}

#[test]
fn enrich_signal_sets_take_profit_for_short_entry() {
    let config = ExitConfig::for_day_trading(); // TP 4%
    let mut signal = create_entry_signal(Side::Sell);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 매도 TP: 50000 * (1 - 0.04) = 48000
    assert_eq!(signal.take_profit, Some(dec!(48000)));
}

#[test]
fn enrich_signal_preserves_existing_stop_loss() {
    let config = ExitConfig::for_day_trading();
    let mut signal = create_entry_signal(Side::Buy);
    signal.stop_loss = Some(dec!(48000)); // 전략이 이미 설정
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 기존값 보존 (ExitConfig 덮어쓰지 않음)
    assert_eq!(signal.stop_loss, Some(dec!(48000)));
}

#[test]
fn enrich_signal_preserves_existing_take_profit() {
    let config = ExitConfig::for_day_trading();
    let mut signal = create_entry_signal(Side::Buy);
    signal.take_profit = Some(dec!(55000)); // 전략이 이미 설정
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 기존값 보존
    assert_eq!(signal.take_profit, Some(dec!(55000)));
}

#[test]
fn enrich_signal_skips_exit_signals() {
    let config = ExitConfig::for_day_trading();
    let mut signal = create_exit_signal();
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // Exit 신호에는 SL/TP 설정하지 않음
    assert_eq!(signal.stop_loss, None);
    assert_eq!(signal.take_profit, None);
    assert!(signal.metadata.is_empty());
}

#[test]
fn enrich_signal_applies_to_add_to_position() {
    let config = ExitConfig::for_day_trading(); // SL 2%, TP 4%
    let mut signal = create_entry_signal(Side::Buy);
    signal.signal_type = SignalType::AddToPosition;
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // AddToPosition에도 SL/TP 적용
    assert_eq!(signal.stop_loss, Some(dec!(49000)));
    assert_eq!(signal.take_profit, Some(dec!(52000)));
}

#[test]
fn enrich_signal_disabled_sections_no_effect() {
    let config = ExitConfig::for_rebalancing(); // 모두 비활성화
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 모든 섹션 비활성화 → SL/TP 없음
    assert_eq!(signal.stop_loss, None);
    assert_eq!(signal.take_profit, None);
    // exit_on_opposite_signal도 false → metadata 비어있음
    assert!(signal.metadata.is_empty());
}

#[test]
fn enrich_signal_trailing_stop_in_metadata() {
    let config = ExitConfig::for_leverage(); // 트레일링 활성화
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(50000);

    config.enrich_signal(&mut signal, entry_price);

    // 트레일링 설정이 metadata에 저장됨
    assert!(signal.metadata.contains_key("trailing_stop"));
    let ts_meta = signal.metadata.get("trailing_stop").unwrap();
    assert_eq!(ts_meta["enabled"], true);
    assert_eq!(ts_meta["mode"], "FixedPercentage");
}

#[test]
fn enrich_signal_profit_lock_in_metadata() {
    let config = ExitConfig {
        profit_lock: ProfitLockConfig {
            enabled: true,
            threshold_pct: dec!(10.0),
            lock_pct: dec!(70.0),
        },
        ..Default::default()
    };

    let mut signal = create_entry_signal(Side::Buy);
    config.enrich_signal(&mut signal, dec!(50000));

    assert!(signal.metadata.contains_key("profit_lock"));
    let pl_meta = signal.metadata.get("profit_lock").unwrap();
    assert_eq!(pl_meta["enabled"], true);
}

#[test]
fn enrich_signal_daily_loss_limit_in_metadata() {
    let config = ExitConfig {
        daily_loss_limit: DailyLossLimitConfig {
            enabled: true,
            max_loss_pct: dec!(5.0),
        },
        ..Default::default()
    };

    let mut signal = create_entry_signal(Side::Buy);
    config.enrich_signal(&mut signal, dec!(50000));

    assert!(signal.metadata.contains_key("daily_loss_limit"));
    let dl_meta = signal.metadata.get("daily_loss_limit").unwrap();
    assert_eq!(dl_meta["enabled"], true);
}

#[test]
fn enrich_signal_exit_on_opposite_in_metadata() {
    let config = ExitConfig::for_day_trading(); // exit_on_opposite = true
    let mut signal = create_entry_signal(Side::Buy);

    config.enrich_signal(&mut signal, dec!(50000));

    assert!(signal.metadata.contains_key("exit_on_opposite"));
    assert_eq!(
        signal.metadata.get("exit_on_opposite").unwrap(),
        &serde_json::Value::Bool(true)
    );
}

#[test]
fn enrich_signal_atr_mode_stores_config_in_metadata() {
    let config = ExitConfig {
        stop_loss: StopLossConfig {
            enabled: true,
            mode: StopLossMode::AtrBased,
            pct: dec!(2.0),
            atr_multiplier: dec!(2.5),
            atr_period: 20,
        },
        ..Default::default()
    };

    let mut signal = create_entry_signal(Side::Buy);
    config.enrich_signal(&mut signal, dec!(50000));

    // ATR 모드: 가격 설정하지 않고 metadata에 설정 저장
    assert_eq!(signal.stop_loss, None);
    assert!(signal.metadata.contains_key("atr_stop_loss"));
    let atr_meta = signal.metadata.get("atr_stop_loss").unwrap();
    assert_eq!(atr_meta["mode"], "AtrBased");
    assert_eq!(atr_meta["atr_period"], 20);
}

#[test]
fn enrich_signals_batch_applies_to_all() {
    let config = ExitConfig::for_day_trading();
    let mut signals = vec![
        create_entry_signal(Side::Buy),
        create_entry_signal(Side::Sell),
        create_exit_signal(), // Exit은 스킵
    ];
    let entry_price = dec!(50000);

    config.enrich_signals(&mut signals, entry_price);

    // 첫 번째 (Long Entry)
    assert_eq!(signals[0].stop_loss, Some(dec!(49000)));
    assert_eq!(signals[0].take_profit, Some(dec!(52000)));

    // 두 번째 (Short Entry)
    assert_eq!(signals[1].stop_loss, Some(dec!(51000)));
    assert_eq!(signals[1].take_profit, Some(dec!(48000)));

    // 세 번째 (Exit) → 변경 없음
    assert_eq!(signals[2].stop_loss, None);
    assert_eq!(signals[2].take_profit, None);
}

// ============================================================================
// 5. Serde 직렬화/역직렬화 테스트
// ============================================================================

#[test]
fn exit_config_serializes_to_json() {
    let config = ExitConfig::for_day_trading();
    let json = serde_json::to_value(&config).unwrap();

    assert_eq!(json["stop_loss"]["enabled"], true);
    assert_eq!(json["stop_loss"]["mode"], "Fixed");
    assert_eq!(json["take_profit"]["enabled"], true);
    assert_eq!(json["trailing_stop"]["enabled"], false);
    assert_eq!(json["exit_on_opposite_signal"], true);
}

#[test]
fn exit_config_deserializes_from_json() {
    let json_str = r#"{
        "stop_loss": {
            "enabled": true,
            "mode": "AtrBased",
            "pct": "3.0",
            "atr_multiplier": "2.5",
            "atr_period": 20
        },
        "take_profit": {
            "enabled": true,
            "pct": "8.0"
        },
        "trailing_stop": {
            "enabled": true,
            "mode": "Step",
            "trigger_pct": "5.0",
            "stop_pct": "2.0",
            "atr_multiplier": "2.0",
            "step_levels": [
                { "profit_pct": "3.0", "trail_pct": "1.0" },
                { "profit_pct": "5.0", "trail_pct": "2.0" }
            ]
        },
        "profit_lock": {
            "enabled": true,
            "threshold_pct": "10.0",
            "lock_pct": "70.0"
        },
        "daily_loss_limit": {
            "enabled": true,
            "max_loss_pct": "5.0"
        },
        "exit_on_opposite_signal": false
    }"#;

    let config: ExitConfig = serde_json::from_str(json_str).unwrap();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.mode, StopLossMode::AtrBased);
    assert_eq!(config.stop_loss.atr_multiplier, dec!(2.5));
    assert_eq!(config.stop_loss.atr_period, 20);

    assert!(config.trailing_stop.enabled);
    assert_eq!(config.trailing_stop.mode, TrailingMode::Step);
    assert_eq!(config.trailing_stop.step_levels.len(), 2);

    assert!(config.profit_lock.enabled);
    assert_eq!(config.profit_lock.threshold_pct, dec!(10.0));

    assert!(config.daily_loss_limit.enabled);
    assert!(!config.exit_on_opposite_signal);
}

#[test]
fn exit_config_deserializes_with_defaults() {
    // 빈 JSON에서 기본값 적용 확인
    let json_str = r#"{}"#;
    let config: ExitConfig = serde_json::from_str(json_str).unwrap();

    assert!(config.stop_loss.enabled);
    assert_eq!(config.stop_loss.pct, dec!(2.0));
    assert!(config.take_profit.enabled);
    assert_eq!(config.take_profit.pct, dec!(4.0));
    assert!(!config.trailing_stop.enabled);
    assert!(config.exit_on_opposite_signal);
}

// ============================================================================
// 6. 경계값 테스트
// ============================================================================

#[test]
fn enrich_signal_with_zero_entry_price() {
    let config = ExitConfig::for_day_trading();
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(0);

    config.enrich_signal(&mut signal, entry_price);

    // 0 가격에서도 패닉 없이 동작
    assert_eq!(signal.stop_loss, Some(dec!(0)));
    assert_eq!(signal.take_profit, Some(dec!(0)));
}

#[test]
fn enrich_signal_with_very_large_price() {
    let config = ExitConfig::for_day_trading();
    let mut signal = create_entry_signal(Side::Buy);
    let entry_price = dec!(999999999);

    config.enrich_signal(&mut signal, entry_price);

    // 큰 가격에서도 패닉 없이 동작
    assert!(signal.stop_loss.is_some());
    assert!(signal.take_profit.is_some());
    // SL < entry < TP (Long)
    assert!(signal.stop_loss.unwrap() < entry_price);
    assert!(signal.take_profit.unwrap() > entry_price);
}

#[test]
fn all_trailing_modes_serialize_correctly() {
    let modes = vec![
        TrailingMode::FixedPercentage,
        TrailingMode::AtrBased,
        TrailingMode::Step,
        TrailingMode::ParabolicSar,
    ];

    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: TrailingMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }
}

#[test]
fn all_stop_loss_modes_serialize_correctly() {
    let modes = vec![StopLossMode::Fixed, StopLossMode::AtrBased];

    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: StopLossMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }
}
