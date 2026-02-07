/**
 * UI 공용 컴포넌트 인덱스
 *
 * 재사용 가능한 기본 UI 컴포넌트들을 export합니다.
 *
 * 사용 예:
 * ```tsx
 * import { Card, CardHeader, Button, Badge, PageHeader } from '../components/ui'
 * ```
 */

// ==================== 상태 뱃지 ====================
export { RouteStateBadge, RouteStateDot } from './RouteStateBadge'
export type { RouteStateType } from './RouteStateBadge'

// ==================== 점수 표시 ====================
export { GlobalScoreBadge, GlobalScoreBar } from './GlobalScoreBadge'
export { RadarChart } from './RadarChart'
export type { RadarChartProps, RadarChartSize } from './RadarChart'

// ==================== 카드 & 컨테이너 ====================
export { Card, CardHeader, CardContent, CardFooter } from './Card'

// ==================== 로딩 & 스켈레톤 ====================
export { Spinner, Skeleton, LoadingOverlay, PageLoader, InlineLoader } from './Loading'

// ==================== 테이블 ====================
export { DataTable, TableHeader, TableRow, TableCell } from './DataTable'
export type { Column } from './DataTable'

// ==================== 차트 유틸 ====================
export {
  ChartTooltip,
  ChartLegend,
  formatNumber,
  formatCurrency,
  formatPercent,
  getPnLColor,
  getPnLBgColor,
  chartColors,
} from './ChartUtils'

// ==================== 통계 카드 ====================
export { StatCard, StatCardGrid } from './StatCard'
export type { StatCardProps, StatCardGridProps } from './StatCard'

// ==================== 페이지 헤더 ====================
export { PageHeader } from './PageHeader'
export type { PageHeaderProps } from './PageHeader'

// ==================== 상태 표시 ====================
export { EmptyState, ErrorState, SuccessState, WarningState } from './StateDisplay'
export type {
  EmptyStateProps,
  ErrorStateProps,
  SuccessStateProps,
  WarningStateProps,
} from './StateDisplay'

// ==================== 폼 컴포넌트 ====================
export { Select, Input, FilterPanel, Button, SearchInput, DateInput, DateRangePicker } from './Form'
export type {
  SelectOption,
  SelectProps,
  InputProps,
  FilterPanelProps,
  ButtonProps,
  SearchInputProps,
  DatePreset,
  DateInputProps,
  DateRangePreset,
  DateRangePickerProps,
} from './Form'

// ==================== 뱃지 & 태그 ====================
export { Badge, StatusBadge, MarketBadge, ConfidenceBadge } from './Badge'
export type {
  BadgeProps,
  BadgeVariant,
  BadgeSize,
  StatusBadgeProps,
  StatusType,
  MarketBadgeProps,
  MarketType,
  ConfidenceBadgeProps,
  ConfidenceLevel,
} from './Badge'

// ==================== 신호 마커 ====================
// NOTE: SignalMarkerOverlay는 charts 디렉토리로 이동됨 (2026-02-06)
// 하위 호환성을 위해 re-export
export { SignalMarkerOverlay, SignalLegend } from '../charts/SignalMarkerOverlay'
export type { SignalMarkerOverlayProps, SignalLegendProps, SignalType } from '../charts/SignalMarkerOverlay'

// ==================== ECharts 범용 차트 ====================
export { EChart, CHART_COLORS, CHART_PALETTE } from './EChart'
export type { EChartProps } from './EChart'

// ==================== 트리맵 차트 (섹터맵) ====================
export { TreemapChart } from './TreemapChart'
export type { TreemapChartProps, TreemapDataItem } from './TreemapChart'

// ==================== 스트릭 뱃지 ====================
// Note: SurvivalBadge → StreakBadge로 이름 변경 (charts/SurvivalBadge와 구분)
export {
  StreakBadge,
  DualStreakBadge,
  StreakSummaryCard,
  // 하위 호환성을 위한 deprecated 별칭
  SurvivalBadge,
  DualSurvivalBadge,
} from './StreakBadge'
export type {
  StreakBadgeProps,
  DualStreakBadgeProps,
  StreakSummaryCardProps,
  StreakType,
  // 하위 호환성을 위한 deprecated 별칭
  SurvivalBadgeProps,
  DualSurvivalBadgeProps,
} from './StreakBadge'

// ==================== 순위 변동 표시 ====================
export { RankChangeIndicator } from './RankChangeIndicator'
export type { RankChangeIndicatorProps } from './RankChangeIndicator'

// ==================== 즐겨찾기 ====================
export { FavoriteButton, getFavorites, addFavorite, removeFavorite, isFavorite, toggleFavorite } from './FavoriteButton'
export type { FavoriteButtonProps } from './FavoriteButton'

// ==================== 내보내기 ====================
export { ExportButton, dataToCSV, downloadCSV } from './ExportButton'
export type { ExportButtonProps, ExportColumn } from './ExportButton'

// ==================== 자동 갱신 ====================
export { AutoRefreshToggle } from './AutoRefreshToggle'
export type { AutoRefreshToggleProps, RefreshInterval } from './AutoRefreshToggle'

// ==================== 가상화 테이블 ====================
export { VirtualizedTable } from './VirtualizedTable'
export type { VirtualizedTableProps, VirtualColumn } from './VirtualizedTable'

// ==================== 이미지 지연 로딩 ====================
export { LazyImage, NativeLazyImage } from './LazyImage'
export type { LazyImageProps } from './LazyImage'
