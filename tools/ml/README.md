# ML 모델 훈련 도구

Yahoo Finance 데이터로 ML 모델을 훈련하고 ONNX 형식으로 내보내는 도구입니다.

## 개요

```
┌─────────────────────────────────────────────────────────────┐
│                      ML Training Pipeline                     │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐   │
│  │ Yahoo       │───▶│ Feature      │───▶│ Model         │   │
│  │ Finance     │    │ Engineering  │    │ Training      │   │
│  │ (yfinance)  │    │              │    │ (sklearn/XGB) │   │
│  └─────────────┘    └──────────────┘    └───────┬───────┘   │
│                                                   │           │
│                                          ┌───────▼───────┐   │
│                                          │ ONNX Export   │   │
│                                          │ (skl2onnx)    │   │
│                                          └───────┬───────┘   │
│                                                   │           │
│                                          ┌───────▼───────┐   │
│                                          │ Rust Inference│   │
│                                          │ (OnnxPredictor)│   │
│                                          └───────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## 설치

```bash
cd tools/ml
pip install -r requirements.txt
```

## 사용법

### 1. 기본 훈련 (XGBoost)

```bash
python train_model.py --symbol SPY --model xgboost
```

### 2. 다른 모델 사용

```bash
# Random Forest
python train_model.py --symbol SPY --model random_forest

# LightGBM
python train_model.py --symbol SPY --model lightgbm

# Gradient Boosting
python train_model.py --symbol SPY --model gradient_boosting
```

### 3. 여러 심볼로 훈련

```bash
python train_model.py --symbols SPY,QQQ,IWM,DIA --model xgboost
```

### 4. 기간 및 예측 horizon 설정

```bash
# 10년 데이터, 10일 후 예측
python train_model.py --symbol SPY --period 10y --horizon 10
```

### 5. 모델 이름 지정

```bash
python train_model.py --symbol SPY --name my_spy_model
```

## 출력 파일

훈련 후 `models/` 디렉토리에 생성되는 파일:

```
models/
├── xgboost_SPY_5y.onnx           # ONNX 모델
├── xgboost_SPY_5y_metadata.json  # 메타데이터 (피처 목록 등)
└── xgboost_SPY_5y_scaler.joblib  # 스케일러 (전처리용)
```

## Rust에서 사용하기

훈련된 ONNX 모델을 Rust의 `OnnxPredictor`에서 사용하려면:

```bash
# 모델 파일 복사
cp models/xgboost_SPY_5y.onnx ../crates/trader-analytics/models/
```

```rust
use trader_analytics::ml::{OnnxPredictor, PredictorConfig};

let config = PredictorConfig {
    model_path: Some("models/xgboost_SPY_5y.onnx".to_string()),
    ..Default::default()
};

let predictor = OnnxPredictor::new(&config)?;
let prediction = predictor.predict(&feature_vector).await?;
```

## 지원 모델

| 모델 | 설명 | 장점 |
|------|------|------|
| `xgboost` | XGBoost Classifier | 빠르고 정확함, 기본 추천 |
| `lightgbm` | LightGBM Classifier | 대용량 데이터에 효율적 |
| `random_forest` | Random Forest | 해석 가능성 높음 |
| `gradient_boosting` | Gradient Boosting | sklearn 기본 제공 |

## 피처 목록

`FeatureEngineer`에서 추출하는 기술적 지표:

### 가격 기반
- `returns_1d`, `returns_5d`, `returns_10d`, `returns_20d` - 수익률
- `log_returns_1d` - 로그 수익률
- `momentum_10d`, `momentum_20d` - 모멘텀

### 이동평균
- `sma_5`, `sma_10`, `sma_20`, `sma_50`, `sma_200` - 단순이동평균
- `ema_12`, `ema_26` - 지수이동평균
- `price_sma_*_ratio` - 가격/이동평균 비율

### MACD
- `macd`, `macd_signal`, `macd_histogram`

### 오실레이터
- `rsi` - RSI (14일)
- `stoch_k`, `stoch_d` - 스토캐스틱
- `roc` - Rate of Change

### 볼린저 밴드
- `bb_upper`, `bb_middle`, `bb_lower`
- `bb_width`, `bb_pct`

### 변동성
- `atr`, `atr_pct` - ATR
- `volatility_20d` - 20일 변동성

### 추세
- `adx`, `adx_pos`, `adx_neg` - ADX

### 거래량
- `obv` - OBV
- `volume_sma_20`, `volume_ratio`

### 캔들 특성
- `body`, `body_pct` - 캔들 몸통
- `upper_shadow`, `lower_shadow` - 꼬리
- `high_low_ratio` - 고저 대비 위치
- `gap`, `gap_pct` - 갭

## 개발 예정

- [ ] 하이퍼파라미터 자동 튜닝 (Optuna)
- [ ] 딥러닝 모델 (LSTM, Transformer)
- [ ] 멀티 타임프레임 피처
- [ ] 앙상블 모델
- [ ] 자동 재훈련 스케줄러
