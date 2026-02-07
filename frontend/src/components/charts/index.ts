// ==================== 통합 차트 (BaseChart) ====================
export {
  BaseChart,
  AreaChart,
  LineChart,
  HistogramChart,
  CandlestickChart,
} from './BaseChart'
export type { BaseChartProps, BaseDataPoint, ChartType } from './BaseChart'

// ==================== 기존 차트 컴포넌트 ====================
export { PriceChart } from './PriceChart'
export type { CandlestickDataPoint, LineDataPoint, IndicatorOverlay, TradeMarker, TradeMarkerType } from './PriceChart'

export { SubPriceChart } from './SubPriceChart'
export type { SeparateIndicatorData, IndicatorSeriesData } from './SubPriceChart'

export { EquityCurve } from './EquityCurve'
export type { EquityDataPoint, ChartSyncState } from './EquityCurve'

export { DrawdownChart } from './DrawdownChart'
export type { DrawdownDataPoint } from './DrawdownChart'

export { MetricsChart } from './MetricsChart'
export type { MetricDataPoint, MetricSeries } from './MetricsChart'

export { PortfolioEquityChart } from './PortfolioEquityChart'
export { AnalyticsDashboard } from './AnalyticsDashboard'
export { SyncedChartPanel } from './SyncedChartPanel'

export { IndicatorFilterPanel } from './IndicatorFilterPanel'
export type { IndicatorFilters, IndicatorFilterPanelProps, FilterPreset } from './IndicatorFilterPanel'

export { FearGreedGauge, getLevel, getLevelInfo } from './FearGreedGauge'
export type { FearGreedGaugeProps, FearGreedLevel } from './FearGreedGauge'

export { MarketBreadthWidget } from './MarketBreadthWidget'
export type { MarketBreadthWidgetProps } from './MarketBreadthWidget'

export { ScoreWaterfall, Factor7Waterfall } from './ScoreWaterfall'
export type { ScoreWaterfallProps, WaterfallDataItem, Factor7WaterfallProps } from './ScoreWaterfall'

export { SectorTreemap, SectorSummaryCard } from './SectorTreemap'
export type { SectorTreemapProps, SectorDataItem, SectorMetric, SectorSummaryCardProps } from './SectorTreemap'

export { KellyVisualization } from './KellyVisualization'
export type { KellyVisualizationProps } from './KellyVisualization'

export { CorrelationHeatmap, MiniCorrelationMatrix } from './CorrelationHeatmap'
export type { CorrelationHeatmapProps, MiniCorrelationMatrixProps } from './CorrelationHeatmap'

export { OpportunityMap } from './OpportunityMap'
export type { OpportunityMapProps, OpportunitySymbol } from './OpportunityMap'

export { KanbanBoard } from './KanbanBoard'
export type { KanbanBoardProps, KanbanSymbol } from './KanbanBoard'

export { RegimeSummaryTable } from './RegimeSummaryTable'
export type { RegimeSummaryTableProps, RegimeData, RegimeTransition, MarketRegime } from './RegimeSummaryTable'

export { SectorMomentumBar } from './SectorMomentumBar'
export type { SectorMomentumBarProps, SectorMomentum } from './SectorMomentumBar'

export { VolumeProfile, VolumeProfileLegend } from './VolumeProfile'
export type { VolumeProfileProps, PriceVolume, VolumeProfileLegendProps } from './VolumeProfile'

export { SurvivalBadge, SurvivalProgress, TierLegend, getTierFromDays, getDaysToNextTier } from './SurvivalBadge'
export type { SurvivalBadgeProps, SurvivalProgressProps, TierLegendProps, SurvivalTier } from './SurvivalBadge'

export { TradeConnectionOverlay, convertBacktestTrades } from './TradeConnectionOverlay'
export type { TradeConnectionOverlayProps, TradeConnection } from './TradeConnectionOverlay'

export { SignalCorrelationChart } from './SignalCorrelationChart'
export type { SignalCorrelationChartProps, SignalDataPoint } from './SignalCorrelationChart'

export { MultiTimeframeChart } from './MultiTimeframeChart'
export type { MultiTimeframeChartProps, TimeframeData, LayoutMode } from './MultiTimeframeChart'

export { ScoreHistoryChart } from './ScoreHistoryChart'
export type { ScoreHistoryChartProps } from './ScoreHistoryChart'

export { SignalMarkerOverlay, SignalLegend } from './SignalMarkerOverlay'
export type { SignalMarkerOverlayProps, SignalLegendProps, SignalType } from './SignalMarkerOverlay'

export { SignalDetailPopup } from './SignalDetailPopup'
export type { SignalDetailPopupProps } from './SignalDetailPopup'
