//! Momentum Surge 전략 (급등 모멘텀 포착)
//!
//! 코스피/코스닥 레버리지와 인버스 ETF를 조합한 복합 양방향 전략.
//! OBV(On-Balance Volume)와 이동평균선을 활용한 추세 판단.
//!
//! # 전략 로직
//! - **대상 ETF**: 코스피 레버리지, 코스닥 레버리지, 코스피 인버스, 코스닥 인버스
//! - **진입 조건**:
//!   - 레버리지: OBV 상승 + MA 정배열 + RSI 조건
//!   - 인버스: OBV 하락 + MA 역배열 + RSI 조건
//! - **청산**: 반대 신호 발생 시 또는 손절/익절
//! - **포지션 분산**: 최대 4개 ETF 동시 보유
//!
//! # 대상 ETF
//! - **코스피 레버리지**: 122630 (KODEX 레버리지)
//! - **코스닥 레버리지**: 233740 (KODEX 코스닥150레버리지)
//! - **코스피 인버스**: 252670 (KODEX 200선물인버스2X)
//! - **코스닥 인버스**: 251340 (KODEX 코스닥150선물인버스)
//!
//! # 권장 타임프레임
//! - 일봉 (1D)

use crate::strategies::common::{deserialize_tickers, ExitConfig};
use crate::Strategy;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use trader_core::domain::{RouteState, StrategyContext};
use trader_core::{Kline, MarketData, MarketDataType, Order, Position, Side, Signal, Timeframe};
use trader_strategy_macro::StrategyConfig;

/// Momentum Surge 전략 설정.
#[derive(Debug, Clone, Deserialize, Serialize, StrategyConfig)]
#[strategy(
    id = "momentum_surge",
    name = "Momentum Surge",
    description = "코스피/코스닥 레버리지/인버스 ETF 조합 양방향 전략",
    category = "Daily"
)]
pub struct MomentumSurgeConfig {
    /// 거래 대상 ETF 리스트
    #[serde(default = "default_etf_list", deserialize_with = "deserialize_tickers")]
    #[schema(label = "거래 대상 ETF", skip)]
    pub tickers: Vec<String>,

    /// 코스피 레버리지 티커
    #[serde(default = "default_kospi_leverage")]
    #[schema(label = "코스피 레버리지 티커", field_type = "symbol", default = "122630", section = "asset")]
    pub kospi_leverage: String,

    /// 코스닥 레버리지 티커
    #[serde(default = "default_kosdaq_leverage")]
    #[schema(label = "코스닥 레버리지 티커", field_type = "symbol", default = "233740", section = "asset")]
    pub kosdaq_leverage: String,

    /// 코스피 인버스 티커
    #[serde(default = "default_kospi_inverse")]
    #[schema(label = "코스피 인버스 티커", field_type = "symbol", default = "252670", section = "asset")]
    pub kospi_inverse: String,

    /// 코스닥 인버스 티커
    #[serde(default = "default_kosdaq_inverse")]
    #[schema(label = "코스닥 인버스 티커", field_type = "symbol", default = "251340", section = "asset")]
    pub kosdaq_inverse: String,

    /// 최대 동시 투자 종목 수 (기본값: 2)
    #[serde(default = "default_max_positions")]
    #[schema(label = "최대 동시 포지션 수", min = 1, max = 4, default = 2, section = "filter")]
    pub max_positions: usize,

    /// 종목당 투자 비율 (기본값: 0.5)
    #[serde(default = "default_position_ratio")]
    #[schema(label = "종목당 투자 비율", min = 0.1, max = 1.0, default = 0.5, section = "sizing")]
    pub position_ratio: f64,

    /// OBV 기간 (기본값: 10)
    #[serde(default = "default_obv_period")]
    #[schema(label = "OBV 기간", min = 5, max = 30, default = 10, section = "indicator")]
    pub obv_period: usize,

    /// MA 단기 (기본값: 5)
    #[serde(default = "default_ma_short")]
    #[schema(label = "MA 단기", min = 3, max = 20, default = 5, section = "indicator")]
    pub ma_short: usize,

    /// MA 중기 (기본값: 20)
    #[serde(default = "default_ma_medium")]
    #[schema(label = "MA 중기", min = 10, max = 60, default = 20, section = "indicator")]
    pub ma_medium: usize,

