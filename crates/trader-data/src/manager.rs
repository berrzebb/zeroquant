//! 데이터 매니저 구현.
//!
//! 스토리지(TimescaleDB)와 캐시(Redis) 사이를 조정하여
//! 자동 캐싱을 통한 효율적인 데이터 접근을 제공합니다.

use crate::error::Result;
use crate::storage::redis::{RedisCache, RedisConfig};
use crate::storage::timescale::{
    Database, DatabaseConfig, OrderRecord, OrderRepository, PositionRecord, PositionRepository,
    SymbolRecord, SymbolRepository, TradeRecord, TradeRepository, TradeTickRecord,
    TradeTickRepository,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};
use trader_core::{Order, OrderBook, OrderStatusType, Side, Ticker, TradeTick};
use uuid::Uuid;

/// 데이터 매니저 설정.
#[derive(Debug, Clone)]
pub struct DataManagerConfig {
    pub database: DatabaseConfig,
    pub cache: RedisConfig,
    /// 심볼/타임프레임별 캐시에 보관할 최대 Kline 수
    pub max_cached_klines: usize,
    /// 캐시 사용 여부
    pub cache_enabled: bool,
}

impl Default for DataManagerConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            cache: RedisConfig::default(),
            max_cached_klines: 500,
            cache_enabled: true,
        }
    }
}

/// 스토리지와 캐시를 조정하는 중앙 데이터 매니저.
pub struct DataManager {
    db: Database,
    cache: Option<RedisCache>,
    #[allow(dead_code)]  // 향후 설정 조회용으로 보관
    config: DataManagerConfig,

    // 리포지토리
    symbols: SymbolRepository,
    trade_ticks: TradeTickRepository,
    orders: OrderRepository,
    trades: TradeRepository,
    positions: PositionRepository,

    // 심볼 ID 캐시 (인메모리)
    symbol_cache: Arc<RwLock<std::collections::HashMap<(String, String), Uuid>>>,
}

