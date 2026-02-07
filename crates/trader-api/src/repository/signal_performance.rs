//! 신호 성과 Repository
//!
//! 신호 발생 후 수익률을 추적하고 통계를 제공합니다.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use ts_rs::TS;
use utoipa::ToSchema;

/// 신호 타입별 통계
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalTypeStats {
    /// 신호 타입 (Entry, Exit 등)
    pub signal_type: String,
    /// 방향 (Buy, Sell)
    pub side: Option<String>,
    /// 총 신호 수
    pub total_signals: i64,
    /// 승리 횟수
    pub win_count: i64,
    /// 패배 횟수
    pub loss_count: i64,
    /// 승률 (%)
    #[ts(type = "number | null")]
    pub win_rate: Option<f64>,
    /// 평균 1일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_1d: Option<f64>,
    /// 평균 5일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_5d: Option<f64>,
    /// 평균 10일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_10d: Option<f64>,
    /// 평균 MFE (%)
    #[ts(type = "number | null")]
    pub avg_max_return: Option<f64>,
    /// 평균 MAE (%)
    #[ts(type = "number | null")]
    pub avg_max_drawdown: Option<f64>,
}

/// 신호 강도별 통계
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalStrengthStats {
    /// 강도 범위 (예: "80-90")
    pub strength_range: String,
    /// 방향 (Buy, Sell)
    pub side: Option<String>,
    /// 총 신호 수
    pub total_signals: i64,
    /// 승률 (%)
    #[ts(type = "number | null")]
    pub win_rate: Option<f64>,
    /// 평균 5일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_5d: Option<f64>,
    /// 평균 MFE (%)
    #[ts(type = "number | null")]
    pub avg_max_return: Option<f64>,
    /// 평균 MAE (%)
    #[ts(type = "number | null")]
    pub avg_max_drawdown: Option<f64>,
}

/// 심볼별 신호 통계
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalSymbolStats {
    /// 종목 코드
    pub ticker: String,
    /// 종목명
    pub symbol_name: Option<String>,
    /// 시장
    pub market: Option<String>,
    /// 총 신호 수
    pub total_signals: i64,
    /// 매수 신호 수
    pub buy_count: i64,
    /// 매도 신호 수
    pub sell_count: i64,
    /// 승률 (%)
    #[ts(type = "number | null")]
    pub win_rate: Option<f64>,
    /// 평균 5일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_5d: Option<f64>,
    /// 평균 신호 강도
    #[ts(type = "number | null")]
    pub avg_strength: Option<f64>,
}

/// 전략별 신호 통계
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalStrategyStats {
    /// 전략 ID
    pub strategy_id: String,
    /// 총 신호 수
    pub total_signals: i64,
    /// 승리 횟수
    pub win_count: i64,
    /// 승률 (%)
    #[ts(type = "number | null")]
    pub win_rate: Option<f64>,
    /// 평균 1일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_1d: Option<f64>,
    /// 평균 5일 수익률 (%)
    #[ts(type = "number | null")]
    pub avg_return_5d: Option<f64>,
    /// 평균 신호 강도
    #[ts(type = "number | null")]
    pub avg_strength: Option<f64>,
    /// 평균 MFE (%)
    #[ts(type = "number | null")]
    pub avg_mfe: Option<f64>,
    /// 평균 MAE (%)
    #[ts(type = "number | null")]
    pub avg_mae: Option<f64>,
}

/// 신호-수익률 상관관계 데이터 포인트 (산점도용)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalReturnPoint {
    /// 신호 ID
    pub signal_id: String,
    /// 신호 강도 (0-1)
    pub strength: f64,
    /// 5일 수익률 (%)
    pub return_5d: Option<f64>,
    /// 신호 타입
    pub signal_type: String,
    /// 방향
    pub side: Option<String>,
    /// 종목 코드
    pub ticker: String,
}

/// 신호 성과 종합 응답
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS)]
#[ts(export, export_to = "signals/")]
pub struct SignalPerformanceResponse {
    /// 타입별 통계
    pub type_stats: Vec<SignalTypeStats>,
    /// 강도별 통계
    pub strength_stats: Vec<SignalStrengthStats>,
    /// 심볼별 통계 (상위 10개)
    pub symbol_stats: Vec<SignalSymbolStats>,
    /// 전략별 통계
    pub strategy_stats: Vec<SignalStrategyStats>,
    /// 총 분석 대상 신호 수
    pub total_analyzed: i64,
    /// 전체 승률 (%)
    pub overall_win_rate: Option<f64>,
    /// 전체 평균 수익률 (%)
    pub overall_avg_return: Option<f64>,
}