    /// MA 장기 (기본값: 60)
    #[serde(default = "default_ma_long")]
    #[schema(label = "MA 장기", min = 30, max = 200, default = 60, section = "indicator")]
    pub ma_long: usize,

    /// RSI 기간 (기본값: 14)
    #[serde(default = "default_rsi_period")]
    #[schema(label = "RSI 기간", min = 7, max = 30, default = 14, section = "indicator")]
    pub rsi_period: usize,

    /// 손절 비율 (기본값: 3%)
    #[serde(default = "default_stop_loss")]
    #[schema(label = "손절 비율 (%)", min = 0.5, max = 10.0, default = 3.0, section = "sizing")]
    pub stop_loss_pct: f64,

    /// 익절 비율 (기본값: 10%)
    #[serde(default = "default_take_profit")]
    #[schema(label = "익절 비율 (%)", min = 1, max = 30, default = 10.0, section = "sizing")]
    pub take_profit_pct: f64,

    /// 최소 글로벌 스코어 (기본값: 60)
    #[serde(default = "default_min_global_score")]
    #[schema(label = "최소 GlobalScore", min = 0, max = 100, default = 60, section = "filter")]
    pub min_global_score: Decimal,

    /// 청산 설정 (손절/익절/트레일링 스탑).
    #[serde(default)]
    #[fragment("risk.exit_config")]
    pub exit_config: ExitConfig,
}

fn default_etf_list() -> Vec<String> {
    vec![
        "122630".to_string(), // 코스피 레버리지
        "233740".to_string(), // 코스닥 레버리지
        "252670".to_string(), // 코스피 인버스
        "251340".to_string(), // 코스닥 인버스
    ]
}

fn default_kospi_leverage() -> String {
    "122630".to_string()
}
fn default_kosdaq_leverage() -> String {
    "233740".to_string()
}
fn default_kospi_inverse() -> String {
    "252670".to_string()
}
fn default_kosdaq_inverse() -> String {
    "251340".to_string()
}
fn default_max_positions() -> usize {
    2
}
fn default_position_ratio() -> f64 {
    0.5
}
fn default_obv_period() -> usize {
    10
}
fn default_ma_short() -> usize {
    5
}
fn default_ma_medium() -> usize {
    20
}
fn default_ma_long() -> usize {
    60
}
fn default_rsi_period() -> usize {
    14
}
fn default_stop_loss() -> f64 {
    3.0
}
fn default_take_profit() -> f64 {
    10.0
}

fn default_min_global_score() -> Decimal {
    dec!(60)
}

impl Default for MomentumSurgeConfig {
    fn default() -> Self {
        Self {
            tickers: default_etf_list(),
            kospi_leverage: "122630".to_string(),
            kosdaq_leverage: "233740".to_string(),
            kospi_inverse: "252670".to_string(),
            kosdaq_inverse: "251340".to_string(),
            max_positions: 2,
            position_ratio: 0.5,
            obv_period: 10,
            ma_short: 5,
            ma_medium: 20,
            ma_long: 60,
            rsi_period: 14,
            stop_loss_pct: 3.0,
            take_profit_pct: 10.0,
            min_global_score: default_min_global_score(),
            exit_config: ExitConfig::for_day_trading(),
        }
    }
}

/// ETF 타입.
#[derive(Debug, Clone, PartialEq)]
enum EtfType {
    KospiLeverage,
    KosdaqLeverage,
    KospiInverse,
    KosdaqInverse,
}

/// ETF 데이터 (가격 히스토리는 StrategyContext에서 가져옴).
#[derive(Debug, Clone)]
struct EtfData {
    ticker: String,
    etf_type: EtfType,
    current_price: Decimal,
    holdings: Decimal,
    entry_price: Decimal,
}

impl EtfData {
    fn new(ticker: String, etf_type: EtfType) -> Self {
        Self {
            ticker,
            etf_type,
            current_price: Decimal::ZERO,
            holdings: Decimal::ZERO,
            entry_price: Decimal::ZERO,
        }
    }

    fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
    }
}

/// Momentum Surge 전략.
pub struct MomentumSurgeStrategy {
    config: Option<MomentumSurgeConfig>,
    tickers: Vec<String>,

