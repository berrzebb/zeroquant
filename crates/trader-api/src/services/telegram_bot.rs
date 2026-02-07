//! 텔레그램 봇 서비스.
//!
//! 실제 데이터를 조회하여 봇 명령어에 응답합니다.

use std::sync::Arc;

use async_trait::async_trait;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, info};

use trader_notification::{
    BotCommandHandler, CommandResponse, NotificationResult, ReportPeriod, TelegramBotHandler,
    TelegramConfig,
};

/// API 연동 봇 핸들러.
///
/// 실제 데이터베이스를 조회하여 응답합니다.
pub struct ApiBotHandler {
    db_pool: PgPool,
}

impl ApiBotHandler {
    /// 새 핸들러 생성.
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// 봇 핸들러 시작 (백그라운드 태스크).
    pub async fn start(db_pool: PgPool) {
        let Some(config) = TelegramConfig::from_env() else {
            info!("텔레그램 설정이 없어 봇 명령어 핸들러를 시작하지 않습니다.");
            return;
        };

        let handler = Arc::new(ApiBotHandler::new(db_pool));
        let bot = TelegramBotHandler::new(config, handler);

        info!("텔레그램 봇 명령어 핸들러 시작");
        bot.start_polling().await;
    }

    /// 금액 포맷팅 (한국 원화).
    fn format_krw(amount: Decimal) -> String {
        let amount_f64 = amount.to_f64().unwrap_or(0.0);
        if amount_f64.abs() >= 100_000_000.0 {
            format!("{:.1}억", amount_f64 / 100_000_000.0)
        } else if amount_f64.abs() >= 10_000.0 {
            format!("{:.0}만", amount_f64 / 10_000.0)
        } else {
            format!("{:.0}", amount_f64)
        }
    }

    /// 수익률 포맷팅.
    fn format_pct(pct: Decimal) -> String {
        let sign = if pct >= Decimal::ZERO { "+" } else { "" };
        format!("{}{:.2}%", sign, pct)
    }

    /// 손익 색상 이모지.
    fn pnl_emoji(pnl: Decimal) -> &'static str {
        if pnl > Decimal::ZERO {
            "📈"
        } else if pnl < Decimal::ZERO {
            "📉"
        } else {
            "➖"
        }
    }
}

#[async_trait]
impl BotCommandHandler for ApiBotHandler {
    async fn handle_portfolio(&self) -> NotificationResult<CommandResponse> {
        debug!("포트폴리오 조회 시작");

        // 보유 현황 조회 (모든 credential의 포지션)
        let positions_result: Result<
            Vec<(
                String,
                Decimal,
                Option<Decimal>,
                Option<Decimal>,
                Option<Decimal>,
            )>,
            _,
        > = sqlx::query_as(
            r#"
            SELECT DISTINCT ON (symbol)
                symbol,
                quantity,
                market_value,
                unrealized_pnl,
                unrealized_pnl_pct
            FROM position_snapshots
            WHERE quantity > 0
            ORDER BY symbol, snapshot_time DESC
            LIMIT 10
            "#,
        )
        .fetch_all(&self.db_pool)
        .await;

        match positions_result {
            Ok(positions) => {
                if positions.is_empty() {
                    return Ok(CommandResponse::html(
                        "📊 <b>포트폴리오 현황</b>\n\n\
                         보유 종목이 없습니다.",
                    ));
                }

                let mut lines = vec!["📊 <b>포트폴리오 현황</b>\n".to_string()];

                let mut total_value = Decimal::ZERO;
                let mut total_pnl = Decimal::ZERO;

                for (symbol, quantity, market_value, unrealized_pnl, unrealized_pnl_pct) in
                    positions
                {
                    let pnl = unrealized_pnl.unwrap_or(Decimal::ZERO);
                    let pnl_pct = unrealized_pnl_pct.unwrap_or(Decimal::ZERO);
                    let value = market_value.unwrap_or(Decimal::ZERO);

                    let pnl_emoji = Self::pnl_emoji(pnl);
                    let pnl_pct_str = Self::format_pct(pnl_pct);

                    lines.push(format!(
                        "\n<code>{}</code> {} {}\n  평가: {} | 손익: {} ({})",
                        symbol,
                        quantity,
                        "주",
                        Self::format_krw(value),
                        pnl_emoji,
                        pnl_pct_str,
                    ));

                    total_value += value;
                    total_pnl += pnl;
                }

                let total_pnl_pct = if total_value > Decimal::ZERO && total_value != total_pnl {
                    total_pnl / (total_value - total_pnl) * Decimal::from(100)
                } else {
                    Decimal::ZERO
                };

                lines.push(format!(
                    "\n━━━━━━━━━━━━━━━━━━\n\
                     💰 총 평가액: {}\n\
                     {} 총 손익: {} ({})",
                    Self::format_krw(total_value),
                    Self::pnl_emoji(total_pnl),
                    Self::format_krw(total_pnl),
                    Self::format_pct(total_pnl_pct),
                ));

                Ok(CommandResponse::html(lines.join("")))
            }
            Err(e) => {
                error!("포트폴리오 조회 실패: {}", e);
                Ok(CommandResponse::html(format!(
                    "📊 <b>포트폴리오 현황</b>\n\n\
                     ❌ 조회 실패: {}",
                    e
                )))
            }
        }
    }

