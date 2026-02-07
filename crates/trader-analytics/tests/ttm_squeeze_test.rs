//! TTM Squeeze 지표 테스트
//!
//! John Carter의 TTM Squeeze 지표 구현 검증

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trader_analytics::indicators::{
    AtrParams, BollingerBandsParams, IndicatorEngine, KeltnerChannelParams, TtmSqueezeParams,
};

/// 샘플 OHLC 데이터 생성 (40개)
fn sample_ohlc_data() -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
    let high = vec![
        dec!(102),
        dec!(104),
        dec!(103),
        dec!(105),
        dec!(107),
        dec!(106),
        dec!(108),
        dec!(110),
        dec!(109),
        dec!(111),
        dec!(113),
        dec!(112),
        dec!(114),
        dec!(116),
        dec!(115),
        dec!(117),
        dec!(119),
        dec!(118),
        dec!(120),
        dec!(122),
        dec!(121),
        dec!(123),
        dec!(125),
        dec!(124),
        dec!(126),
        dec!(128),
        dec!(127),
        dec!(129),
        dec!(131),
        dec!(130),
        dec!(132),
        dec!(134),
        dec!(133),
        dec!(135),
        dec!(137),
        dec!(136),
        dec!(138),
        dec!(140),
        dec!(139),
        dec!(141),
    ];
    let low = vec![
        dec!(98),
        dec!(100),
        dec!(99),
        dec!(101),
        dec!(103),
        dec!(102),
        dec!(104),
        dec!(106),
        dec!(105),
        dec!(107),
        dec!(109),
        dec!(108),
        dec!(110),
        dec!(112),
        dec!(111),
        dec!(113),
        dec!(115),
        dec!(114),
        dec!(116),
        dec!(118),
        dec!(117),
        dec!(119),
        dec!(121),
        dec!(120),
        dec!(122),
        dec!(124),
        dec!(123),
        dec!(125),
        dec!(127),
        dec!(126),
        dec!(128),
        dec!(130),
        dec!(129),
        dec!(131),
        dec!(133),
        dec!(132),
        dec!(134),
        dec!(136),
        dec!(135),
        dec!(137),
    ];
    let close = vec![
        dec!(100),
        dec!(102),
        dec!(101),
        dec!(103),
        dec!(105),
        dec!(104),
        dec!(106),
        dec!(108),
        dec!(107),
        dec!(109),
        dec!(111),
        dec!(110),
        dec!(112),
        dec!(114),
        dec!(113),
        dec!(115),
        dec!(117),
        dec!(116),
        dec!(118),
        dec!(120),
        dec!(119),
        dec!(121),
        dec!(123),
        dec!(122),
        dec!(124),
        dec!(126),
        dec!(125),
        dec!(127),
        dec!(129),
        dec!(128),
        dec!(130),
        dec!(132),
        dec!(131),
        dec!(133),
        dec!(135),
        dec!(134),
        dec!(136),
        dec!(138),
        dec!(137),
        dec!(139),
    ];

    (high, low, close)
}

/// 횡보 패턴 (squeeze 발생 가능)
fn sample_consolidation_data() -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
    let high = vec![
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
        dec!(105),
        dec!(106),
    ];
    let low = vec![
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
        dec!(95),
        dec!(94),
    ];
    let close = vec![
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
        dec!(100),
        dec!(101),
    ];

    (high, low, close)
}

#[test]
fn test_keltner_channel_basic() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    let kc = engine
        .keltner_channel(&high, &low, &close, KeltnerChannelParams::default())
        .unwrap();

    assert_eq!(kc.len(), close.len());

    // 처음 19개는 None (period=20이므로)
    assert!(kc[18].middle.is_none());

    // 20번째부터 값이 있어야 함
    assert!(kc[19].middle.is_some());
    assert!(kc[19].upper.is_some());
    assert!(kc[19].lower.is_some());

    // 상단 > 중간 > 하단
    if let (Some(u), Some(m), Some(l)) = (kc[25].upper, kc[25].middle, kc[25].lower) {
        assert!(u > m, "상단 채널이 중간선보다 커야 함");
        assert!(m > l, "중간선이 하단 채널보다 커야 함");
    }
}