    /// ETF별 데이터
    etf_data: HashMap<String, EtfData>,

    /// 현재 날짜
    current_date: Option<chrono::NaiveDate>,

    /// 초기화 완료
    started: bool,

    /// 통계
    trades_count: u32,
    wins: u32,
    total_pnl: Decimal,

    initialized: bool,

    /// 전략 컨텍스트
    context: Option<Arc<RwLock<StrategyContext>>>,
}

impl MomentumSurgeStrategy {
    pub fn new() -> Self {
        Self {
            config: None,
            tickers: Vec::new(),
            etf_data: HashMap::new(),
            current_date: None,
            started: false,
            trades_count: 0,
            wins: 0,
            total_pnl: Decimal::ZERO,
            initialized: false,
            context: None,
        }
    }

    // ========================================================================
    // StrategyContext 연동 헬퍼
    // ========================================================================

    /// StrategyContext에서 klines 가져오기
    fn get_etf_klines(&self, ticker: &str) -> Vec<Kline> {
        let ctx = match self.context.as_ref() {
            Some(c) => c,
            None => return vec![],
        };
        let ctx_lock = match ctx.try_read() {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        // ticker는 "122630/KRW" 형식일 수 있음, StrategyContext에는 "122630"으로 저장됨
        let ticker_base = ticker.split('/').next().unwrap_or(ticker);
        ctx_lock.get_klines(ticker_base, Timeframe::D1).to_vec()
    }

    /// klines에서 MA 계산
    fn calculate_ma(klines: &[Kline], period: usize) -> Option<Decimal> {
        if klines.len() < period {
            return None;
        }
        let sum: Decimal = klines.iter().rev().take(period).map(|k| k.close).sum();
        Some(sum / Decimal::from(period))
    }

    /// klines에서 RSI 계산
    fn calculate_rsi(klines: &[Kline], period: usize) -> Option<Decimal> {
        if klines.len() < period + 1 {
            return None;
        }

        let closes: Vec<_> = klines.iter().rev().take(period + 1).map(|k| k.close).collect();
        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..closes.len() {
            let change = closes[i] - closes[i - 1];
            if change > Decimal::ZERO {
                gains.push(change);
                losses.push(Decimal::ZERO);
            } else {
                gains.push(Decimal::ZERO);
                losses.push(change.abs());
            }
        }

        if gains.len() < period {
            return None;
        }

        let avg_gain: Decimal = gains.iter().take(period).sum::<Decimal>() / Decimal::from(period);
        let avg_loss: Decimal = losses.iter().take(period).sum::<Decimal>() / Decimal::from(period);

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    /// klines에서 OBV 계산
    fn calculate_obv(klines: &[Kline]) -> Vec<Decimal> {
        let mut obv_values = Vec::new();
        let mut current_obv = Decimal::ZERO;

        for (i, kline) in klines.iter().enumerate() {
            if i == 0 {
                current_obv = kline.volume;
            } else {
                let prev = &klines[i - 1];
                if kline.close > prev.close {
                    current_obv += kline.volume;
                } else if kline.close < prev.close {
                    current_obv -= kline.volume;
                }
            }
            obv_values.push(current_obv);
        }

        obv_values
    }

    /// OBV 추세 확인 (상승세인지)
    fn obv_trend(klines: &[Kline], period: usize) -> Option<bool> {
        let obv = Self::calculate_obv(klines);
        if obv.len() < period {
            return None;
        }

        let current = *obv.last()?;
        let past = *obv.get(obv.len().saturating_sub(period))?;

        Some(current > past)
    }

    /// MA 정렬 확인 (상승 추세: short > medium > long)
    fn is_ma_aligned_bullish(klines: &[Kline], short: usize, medium: usize, long: usize) -> bool {
        let ma_s = Self::calculate_ma(klines, short);
        let ma_m = Self::calculate_ma(klines, medium);
        let ma_l = Self::calculate_ma(klines, long);

        match (ma_s, ma_m, ma_l) {
            (Some(s), Some(m), Some(l)) => s > m && m > l,
            _ => false,
        }
    }

    /// MA 정렬 확인 (하락 추세: short < medium < long)
    fn is_ma_aligned_bearish(klines: &[Kline], short: usize, medium: usize, long: usize) -> bool {
        let ma_s = Self::calculate_ma(klines, short);
        let ma_m = Self::calculate_ma(klines, medium);
        let ma_l = Self::calculate_ma(klines, long);

        match (ma_s, ma_m, ma_l) {
            (Some(s), Some(m), Some(l)) => s < m && m < l,
            _ => false,
        }
    }

    /// RouteState 기반 진입 조건 체크.
    /// 고정된 레버리지/인버스 ETF 리스트이므로 GlobalScore 스크리닝 불필요.
    fn can_enter(&self) -> bool {
        let context = match &self.context {
            Some(ctx) => ctx,
            None => return true, // context 없으면 기본 허용
        };

        let _config = match &self.config {
            Some(cfg) => cfg,
            None => return true,
        };

        let ctx = match context.try_read() {
            Ok(ctx) => ctx,
            Err(_) => return true,
        };

        // RouteState 체크 (첫 번째 티커 기준) - Overheat 시만 진입 제한
        if let Some(ticker) = self.tickers.first() {
            if let Some(route) = ctx.get_route_state(ticker) {
                match route {
                    RouteState::Overheat => {
                        debug!("[MomentumSurge] 시장 과열 - 진입 제한");
                        return false;
                    }
                    _ => {}
                }
            }
        }

        true
    }

    /// 새로운 날인지 확인.
    fn is_new_day(&self, current_time: DateTime<Utc>) -> bool {
        match self.current_date {
            Some(date) => current_time.date_naive() != date,
            None => true,
        }
    }

    /// 현재 포지션 수 계산.
    fn current_position_count(&self) -> usize {
        self.etf_data
            .values()
            .filter(|d| d.holdings > Decimal::ZERO)
            .count()
    }

    /// 레버리지 매수 조건 확인.
    fn should_buy_leverage(&self, data: &EtfData) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 이미 보유 중이면 매수 안 함
        if data.holdings > Decimal::ZERO {
            return false;
        }

        // 최대 포지션 수 확인
        if self.current_position_count() >= config.max_positions {
            return false;
        }

        // StrategyContext에서 klines 가져오기
        let klines = self.get_etf_klines(&data.ticker);
        if klines.len() < 60 {
            return false;
        }

        // OBV 상승 추세
        let obv_up = match Self::obv_trend(&klines, config.obv_period) {
            Some(v) => v,
            None => return false,
        };

        if !obv_up {
            return false;
        }

        // MA 정배열
        let ma_bullish = Self::is_ma_aligned_bullish(&klines, config.ma_short, config.ma_medium, config.ma_long);

        if !ma_bullish {
            return false;
        }

        // RSI 조건 (과매수 아닐 때)
        let rsi = match Self::calculate_rsi(&klines, config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };

        let rsi_ok = rsi < 70.0 && rsi > 30.0;

        debug!(
            ticker = %data.ticker,
            obv_up = obv_up,
            ma_bullish = ma_bullish,
            rsi = %format!("{:.1}", rsi),
            "레버리지 매수 조건 체크"
        );

        rsi_ok
    }

    /// 인버스 매수 조건 확인.
    fn should_buy_inverse(&self, data: &EtfData) -> bool {
        let config = match self.config.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 이미 보유 중이면 매수 안 함
        if data.holdings > Decimal::ZERO {
            return false;
        }

        // 최대 포지션 수 확인
        if self.current_position_count() >= config.max_positions {
            return false;
        }

        // 해당 인버스의 페어 레버리지 데이터 확인
        let pair_ticker_base = match data.etf_type {
            EtfType::KospiInverse => &config.kospi_leverage,
            EtfType::KosdaqInverse => &config.kosdaq_leverage,
            _ => return false,
        };

        // etf_data는 "122630/KRW" 형식, config는 "122630" 형식이므로 변환 필요
        let pair_ticker_full = self
            .tickers
            .iter()
            .find(|t| t.starts_with(&format!("{}/", pair_ticker_base)))
            .cloned();

        let pair_ticker = match pair_ticker_full {
            Some(t) => t,
            None => return false,
        };

        // StrategyContext에서 페어의 klines 가져오기
        let pair_klines = self.get_etf_klines(&pair_ticker);
        if pair_klines.len() < 60 {
            return false;
        }

        // 페어 레버리지의 OBV 하락 추세
        let obv_down = match Self::obv_trend(&pair_klines, config.obv_period) {
            Some(v) => !v, // 반대
            None => return false,
        };

        if !obv_down {
            return false;
        }

        // 페어 레버리지의 MA 역배열
        let ma_bearish = Self::is_ma_aligned_bearish(&pair_klines, config.ma_short, config.ma_medium, config.ma_long);

        if !ma_bearish {
            return false;
        }

        // RSI 조건 (과매도 회복 구간)
        let rsi = match Self::calculate_rsi(&pair_klines, config.rsi_period) {
            Some(v) => v.to_f64().unwrap_or(50.0),
            None => return false,
        };

        let rsi_ok = rsi < 40.0; // 레버리지가 하락 중

        debug!(
            ticker = %data.ticker,
            pair = %pair_ticker_base,
            obv_down = obv_down,
            ma_bearish = ma_bearish,
            rsi = %format!("{:.1}", rsi),
            "인버스 매수 조건 체크"
        );

        rsi_ok
    }

    /// 매도 조건 확인.
    fn should_sell(&self, data: &EtfData) -> Option<String> {
        let config = self.config.as_ref()?;

        // 보유 중이 아니면 매도 불가
        if data.holdings <= Decimal::ZERO {
            return None;
        }

        // 손절 체크
        if data.entry_price > Decimal::ZERO {
            let pnl_pct = ((data.current_price - data.entry_price) / data.entry_price * dec!(100))
                .to_f64()
                .unwrap_or(0.0);

            if pnl_pct <= -config.stop_loss_pct {
                return Some("stop_loss".to_string());
            }

            if pnl_pct >= config.take_profit_pct {
                return Some("take_profit".to_string());
            }
        }

        // StrategyContext에서 klines 가져오기
        let klines = self.get_etf_klines(&data.ticker);

        // 레버리지는 MA 역배열 시 매도
        if data.etf_type == EtfType::KospiLeverage || data.etf_type == EtfType::KosdaqLeverage {
            if Self::is_ma_aligned_bearish(&klines, config.ma_short, config.ma_medium, config.ma_long) {
                return Some("ma_bearish".to_string());
            }

            // OBV 하락 전환
            if let Some(false) = Self::obv_trend(&klines, config.obv_period) {
                return Some("obv_down".to_string());
            }
        }

        // 인버스는 MA 정배열 시 매도
        if data.etf_type == EtfType::KospiInverse || data.etf_type == EtfType::KosdaqInverse {
            let pair_ticker_base = match data.etf_type {
                EtfType::KospiInverse => &config.kospi_leverage,
                EtfType::KosdaqInverse => &config.kosdaq_leverage,
                _ => return None,
            };

            // etf_data는 "122630/KRW" 형식, config는 "122630" 형식이므로 변환 필요
            let pair_ticker_full = self
                .tickers
                .iter()
                .find(|t| t.starts_with(&format!("{}/", pair_ticker_base)));

            if let Some(pair_ticker) = pair_ticker_full {
                let pair_klines = self.get_etf_klines(pair_ticker);
                if Self::is_ma_aligned_bullish(&pair_klines, config.ma_short, config.ma_medium, config.ma_long) {
                    return Some("ma_bullish".to_string());
                }
            }
        }

        None
    }

    /// 신호 생성.
    fn generate_signals(&mut self) -> Vec<Signal> {
        let config = match self.config.as_ref() {
            Some(c) => c.clone(),
            None => return Vec::new(),
        };

        let mut signals = Vec::new();

        // 각 ETF에 대해 신호 확인
        let tickers: Vec<String> = self.etf_data.keys().cloned().collect();

        for ticker in tickers {
            let data = match self.etf_data.get(&ticker) {
                Some(d) => d.clone(),
                None => continue,
            };

            // etf_data 키와 self.tickers는 동일한 형식 ("122630/KRW")
            // 유효한 티커인지 확인
            if !self.tickers.contains(&ticker) {
                continue;
            }

            // 매도 신호 확인
            if let Some(reason) = self.should_sell(&data) {
                signals.push(
                    Signal::exit("momentum_surge", ticker.clone(), Side::Sell)
                        .with_strength(1.0)
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("exit_reason", json!(reason))
                        .with_metadata("etf_type", json!(format!("{:?}", data.etf_type))),
                );
                info!(
                    ticker = %ticker,
                    reason = %reason,
                    price = %data.current_price,
                    "매도 신호"
                );
                continue;
            }

            // 매수 신호 확인
            let should_buy = match data.etf_type {
                EtfType::KospiLeverage | EtfType::KosdaqLeverage => self.should_buy_leverage(&data),
                EtfType::KospiInverse | EtfType::KosdaqInverse => self.should_buy_inverse(&data),
            };

            if should_buy {
                // can_enter() 체크 - 진입 조건 미충족 시 스킵
                if !self.can_enter() {
                    debug!("[MomentumSurge] can_enter() 실패 - 매수 신호 스킵");
                    continue;
                }

                info!(
                    ticker = %ticker,
                    etf_type = ?data.etf_type,
                    price = %data.current_price,
                    "매수 신호"
                );
                signals.push(
                    Signal::entry("momentum_surge", ticker, Side::Buy)
                        .with_strength(config.position_ratio)
                        .with_prices(Some(data.current_price), None, None)
                        .with_metadata("etf_type", json!(format!("{:?}", data.etf_type)))
                        .with_metadata("action", json!("buy")),
                );
            }
        }

        signals
    }
}

impl Default for MomentumSurgeStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Strategy for MomentumSurgeStrategy {
    fn name(&self) -> &str {
        "Momentum Surge"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Momentum Surge 전략. 코스피/코스닥 레버리지와 인버스 ETF를 조합한 \
         양방향 전략. OBV와 MA 조합으로 추세 판단."
    }