    async fn handle_status(&self) -> NotificationResult<CommandResponse> {
        debug!("시스템 상태 조회");

        // 간단한 상태 확인 (DB 연결 등)
        let db_status = sqlx::query("SELECT 1")
            .fetch_one(&self.db_pool)
            .await
            .is_ok();

        let status_emoji = if db_status { "✅" } else { "❌" };

        // 활성 전략 수 조회
        let active_strategies: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM strategies WHERE status = 'active'")
                .fetch_one(&self.db_pool)
                .await
                .unwrap_or(0);

        // 오늘 거래 수
        let today_trades: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM executions WHERE DATE(executed_at) = CURRENT_DATE",
        )
        .fetch_one(&self.db_pool)
        .await
        .unwrap_or(0);

        Ok(CommandResponse::html(format!(
            "🔍 <b>시스템 상태</b>\n\n\
             {status_emoji} 데이터베이스: {db_status}\n\
             🤖 활성 전략: {active_strategies}개\n\
             📝 오늘 거래: {today_trades}건\n\n\
             <i>마지막 확인: {}</i>",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            db_status = if db_status { "정상" } else { "연결 실패" },
        )))
    }

    async fn handle_stop(&self, strategy_id: Option<&str>) -> NotificationResult<CommandResponse> {
        let Some(id) = strategy_id else {
            // 활성 전략 목록 표시
            let strategies: Vec<(String, String)> = sqlx::query_as(
                "SELECT strategy_id, strategy_type FROM strategies WHERE status = 'active' LIMIT 10"
            )
            .fetch_all(&self.db_pool)
            .await
            .unwrap_or_default();

            if strategies.is_empty() {
                return Ok(CommandResponse::html(
                    "⏹️ <b>전략 중지</b>\n\n\
                     활성 전략이 없습니다.",
                ));
            }

            let list: Vec<String> = strategies
                .iter()
                .map(|(id, type_)| format!("• <code>{}</code> ({})", id, type_))
                .collect();

            return Ok(CommandResponse::html(format!(
                "⏹️ <b>전략 중지</b>\n\n\
                 <b>활성 전략:</b>\n{}\n\n\
                 사용법: /stop [전략ID]",
                list.join("\n")
            )));
        };

        // 전략 상태 업데이트
        let result = sqlx::query(
            "UPDATE strategies SET status = 'stopped', updated_at = NOW() WHERE strategy_id = $1",
        )
        .bind(id)
        .execute(&self.db_pool)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                info!(strategy_id = id, "전략 중지 성공");
                Ok(CommandResponse::html(format!(
                    "⏹️ <b>전략 중지 완료</b>\n\n\
                     전략 ID: <code>{}</code>\n\
                     상태: 중지됨",
                    id
                )))
            }
            Ok(_) => Ok(CommandResponse::html(format!(
                "⏹️ <b>전략 중지 실패</b>\n\n\
                 전략 ID <code>{}</code>를 찾을 수 없습니다.",
                id
            ))),
            Err(e) => {
                error!(strategy_id = id, error = %e, "전략 중지 실패");
                Ok(CommandResponse::html(format!(
                    "⏹️ <b>전략 중지 실패</b>\n\n\
                     오류: {}",
                    e
                )))
            }
        }
    }

    async fn handle_report(&self, period: ReportPeriod) -> NotificationResult<CommandResponse> {
        let (period_name, date_filter) = match period {
            ReportPeriod::Daily => ("일일", "DATE(executed_at) = CURRENT_DATE"),
            ReportPeriod::Weekly => ("주간", "executed_at >= CURRENT_DATE - INTERVAL '7 days'"),
            ReportPeriod::Monthly => ("월간", "executed_at >= CURRENT_DATE - INTERVAL '30 days'"),
        };

        // 거래 통계 조회
        let stats: Option<(i64, Decimal, Decimal)> = sqlx::query_as(&format!(
            "SELECT
                COUNT(*) as trades,
                COALESCE(SUM(realized_pnl), 0) as total_pnl,
                COALESCE(SUM(CASE WHEN realized_pnl > 0 THEN 1 ELSE 0 END)::decimal / NULLIF(COUNT(*), 0) * 100, 0) as win_rate
             FROM executions
             WHERE {}",
            date_filter
        ))
        .fetch_optional(&self.db_pool)
        .await
        .ok()
        .flatten();

        match stats {
            Some((trades, total_pnl, win_rate)) => {
                let pnl_emoji = Self::pnl_emoji(total_pnl);

                Ok(CommandResponse::html(format!(
                    "📈 <b>{} 리포트</b>\n\n\
                     📝 총 거래: {}건\n\
                     {} 총 손익: {}\n\
                     🎯 승률: {:.1}%\n\n\
                     <i>기간: {}</i>",
                    period_name,
                    trades,
                    pnl_emoji,
                    Self::format_krw(total_pnl),
                    win_rate.to_f64().unwrap_or(0.0),
                    match period {
                        ReportPeriod::Daily => "오늘",
                        ReportPeriod::Weekly => "최근 7일",
                        ReportPeriod::Monthly => "최근 30일",
                    }
                )))
            }
            None => Ok(CommandResponse::html(format!(
                "📈 <b>{} 리포트</b>\n\n\
                 해당 기간 거래 내역이 없습니다.",
                period_name
            ))),
        }
    }

    async fn handle_attack(&self) -> NotificationResult<CommandResponse> {
        debug!("ATTACK 상태 종목 조회");

        // GlobalScore 테이블에서 ATTACK 상태 종목 조회
        let attack_symbols: Vec<(String, Decimal, String)> = sqlx::query_as(
            "SELECT
                ticker,
                overall_score,
                recommendation
             FROM symbol_global_score
             WHERE recommendation IN ('ATTACK', 'BUY')
             ORDER BY overall_score DESC
             LIMIT 10",
        )
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        if attack_symbols.is_empty() {
            return Ok(CommandResponse::html(
                "🎯 <b>ATTACK 상태 종목</b>\n\n\
                 현재 ATTACK 상태 종목이 없습니다.",
            ));
        }

        let mut lines = vec!["🎯 <b>ATTACK 상태 종목</b>\n".to_string()];

        for (i, (ticker, score, rec)) in attack_symbols.iter().enumerate() {
            let rank = i + 1;
            let rec_emoji = match rec.as_str() {
                "ATTACK" => "🔥",
                "BUY" => "🟢",
                _ => "⚪",
            };

            lines.push(format!(
                "\n{}. <code>{}</code> {} ({:.0}점)",
                rank, ticker, rec_emoji, score
            ));
        }

        lines.push(format!(
            "\n\n<i>업데이트: {}</i>",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        ));

        Ok(CommandResponse::html(lines.join("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_krw() {
        assert_eq!(
            ApiBotHandler::format_krw(Decimal::from(150_000_000)),
            "1.5억"
        );
        assert_eq!(ApiBotHandler::format_krw(Decimal::from(5_000_000)), "500만");
        assert_eq!(ApiBotHandler::format_krw(Decimal::from(5_000)), "5000");
    }

    #[test]
    fn test_format_pct() {
        assert_eq!(ApiBotHandler::format_pct(Decimal::from(5)), "+5.00%");
        assert_eq!(ApiBotHandler::format_pct(Decimal::from(-3)), "-3.00%");
    }
}
