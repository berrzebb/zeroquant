//! Strategy persistence repository.
//!
//! Handles database operations for storing and retrieving trading strategies.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::StrategyContext;
use uuid::Uuid;

/// Database representation of a strategy.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StrategyRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub strategy_type: Option<String>,
    pub symbols: Option<Value>,
    pub market: Option<String>,
    pub timeframe: Option<String>,
    pub version: Option<String>,
    pub is_active: bool,
    pub config: Value,
    pub risk_limits: Value,
    /// Capital allocated to this strategy (NULL = use full account balance)
    pub allocated_capital: Option<Decimal>,
    /// Risk profile: conservative, default, aggressive, or custom
    pub risk_profile: Option<String>,
    /// Multi-timeframe configuration (NULL = single timeframe strategy)
    /// Format: {"primary": "5m", "secondary": [{"timeframe": "1h", "candle_count": 24}]}
    pub multi_timeframe_config: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_started_at: Option<DateTime<Utc>>,
    pub last_stopped_at: Option<DateTime<Utc>>,
    /// 전략이 연결된 거래소 계정 ID (NULL이면 기본 활성 계정 사용)
    pub credential_id: Option<Uuid>,
}

/// Input for creating a new strategy.
#[derive(Debug)]
pub struct CreateStrategyInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub strategy_type: String,
    pub symbols: Vec<String>,
    pub market: String,
    pub timeframe: String,
    pub config: Value,
    /// Risk configuration for this strategy (optional)
    pub risk_config: Option<Value>,
    /// Capital allocated to this strategy (optional)
    pub allocated_capital: Option<Decimal>,
    /// Risk profile: conservative, default, aggressive, or custom
    pub risk_profile: Option<String>,
    /// Multi-timeframe configuration (optional)
    /// Format: {"primary": "5m", "secondary": [{"timeframe": "1h", "candle_count": 24}]}
    pub multi_timeframe_config: Option<Value>,
    /// 전략이 연결된 거래소 계정 ID (NULL이면 기본 활성 계정 사용)
    pub credential_id: Option<Uuid>,
}

/// Strategy repository for database operations.
pub struct StrategyRepository;