/// 신호 성과 Repository
pub struct SignalPerformanceRepository;

impl SignalPerformanceRepository {
    /// 타입별 신호 통계 조회
    pub async fn get_type_stats(pool: &PgPool) -> Result<Vec<SignalTypeStats>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                signal_type,
                side,
                total_signals,
                win_count,
                loss_count,
                win_rate,
                avg_return_1d,
                avg_return_5d,
                avg_return_10d,
                avg_max_return,
                avg_max_drawdown
            FROM v_signal_type_stats
            ORDER BY total_signals DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|row| SignalTypeStats {
                signal_type: row.get("signal_type"),
                side: row.get("side"),
                total_signals: row.get("total_signals"),
                win_count: row.get("win_count"),
                loss_count: row.get("loss_count"),
                win_rate: row.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_1d: row.get::<Option<Decimal>, _>("avg_return_1d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_5d: row.get::<Option<Decimal>, _>("avg_return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_10d: row.get::<Option<Decimal>, _>("avg_return_10d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_max_return: row.get::<Option<Decimal>, _>("avg_max_return").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_max_drawdown: row.get::<Option<Decimal>, _>("avg_max_drawdown").map(|d| d.to_string().parse().unwrap_or(0.0)),
            })
            .collect();

        Ok(stats)
    }

    /// 강도별 신호 통계 조회
    pub async fn get_strength_stats(pool: &PgPool) -> Result<Vec<SignalStrengthStats>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                strength_range,
                side,
                total_signals,
                win_rate,
                avg_return_5d,
                avg_max_return,
                avg_max_drawdown
            FROM v_signal_strength_stats
            ORDER BY strength_range DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|row| SignalStrengthStats {
                strength_range: row.get("strength_range"),
                side: row.get("side"),
                total_signals: row.get("total_signals"),
                win_rate: row.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_5d: row.get::<Option<Decimal>, _>("avg_return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_max_return: row.get::<Option<Decimal>, _>("avg_max_return").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_max_drawdown: row.get::<Option<Decimal>, _>("avg_max_drawdown").map(|d| d.to_string().parse().unwrap_or(0.0)),
            })
            .collect();