#[test]
fn test_keltner_channel_width() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    let kc = engine
        .keltner_channel(&high, &low, &close, KeltnerChannelParams::default())
        .unwrap();

    // 채널 폭 검증
    for result in kc.iter().skip(20) {
        if let Some(width) = result.width {
            assert!(width > Decimal::ZERO, "채널 폭은 양수여야 함");
        }
    }
}

#[test]
fn test_ttm_squeeze_basic() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    let squeeze = engine
        .ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default())
        .unwrap();

    assert_eq!(squeeze.len(), close.len());

    // 모든 결과에 대해 기본 검증
    for (i, result) in squeeze.iter().enumerate() {
        // Squeeze 카운트는 0 이상
        assert!(
            result.squeeze_count >= 0,
            "인덱스 {}: squeeze_count는 0 이상이어야 함",
            i
        );

        // Squeeze 상태가 false면 카운트는 0
        if !result.is_squeeze {
            assert_eq!(
                result.squeeze_count, 0,
                "인덱스 {}: squeeze 상태가 아니면 카운트는 0",
                i
            );
        }

        // Released는 이전에 squeeze였다가 지금 해제된 경우만 true
        if result.released {
            assert!(
                !result.is_squeeze,
                "인덱스 {}: released는 현재 squeeze가 아닐 때만 true",
                i
            );
        }
    }
}

#[test]
fn test_ttm_squeeze_consolidation() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_consolidation_data();

    let squeeze = engine
        .ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default())
        .unwrap();

    // 횡보 구간에서는 squeeze가 발생할 가능성이 높음
    let squeeze_count = squeeze.iter().filter(|s| s.is_squeeze).count();

    println!(
        "횡보 구간 squeeze 발생 횟수: {}/{}",
        squeeze_count,
        squeeze.len()
    );

    // 적어도 일부 구간에서는 squeeze가 발생해야 함
    assert!(squeeze_count > 0, "횡보 구간에서 squeeze가 발생해야 함");
}

#[test]
fn test_ttm_squeeze_count_continuity() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_consolidation_data();

    let squeeze = engine
        .ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default())
        .unwrap();

    // Squeeze 카운트가 연속적으로 증가하는지 확인
    let mut prev_count = 0u32;
    let mut in_squeeze = false;

    for result in squeeze.iter().skip(20) {
        if result.is_squeeze {
            if in_squeeze {
                // 연속 squeeze: 카운트가 증가해야 함
                assert_eq!(
                    result.squeeze_count,
                    prev_count + 1,
                    "연속 squeeze 시 카운트가 1씩 증가해야 함"
                );
            }
            in_squeeze = true;
            prev_count = result.squeeze_count;
        } else {
            // Squeeze 해제
            assert_eq!(result.squeeze_count, 0, "squeeze 해제 시 카운트는 0");
            in_squeeze = false;
            prev_count = 0;
        }
    }
}

#[test]
fn test_ttm_squeeze_momentum() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    let squeeze = engine
        .ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default())
        .unwrap();

    // 모멘텀 값 검증
    for result in squeeze.iter().skip(20) {
        if let Some(momentum) = result.momentum {
            // 모멘텀은 종가 - KC 중간선이므로 합리적인 범위여야 함
            assert!(
                momentum.abs() < dec!(50),
                "모멘텀이 너무 큰 값: {}",
                momentum
            );
        }
    }
}

#[test]
fn test_ttm_squeeze_released_detection() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_consolidation_data();

    let squeeze = engine
        .ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default())
        .unwrap();

    // Released 이벤트가 발생한 시점 찾기
    let release_indices: Vec<usize> = squeeze
        .iter()
        .enumerate()
        .filter(|(_, s)| s.released)
        .map(|(i, _)| i)
        .collect();

    println!("Squeeze 해제 시점: {:?}", release_indices);

    // Released가 발생했다면 이전 시점은 squeeze였어야 함
    for &idx in &release_indices {
        if idx > 0 {
            assert!(
                squeeze[idx - 1].is_squeeze,
                "인덱스 {}: released 전 시점은 squeeze였어야 함",
                idx
            );
            assert!(
                !squeeze[idx].is_squeeze,
                "인덱스 {}: released 시점은 squeeze가 아니어야 함",
                idx
            );
        }
    }
}