    async fn initialize(
        &mut self,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ms_config: MomentumSurgeConfig = serde_json::from_value(config)?;

        info!(
            tickers = ?ms_config.tickers,
            max_positions = ms_config.max_positions,
            position_ratio = %format!("{:.0}%", ms_config.position_ratio * 100.0),
            "Momentum Surge 전략 초기화"
        );

        // ETF 데이터 초기화
        for ticker_base in &ms_config.tickers {
            let ticker = format!("{}/KRW", ticker_base);
            self.tickers.push(ticker.clone());

            let etf_type = if ticker_base == &ms_config.kospi_leverage {
                EtfType::KospiLeverage
            } else if ticker_base == &ms_config.kosdaq_leverage {
                EtfType::KosdaqLeverage
            } else if ticker_base == &ms_config.kospi_inverse {
                EtfType::KospiInverse
            } else if ticker_base == &ms_config.kosdaq_inverse {
                EtfType::KosdaqInverse
            } else {
                continue;
            };

            self.etf_data
                .insert(ticker.clone(), EtfData::new(ticker.clone(), etf_type));
        }

        self.config = Some(ms_config);
        self.initialized = true;

        Ok(())
    }

    async fn on_market_data(
        &mut self,
        data: &MarketData,
    ) -> Result<Vec<Signal>, Box<dyn std::error::Error + Send + Sync>> {
        if !self.initialized {
            return Ok(vec![]);
        }

        let ticker_base = data.ticker.clone();
        // etf_data는 "TICKER/KRW" 형식으로 저장됨, MarketData의 ticker는 "TICKER" 형식
        let ticker_key = format!("{}/KRW", ticker_base);

        // 등록된 ETF인지 확인
        if !self.etf_data.contains_key(&ticker_key) {
            return Ok(vec![]);
        }

        // kline에서 데이터 추출 (volume은 StrategyContext의 klines에서 사용)
        let (close, timestamp) = match &data.data {
            MarketDataType::Kline(kline) => (kline.close, kline.open_time),
            _ => return Ok(vec![]),
        };

        // 새 날짜 확인
        if self.is_new_day(timestamp) {
            self.current_date = Some(timestamp.date_naive());
        }

        // ETF 데이터 업데이트 (StrategyContext에서 klines 조회하므로 현재가만 저장)
        if let Some(etf) = self.etf_data.get_mut(&ticker_key) {
            etf.update_price(close);
        }

        // 충분한 데이터가 있는지 확인 (StrategyContext 기반)
        let data_status: Vec<(String, usize)> = self.etf_data.keys()
            .map(|ticker| {
                let klines = self.get_etf_klines(ticker);
                (ticker.clone(), klines.len())
            })
            .collect();

        let all_have_data = data_status.iter().all(|(_, len)| *len >= 60);

        if !all_have_data {
            debug!(
                data_status = ?data_status,
                "데이터 부족으로 신호 생성 스킵"
            );
            return Ok(vec![]);
        }

        self.started = true;
        debug!("MomentumSurge: 데이터 충분, 신호 생성 시작");

        // 신호 생성
        let signals = self.generate_signals();

        Ok(signals)
    }