        Ok(stats)
    }

    /// 심볼별 신호 통계 조회 (상위 N개)
    pub async fn get_symbol_stats(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SignalSymbolStats>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                ticker,
                symbol_name,
                market,
                total_signals,
                buy_count,
                sell_count,
                win_rate,
                avg_return_5d,
                avg_strength
            FROM v_signal_symbol_stats
            ORDER BY total_signals DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|row| SignalSymbolStats {
                ticker: row.get("ticker"),
                symbol_name: row.get("symbol_name"),
                market: row.get("market"),
                total_signals: row.get("total_signals"),
                buy_count: row.get("buy_count"),
                sell_count: row.get("sell_count"),
                win_rate: row.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_5d: row.get::<Option<Decimal>, _>("avg_return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_strength: row.get::<Option<Decimal>, _>("avg_strength").map(|d| d.to_string().parse().unwrap_or(0.0)),
            })
            .collect();

        Ok(stats)
    }

    /// 전략별 신호 통계 조회
    pub async fn get_strategy_stats(pool: &PgPool) -> Result<Vec<SignalStrategyStats>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                strategy_id,
                total_signals,
                win_count,
                win_rate,
                avg_return_1d,
                avg_return_5d,
                avg_strength,
                avg_mfe,
                avg_mae
            FROM v_signal_strategy_stats
            ORDER BY win_rate DESC NULLS LAST
            "#,
        )
        .fetch_all(pool)
        .await?;

        let stats = rows
            .iter()
            .map(|row| SignalStrategyStats {
                strategy_id: row.get("strategy_id"),
                total_signals: row.get("total_signals"),
                win_count: row.get("win_count"),
                win_rate: row.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_1d: row.get::<Option<Decimal>, _>("avg_return_1d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_return_5d: row.get::<Option<Decimal>, _>("avg_return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_strength: row.get::<Option<Decimal>, _>("avg_strength").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_mfe: row.get::<Option<Decimal>, _>("avg_mfe").map(|d| d.to_string().parse().unwrap_or(0.0)),
                avg_mae: row.get::<Option<Decimal>, _>("avg_mae").map(|d| d.to_string().parse().unwrap_or(0.0)),
            })
            .collect();

        Ok(stats)
    }

    /// 특정 심볼의 신호-수익률 상관관계 데이터 조회 (산점도용)
    pub async fn get_return_scatter(
        pool: &PgPool,
        ticker: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SignalReturnPoint>, sqlx::Error> {
        let rows = if let Some(t) = ticker {
            sqlx::query(
                r#"
                SELECT
                    signal_id::text,
                    strength,
                    return_5d,
                    signal_type,
                    side,
                    ticker
                FROM signal_performance
                WHERE ticker = $1 AND calculated_at IS NOT NULL
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(t)
            .bind(limit)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    signal_id::text,
                    strength,
                    return_5d,
                    signal_type,
                    side,
                    ticker
                FROM signal_performance
                WHERE calculated_at IS NOT NULL
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(pool)
            .await?
        };

        let points = rows
            .iter()
            .map(|row| SignalReturnPoint {
                signal_id: row.get("signal_id"),
                strength: row.get::<Decimal, _>("strength").to_string().parse().unwrap_or(0.0),
                return_5d: row.get::<Option<Decimal>, _>("return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
                signal_type: row.get("signal_type"),
                side: row.get("side"),
                ticker: row.get("ticker"),
            })
            .collect();

        Ok(points)
    }

    /// 전체 신호 성과 종합 조회
    pub async fn get_performance_summary(
        pool: &PgPool,
    ) -> Result<SignalPerformanceResponse, sqlx::Error> {
        // 각 통계 병렬 조회
        let (type_stats, strength_stats, symbol_stats, strategy_stats) = tokio::try_join!(
            Self::get_type_stats(pool),
            Self::get_strength_stats(pool),
            Self::get_symbol_stats(pool, 10),
            Self::get_strategy_stats(pool),
        )?;

        // 전체 통계 계산
        let overall = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                ROUND(100.0 * COUNT(*) FILTER (WHERE is_winner = true) / NULLIF(COUNT(*) FILTER (WHERE is_winner IS NOT NULL), 0), 2) as win_rate,
                ROUND(AVG(return_5d)::NUMERIC, 4) as avg_return
            FROM signal_performance
            WHERE calculated_at IS NOT NULL
            "#,
        )
        .fetch_one(pool)
        .await?;

        Ok(SignalPerformanceResponse {
            type_stats,
            strength_stats,
            symbol_stats,
            strategy_stats,
            total_analyzed: overall.get::<Option<i64>, _>("total").unwrap_or(0),
            overall_win_rate: overall.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
            overall_avg_return: overall.get::<Option<Decimal>, _>("avg_return").map(|d| d.to_string().parse().unwrap_or(0.0)),
        })
    }

    /// 특정 심볼의 신호 성과 조회
    pub async fn get_symbol_performance(
        pool: &PgPool,
        ticker: &str,
    ) -> Result<Option<SignalSymbolStats>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
                ticker,
                symbol_name,
                market,
                total_signals,
                buy_count,
                sell_count,
                win_rate,
                avg_return_5d,
                avg_strength
            FROM v_signal_symbol_stats
            WHERE ticker = $1
            "#,
        )
        .bind(ticker)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| SignalSymbolStats {
            ticker: r.get("ticker"),
            symbol_name: r.get("symbol_name"),
            market: r.get("market"),
            total_signals: r.get("total_signals"),
            buy_count: r.get("buy_count"),
            sell_count: r.get("sell_count"),
            win_rate: r.get::<Option<Decimal>, _>("win_rate").map(|d| d.to_string().parse().unwrap_or(0.0)),
            avg_return_5d: r.get::<Option<Decimal>, _>("avg_return_5d").map(|d| d.to_string().parse().unwrap_or(0.0)),
            avg_strength: r.get::<Option<Decimal>, _>("avg_strength").map(|d| d.to_string().parse().unwrap_or(0.0)),
        }))
    }
}