impl StrategyRepository {
    /// Save a new strategy to the database.
    ///
    /// Uses a transaction to ensure atomicity of the INSERT operation.
    pub async fn create(
        pool: &PgPool,
        input: CreateStrategyInput,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let symbols_json = serde_json::to_value(&input.symbols).unwrap_or(Value::Array(vec![]));
        let risk_limits = input.risk_config.unwrap_or_else(|| serde_json::json!({}));
        let risk_profile = input.risk_profile.unwrap_or_else(|| "default".to_string());

        let mut tx = pool.begin().await?;

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            INSERT INTO strategies (id, name, description, strategy_type, symbols, market, timeframe, config, risk_limits, allocated_capital, risk_profile, multi_timeframe_config, credential_id, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, false)
            RETURNING *
            "#
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.strategy_type)
        .bind(&symbols_json)
        .bind(&input.market)
        .bind(&input.timeframe)
        .bind(&input.config)
        .bind(&risk_limits)
        .bind(input.allocated_capital)
        .bind(&risk_profile)
        .bind(&input.multi_timeframe_config)
        .bind(input.credential_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// Get a strategy by ID.
    pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<StrategyRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>("SELECT * FROM strategies WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(record)
    }

    /// Get all strategies.
    pub async fn get_all(pool: &PgPool) -> Result<Vec<StrategyRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, StrategyRecord>(
            "SELECT * FROM strategies ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// Get strategies filtered by credential_id.
    /// If credential_id is None, returns all strategies.
    pub async fn get_by_credential(
        pool: &PgPool,
        credential_id: Option<Uuid>,
    ) -> Result<Vec<StrategyRecord>, sqlx::Error> {
        match credential_id {
            Some(cred_id) => {
                let records = sqlx::query_as::<_, StrategyRecord>(
                    "SELECT * FROM strategies WHERE credential_id = $1 ORDER BY created_at DESC",
                )
                .bind(cred_id)
                .fetch_all(pool)
                .await?;
                Ok(records)
            }
            None => Self::get_all(pool).await,
        }
    }

    /// Update strategy configuration.
    ///
    /// Uses a transaction to ensure atomicity of the UPDATE operation.
    pub async fn update_config(
        pool: &PgPool,
        id: &str,
        config: Value,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET config = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(config)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    /// Update strategy active status.
    pub async fn set_active(pool: &PgPool, id: &str, is_active: bool) -> Result<(), sqlx::Error> {
        let timestamp_field = if is_active {
            "last_started_at"
        } else {
            "last_stopped_at"
        };

        sqlx::query(&format!(
            r#"
            UPDATE strategies
            SET is_active = $2, {} = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
            timestamp_field
        ))
        .bind(id)
        .bind(is_active)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete a strategy by ID.
    ///
    /// Uses a transaction to ensure atomicity of the DELETE operation.
    pub async fn delete(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let result = sqlx::query("DELETE FROM strategies WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if a strategy exists by ID.
    pub async fn exists(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        let result: (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM strategies WHERE id = $1)")
                .bind(id)
                .fetch_one(pool)
                .await?;

        Ok(result.0)
    }

    /// Update strategy risk configuration.
    pub async fn update_risk_config(
        pool: &PgPool,
        id: &str,
        risk_config: Value,
        risk_profile: Option<&str>,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let profile = risk_profile.unwrap_or("custom");

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET risk_limits = $2, risk_profile = $3, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(risk_config)
        .bind(profile)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Update strategy allocated capital.
    pub async fn update_allocated_capital(
        pool: &PgPool,
        id: &str,
        allocated_capital: Option<Decimal>,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET allocated_capital = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(allocated_capital)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Update strategy risk settings (both risk config and allocated capital).
    pub async fn update_risk_settings(
        pool: &PgPool,
        id: &str,
        risk_config: Option<Value>,
        allocated_capital: Option<Decimal>,
        risk_profile: Option<&str>,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let profile = risk_profile.unwrap_or("custom");

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET
                risk_limits = COALESCE($2, risk_limits),
                allocated_capital = $3,
                risk_profile = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(risk_config)
        .bind(allocated_capital)
        .bind(profile)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Update strategy symbols (trading targets).
    pub async fn update_symbols(
        pool: &PgPool,
        id: &str,
        symbols: Vec<String>,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let symbols_json =
            serde_json::to_value(&symbols).unwrap_or(serde_json::Value::Array(vec![]));

        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET symbols = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(symbols_json)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 전략의 거래소 계정을 업데이트합니다.
    pub async fn update_credential(
        pool: &PgPool,
        id: &str,
        credential_id: Option<Uuid>,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET credential_id = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(credential_id)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// 전략의 대상 시장을 업데이트합니다.
    pub async fn update_market(
        pool: &PgPool,
        id: &str,
        market: &str,
    ) -> Result<StrategyRecord, sqlx::Error> {
        let record = sqlx::query_as::<_, StrategyRecord>(
            r#"
            UPDATE strategies
            SET market = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(market)
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    /// Load all strategies from the database and register them with the engine.
    ///
    /// This is called during server startup to restore previously saved strategies.
    pub async fn load_strategies_into_engine(
        pool: &PgPool,
        engine: &trader_strategy::StrategyEngine,
        shared_context: Option<Arc<RwLock<StrategyContext>>>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        use trader_strategy::strategies::{
            // 그룹 전략 (통합)
            AssetAllocationStrategy,
            DcaStrategy,
            // 독립 전략
            CandlePatternStrategy,
            CompoundMomentumStrategy,
            DayTradingStrategy,
            MarketBothSideStrategy,
            MeanReversionStrategy,
            MomentumPowerStrategy,
            MomentumSurgeStrategy,
            PensionBotStrategy,
            RangeTradingStrategy,
            RotationStrategy,
            SectorVbStrategy,
            SmallCapQuantStrategy,
            Us3xLeverageStrategy,
        };
        use trader_strategy::Strategy;

        let records = Self::get_all(pool).await?;
        let mut loaded_count = 0;

        for record in records {
            let strategy_type = record.strategy_type.as_deref().unwrap_or("unknown");

            // Create strategy instance based on type
            let strategy: Option<Box<dyn Strategy>> = match strategy_type {
                // MeanReversion 그룹 (RSI, Bollinger)
                "rsi" | "rsi_mean_reversion" => Some(Box::new(MeanReversionStrategy::rsi())),
                "rsi_multi_tf" | "rsi_mtf" | "multi_rsi" => {
                    Some(Box::new(MeanReversionStrategy::rsi()))
                }
                "bollinger" | "bollinger_bands" => {
                    Some(Box::new(MeanReversionStrategy::bollinger()))
                }

                // DCA 그룹 (Grid, Magic Split, Infinity Bot)
                "grid" | "grid_trading" => Some(Box::new(DcaStrategy::grid())),
                "magic_split" | "split" => Some(Box::new(DcaStrategy::magic_split())),
                "infinity_bot" => Some(Box::new(DcaStrategy::infinity_bot())),

                // DayTrading 그룹 (Volatility Breakout, SMA Crossover, Market Interest Day)
                "volatility_breakout" | "volatility" => {
                    Some(Box::new(DayTradingStrategy::breakout()))
                }
                "sma" | "sma_crossover" | "ma_crossover" => {
                    Some(Box::new(DayTradingStrategy::crossover()))
                }
                "market_interest_day" => Some(Box::new(DayTradingStrategy::volume_surge())),

                // AssetAllocation 그룹 (HAA, XAA, BAA, All Weather, Dual Momentum)
                "haa" => Some(Box::new(AssetAllocationStrategy::haa())),
                "xaa" => Some(Box::new(AssetAllocationStrategy::xaa())),
                "all_weather" => {
                    Some(Box::new(AssetAllocationStrategy::all_weather()))
                }
                "baa" => Some(Box::new(AssetAllocationStrategy::baa())),
                "dual_momentum" => Some(Box::new(AssetAllocationStrategy::dual_momentum())),

                // Rotation 그룹 (Stock Rotation, Sector Momentum, Market Cap Top)
                "stock_rotation" => Some(Box::new(RotationStrategy::stock_rotation())),
                "sector_momentum" => Some(Box::new(RotationStrategy::sector_momentum())),
                "market_cap_top" => Some(Box::new(RotationStrategy::market_cap_top())),

                // 독립 전략들
                "compound_momentum" => {
                    Some(Box::new(CompoundMomentumStrategy::new()))
                }
                "momentum_power" => {
                    Some(Box::new(MomentumPowerStrategy::new()))
                }
                "pension_bot" => Some(Box::new(PensionBotStrategy::new())),
                "candle_pattern" => Some(Box::new(CandlePatternStrategy::new())),
                "sector_vb" => Some(Box::new(SectorVbStrategy::new())),
                "market_both_side" => {
                    Some(Box::new(MarketBothSideStrategy::new()))
                }
                "momentum_surge" => {
                    Some(Box::new(MomentumSurgeStrategy::new()))
                }
                "us_3x_leverage" => Some(Box::new(Us3xLeverageStrategy::new())),
                "range_trading" => Some(Box::new(RangeTradingStrategy::new())),
                "small_cap_quant" => Some(Box::new(SmallCapQuantStrategy::new())),

                _ => {
                    tracing::warn!(
                        "Unknown strategy type: {} for strategy {}",
                        strategy_type,
                        record.id
                    );
                    None
                }
            };

            if let Some(strategy) = strategy {
                match engine
                    .register_strategy(
                        &record.id,
                        strategy,
                        record.config.clone(),
                        Some(record.name.clone()),
                        shared_context.clone(),
                    )
                    .await
                {
                    Ok(_) => {
                        tracing::info!("Loaded strategy from DB: {} ({})", record.name, record.id);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        tracing::error!("Failed to load strategy {}: {:?}", record.id, e);
                    }
                }
            }
        }

        Ok(loaded_count)
    }
}