#[test]
fn test_ttm_squeeze_with_custom_params() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    // 커스텀 파라미터: BB(20, 2.5), KC(20, 2.0)
    let params = TtmSqueezeParams {
        bb_period: 20,
        bb_std_dev: dec!(2.5),
        kc_period: 20,
        kc_atr_multiplier: dec!(2.0),
        atr_period: 14,
    };

    let squeeze = engine.ttm_squeeze(&high, &low, &close, params).unwrap();

    assert_eq!(squeeze.len(), close.len());

    // 파라미터가 다르면 결과도 달라질 수 있음
    let squeeze_count = squeeze.iter().filter(|s| s.is_squeeze).count();
    println!(
        "커스텀 파라미터 squeeze 발생 횟수: {}/{}",
        squeeze_count,
        squeeze.len()
    );
}

#[test]
fn test_bollinger_bands_comparison() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_consolidation_data();

    // BB와 KC 직접 계산하여 비교
    let bb = engine
        .bollinger_bands(&close, BollingerBandsParams::default())
        .unwrap();

    let kc = engine
        .keltner_channel(&high, &low, &close, KeltnerChannelParams::default())
        .unwrap();

    // 횡보 구간에서는 BB가 KC보다 좁아질 가능성이 높음
    let mut squeeze_count = 0;
    for i in 20..bb.len() {
        if let (Some(bb_upper), Some(bb_lower), Some(kc_upper), Some(kc_lower)) =
            (bb[i].upper, bb[i].lower, kc[i].upper, kc[i].lower)
        {
            if bb_upper < kc_upper && bb_lower > kc_lower {
                squeeze_count += 1;
            }
        }
    }

    println!(
        "수동 계산 squeeze 횟수: {}/{}",
        squeeze_count,
        bb.len() - 20
    );
}

#[test]
fn test_insufficient_data_error() {
    let engine = IndicatorEngine::new();

    // 데이터가 부족한 경우 에러 반환
    let high = vec![dec!(100), dec!(101), dec!(102)];
    let low = vec![dec!(95), dec!(96), dec!(97)];
    let close = vec![dec!(98), dec!(99), dec!(100)];

    let result = engine.ttm_squeeze(&high, &low, &close, TtmSqueezeParams::default());

    assert!(result.is_err(), "데이터 부족 시 에러가 발생해야 함");
}

#[test]
fn test_atr_calculation_consistency() {
    let engine = IndicatorEngine::new();
    let (high, low, close) = sample_ohlc_data();

    // ATR이 KC 계산에 올바르게 사용되는지 확인
    let atr = engine
        .atr(&high, &low, &close, AtrParams { period: 20 })
        .unwrap();

    let kc = engine
        .keltner_channel(
            &high,
            &low,
            &close,
            KeltnerChannelParams {
                period: 20,
                atr_multiplier: dec!(1.5),
            },
        )
        .unwrap();

    // KC의 채널 폭이 ATR과 연관되어 있는지 확인
    for i in 20..atr.len() {
        if let (Some(atr_val), Some(kc_upper), Some(kc_middle), Some(kc_lower)) =
            (atr[i], kc[i].upper, kc[i].middle, kc[i].lower)
        {
            let kc_width = kc_upper - kc_lower;
            let expected_width = dec!(2) * dec!(1.5) * atr_val; // 2 * multiplier * ATR

            // 약간의 오차 허용 (이동평균 차이로 인해)
            let diff = (kc_width - expected_width).abs();
            let tolerance = kc_middle * dec!(0.05); // 5% 허용

            assert!(
                diff < tolerance,
                "인덱스 {}: KC 폭이 ATR과 일치하지 않음. KC폭: {}, 예상: {}, 차이: {}",
                i,
                kc_width,
                expected_width,
                diff
            );
        }
    }
}