    async fn on_order_filled(
        &mut self,
        order: &Order,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = order.ticker.to_string();
        let price = order.price.unwrap_or(Decimal::ZERO);

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            match order.side {
                Side::Buy => {
                    let old_value = etf.holdings * etf.entry_price;
                    let new_value = order.quantity * price;
                    let total_qty = etf.holdings + order.quantity;

                    if total_qty > Decimal::ZERO {
                        etf.entry_price = (old_value + new_value) / total_qty;
                    }
                    etf.holdings += order.quantity;
                }
                Side::Sell => {
                    let pnl = order.quantity * (price - etf.entry_price);
                    self.total_pnl += pnl;
                    if pnl > Decimal::ZERO {
                        self.wins += 1;
                    }
                    self.trades_count += 1;

                    etf.holdings -= order.quantity;
                    if etf.holdings <= Decimal::ZERO {
                        etf.holdings = Decimal::ZERO;
                        etf.entry_price = Decimal::ZERO;
                    }
                }
            }
        }

        debug!(
            ticker = %ticker,
            side = ?order.side,
            quantity = %order.quantity,
            price = %price,
            "주문 체결"
        );
        Ok(())
    }

    async fn on_position_update(
        &mut self,
        position: &Position,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ticker = position.ticker.to_string();

        if let Some(etf) = self.etf_data.get_mut(&ticker) {
            etf.holdings = position.quantity;
            if position.quantity > Decimal::ZERO {
                etf.entry_price = position.entry_price;
            }
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let win_rate = if self.trades_count > 0 {
            (self.wins as f64 / self.trades_count as f64) * 100.0
        } else {
            0.0
        };

        info!(
            trades = self.trades_count,
            wins = self.wins,
            win_rate = %format!("{:.1}%", win_rate),
            total_pnl = %self.total_pnl,
            "Momentum Surge 전략 종료"
        );

        Ok(())
    }

    fn set_context(&mut self, context: Arc<RwLock<StrategyContext>>) {
        self.context = Some(context);
        info!("StrategyContext injected into MomentumSurge strategy");
    }

    fn exit_config(&self) -> Option<&ExitConfig> {
        self.config.as_ref().map(|c| &c.exit_config)
    }

    fn get_state(&self) -> Value {
        let holdings: HashMap<_, _> = self
            .etf_data
            .iter()
            .filter(|(_, v)| v.holdings > Decimal::ZERO)
            .map(|(k, v)| {
                (
                    k.clone(),
                    json!({
                        "holdings": v.holdings.to_string(),
                        "entry_price": v.entry_price.to_string(),
                        "current_price": v.current_price.to_string(),
                        "etf_type": format!("{:?}", v.etf_type),
                    }),
                )
            })
            .collect();

        json!({
            "initialized": self.initialized,
            "started": self.started,
            "position_count": self.current_position_count(),
            "holdings": holdings,
            "trades_count": self.trades_count,
            "wins": self.wins,
            "total_pnl": self.total_pnl.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_momentum_surge_initialization() {
        let mut strategy = MomentumSurgeStrategy::new();

        let config = json!({
            "tickers": ["122630", "233740", "252670", "251340"],
            "max_positions": 2
        });

        strategy.initialize(config).await.unwrap();
        assert!(strategy.initialized);
        assert_eq!(strategy.etf_data.len(), 4);
    }

}

// 전략 레지스트리에 자동 등록
use crate::register_strategy;

register_strategy! {
    id: "momentum_surge",
    aliases: [],
    name: "Momentum Surge",
    description: "모멘텀 급등 포착 전략입니다.",
    timeframe: "15m",
    tickers: ["122630", "252670", "233740", "251340"],
    category: Intraday,
    markets: [Stock],
    type: MomentumSurgeStrategy,
    config: MomentumSurgeConfig
}