impl DataManager {
    /// 새 데이터 매니저를 생성합니다.
    pub async fn new(config: DataManagerConfig) -> Result<Self> {
        info!("Initializing DataManager...");

        // 데이터베이스 연결
        let db = Database::connect(&config.database).await?;

        // 캐시가 활성화된 경우 연결
        let cache = if config.cache_enabled {
            match RedisCache::connect(&config.cache).await {
                Ok(cache) => {
                    info!("Redis cache connected");
                    Some(cache)
                }
                Err(e) => {
                    warn!(
                        "Failed to connect to Redis cache: {}. Continuing without cache.",
                        e
                    );
                    None
                }
            }
        } else {
            info!("Cache disabled");
            None
        };

        // 리포지토리 생성
        let symbols = SymbolRepository::new(db.clone());
        let trade_ticks = TradeTickRepository::new(db.clone());
        let orders = OrderRepository::new(db.clone());
        let trades = TradeRepository::new(db.clone());
        let positions = PositionRepository::new(db.clone());

        info!("DataManager initialized successfully");

        Ok(Self {
            db,
            cache,
            config,
            symbols,
            trade_ticks,
            orders,
            trades,
            positions,
            symbol_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// 데이터베이스 마이그레이션을 실행합니다.
    pub async fn migrate(&self) -> Result<()> {
        self.db.migrate().await
    }

    /// 모든 연결의 상태를 확인합니다.
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let db_healthy = self.db.health_check().await.unwrap_or(false);

        let cache_healthy = if let Some(cache) = &self.cache {
            cache.health_check().await.unwrap_or(false)
        } else {
            true // 캐시가 없는 것은 비정상이 아님
        };

        Ok(HealthStatus {
            database: db_healthy,
            cache: cache_healthy,
            overall: db_healthy && cache_healthy,
        })
    }

    /// 데이터베이스 연결을 가져옵니다.
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// 캐시 연결을 가져옵니다.
    pub fn cache(&self) -> Option<&RedisCache> {
        self.cache.as_ref()
    }

    // =========================================================================
    // 심볼 작업
    // =========================================================================

    /// 스토리지 작업을 위한 심볼 ID를 가져오거나 생성합니다.
    #[instrument(skip(self))]
    pub async fn get_symbol_id(&self, ticker: &str, exchange: &str) -> Result<Uuid> {
        let key = (exchange.to_string(), ticker.to_string());

        // 먼저 인메모리 캐시 확인
        {
            let cache = self.symbol_cache.read().await;
            if let Some(id) = cache.get(&key) {
                return Ok(*id);
            }
        }

        // 데이터베이스에서 가져오거나 생성
        // TODO: SymbolResolver를 통해 ticker로부터 quote, market_type 조회
        // 임시로 기본값 사용
        let quote = if exchange == "kis" { "KRW" } else { "USD" };
        let market_type = "stock";
        let id = self
            .symbols
            .get_or_create(ticker, quote, market_type, exchange)
            .await?;

        // 메모리 캐시에 저장
        {
            let mut cache = self.symbol_cache.write().await;
            cache.insert(key, id);
        }

        Ok(id)
    }

    /// 거래소의 활성 심볼 목록을 조회합니다.
    pub async fn list_symbols(&self, exchange: &str) -> Result<Vec<SymbolRecord>> {
        self.symbols.list_active(exchange).await
    }

    // =========================================================================
    // Ticker 작업
    // =========================================================================

    /// Ticker를 캐시에 저장합니다.
    pub async fn store_ticker(&self, exchange: &str, ticker: &Ticker) -> Result<()> {
        if let Some(cache) = &self.cache {
            cache.set_ticker(exchange, ticker).await?;
        }
        Ok(())
    }

    /// 캐시된 Ticker를 가져옵니다.
    pub async fn get_ticker(&self, exchange: &str, symbol: &str) -> Result<Option<Ticker>> {
        if let Some(cache) = &self.cache {
            return cache.get_ticker(exchange, symbol).await;
        }
        Ok(None)
    }

    // =========================================================================
    // 오더북 작업
    // =========================================================================

    /// 오더북을 캐시에 저장합니다.
    pub async fn store_orderbook(&self, exchange: &str, orderbook: &OrderBook) -> Result<()> {
        if let Some(cache) = &self.cache {
            cache.set_orderbook(exchange, orderbook).await?;
        }
        Ok(())
    }

    /// 캐시된 오더북을 가져옵니다.
    pub async fn get_orderbook(&self, exchange: &str, symbol: &str) -> Result<Option<OrderBook>> {
        if let Some(cache) = &self.cache {
            return cache.get_orderbook(exchange, symbol).await;
        }
        Ok(None)
    }

    // =========================================================================
    // 체결 틱 작업
    // =========================================================================

    /// 체결 틱을 저장합니다.
    pub async fn store_trade_tick(&self, exchange: &str, trade: &TradeTick) -> Result<()> {
        let symbol_id = self.get_symbol_id(&trade.ticker, exchange).await?;
        self.trade_ticks.insert(symbol_id, trade).await
    }

    /// 여러 체결 틱을 저장합니다.
    pub async fn store_trade_ticks(&self, exchange: &str, trades: &[TradeTick]) -> Result<usize> {
        if trades.is_empty() {
            return Ok(0);
        }

        let symbol_id = self.get_symbol_id(&trades[0].ticker, exchange).await?;
        self.trade_ticks.insert_batch(symbol_id, trades).await
    }

    /// 최근 체결 틱을 가져옵니다.
    pub async fn get_recent_trades(
        &self,
        exchange: &str,
        ticker: &str,
        count: i32,
    ) -> Result<Vec<TradeTickRecord>> {
        let symbol_id = self.get_symbol_id(ticker, exchange).await?;
        self.trade_ticks.get_recent(symbol_id, count).await
    }

    // =========================================================================
    // 주문 작업
    // =========================================================================

    /// 주문을 저장합니다.
    pub async fn store_order(&self, order: &Order) -> Result<()> {
        self.orders.insert(order).await
    }

    /// 주문 상태를 업데이트합니다.
    pub async fn update_order_status(
        &self,
        order_id: Uuid,
        status: OrderStatusType,
        filled_quantity: Decimal,
        average_fill_price: Option<Decimal>,
    ) -> Result<()> {
        self.orders
            .update_status(order_id, status, filled_quantity, average_fill_price)
            .await
    }

    /// 거래소 주문 ID를 설정합니다.
    pub async fn set_exchange_order_id(
        &self,
        order_id: Uuid,
        exchange_order_id: &str,
    ) -> Result<()> {
        self.orders
            .set_exchange_order_id(order_id, exchange_order_id)
            .await
    }

    /// ID로 주문을 조회합니다.
    pub async fn get_order(&self, order_id: Uuid) -> Result<OrderRecord> {
        self.orders.get_by_id(order_id).await
    }

    /// 거래소 주문 ID로 주문을 조회합니다.
    pub async fn get_order_by_exchange_id(
        &self,
        exchange: &str,
        exchange_order_id: &str,
    ) -> Result<Option<OrderRecord>> {
        self.orders
            .get_by_exchange_id(exchange, exchange_order_id)
            .await
    }

    /// 미체결 주문을 조회합니다.
    pub async fn get_open_orders(&self, exchange: Option<&str>) -> Result<Vec<OrderRecord>> {
        self.orders.get_open_orders(exchange).await
    }

    /// 전략별 주문을 조회합니다.
    pub async fn get_strategy_orders(&self, strategy_id: &str) -> Result<Vec<OrderRecord>> {
        self.orders.get_by_strategy(strategy_id).await
    }

    /// 최근 주문을 조회합니다.
    pub async fn get_recent_orders(
        &self,
        exchange: Option<&str>,
        limit: i32,
    ) -> Result<Vec<OrderRecord>> {
        self.orders.get_recent(exchange, limit).await
    }

    // =========================================================================
    // 체결 작업 (실행된 거래)
    // =========================================================================

    /// 거래 체결을 저장합니다.
    pub async fn store_trade(
        &self,
        order_id: Uuid,
        exchange_trade_id: &str,
        price: Decimal,
        quantity: Decimal,
        commission: Decimal,
        commission_asset: &str,
        executed_at: DateTime<Utc>,
    ) -> Result<Uuid> {
        self.trades
            .insert(
                order_id,
                exchange_trade_id,
                price,
                quantity,
                commission,
                commission_asset,
                executed_at,
            )
            .await
    }

    /// 주문에 대한 체결 내역을 조회합니다.
    pub async fn get_order_trades(&self, order_id: Uuid) -> Result<Vec<TradeRecord>> {
        self.trades.get_by_order(order_id).await
    }

    /// 최근 체결 내역을 조회합니다.
    pub async fn get_recent_executed_trades(&self, limit: i32) -> Result<Vec<TradeRecord>> {
        self.trades.get_recent(limit).await
    }

    /// 기간별 총 수수료를 조회합니다.
    pub async fn get_total_commission(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Decimal> {
        self.trades.total_commission(start, end).await
    }

    // =========================================================================
    // 포지션 작업
    // =========================================================================

    /// 포지션을 업데이트합니다.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_position(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
        side: Side,
        quantity: Decimal,
        entry_price: Decimal,
        unrealized_pnl: Decimal,
        realized_pnl: Decimal,
    ) -> Result<Uuid> {
        self.positions
            .upsert(
                exchange,
                symbol,
                strategy_id,
                side,
                quantity,
                entry_price,
                unrealized_pnl,
                realized_pnl,
            )
            .await
    }

    /// 포지션을 조회합니다.
    pub async fn get_position(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
    ) -> Result<Option<PositionRecord>> {
        self.positions.get(exchange, symbol, strategy_id).await
    }

    /// 모든 열린 포지션을 조회합니다.
    pub async fn get_all_positions(&self) -> Result<Vec<PositionRecord>> {
        self.positions.get_all_open().await
    }

    /// 전략별 포지션을 조회합니다.
    pub async fn get_strategy_positions(&self, strategy_id: &str) -> Result<Vec<PositionRecord>> {
        self.positions.get_by_strategy(strategy_id).await
    }

    /// 포지션을 종료합니다.
    pub async fn close_position(
        &self,
        exchange: &str,
        symbol: &str,
        strategy_id: Option<&str>,
        realized_pnl: Decimal,
    ) -> Result<()> {
        self.positions
            .close(exchange, symbol, strategy_id, realized_pnl)
            .await
    }

    // =========================================================================
    // 유틸리티 작업
    // =========================================================================

    /// 심볼의 캐시를 무효화합니다.
    pub async fn invalidate_cache(&self, exchange: &str, symbol: &str) -> Result<()> {
        if let Some(cache) = &self.cache {
            let pattern = format!("*:{}:{}*", exchange, symbol);
            cache.delete_pattern(&pattern).await?;
        }
        Ok(())
    }
}

/// 데이터 매니저의 상태 정보.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub database: bool,
    pub cache: bool,
    pub overall: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DataManagerConfig::default();
        assert_eq!(config.max_cached_klines, 500);
        assert!(config.cache_enabled);
    }
}
