/**
 * 보유 현황 테이블 컴포넌트
 *
 * DataTable 컴포넌트를 사용하여 정렬과 행 클릭 기능을 지원합니다.
 */
import { Show } from 'solid-js'
import { SymbolDisplay } from '../SymbolDisplay'
import { DataTable, EmptyState, type Column } from '../ui'
import { formatCurrency, formatPercent, formatQuantity, getPnLColor } from '../../utils/format'
import type { JournalPosition } from '../../api/client'

interface PositionsTableProps {
  positions: JournalPosition[]
  /** 행 클릭 핸들러 (상세 모달용) */
  onRowClick?: (position: JournalPosition) => void
}

export function PositionsTable(props: PositionsTableProps) {
  // 컬럼 정의 (정렬 가능)
  const columns: Column<JournalPosition>[] = [
    {
      key: 'symbol',
      header: '종목',
      sortable: true,
      render: (_, row) => (
        <SymbolDisplay
          ticker={row.symbol}
          symbolName={row.symbol_name}
          exchange={row.exchange}
          mode="full"
          size="sm"
          autoFetch={false}
        />
      ),
    },
    {
      key: 'quantity',
      header: '수량',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class="text-gray-300">{formatQuantity(value)}</span>
      ),
    },
    {
      key: 'entry_price',
      header: '평균단가',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class="text-gray-300">{formatCurrency(value)}</span>
      ),
    },
    {
      key: 'current_price',
      header: '현재가',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class="text-gray-300">{value ? formatCurrency(value) : '-'}</span>
      ),
    },
    {
      key: 'market_value',
      header: '평가금액',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class="text-gray-300">{value ? formatCurrency(value) : '-'}</span>
      ),
    },
    {
      key: 'unrealized_pnl',
      header: '손익',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class={`font-medium ${getPnLColor(value)}`}>
          {value ? formatCurrency(value) : '-'}
        </span>
      ),
    },
    {
      key: 'unrealized_pnl_pct',
      header: '수익률',
      align: 'right',
      sortable: true,
      render: (value) => (
        <span class={`font-medium ${getPnLColor(value)}`}>
          {value ? formatPercent(value) : '-'}
        </span>
      ),
    },
    {
      key: 'weight_pct',
      header: '비중',
      align: 'right',
      sortable: true,
      render: (value) => (
        <div class="flex items-center justify-end gap-2">
          {/* 비중 막대 표시 */}
          <Show when={value}>
            <div class="w-16 h-1.5 bg-gray-700 rounded-full overflow-hidden">
              <div
                class="h-full bg-blue-500 rounded-full"
                style={{ width: `${Math.min(parseFloat(value || '0'), 100)}%` }}
              />
            </div>
          </Show>
          <span class="text-gray-400 min-w-[3rem] text-right">
            {value ? `${value}%` : '-'}
          </span>
        </div>
      ),
    },
  ]

  return (
    <Show
      when={props.positions.length > 0}
      fallback={
        <EmptyState
          icon="📭"
          title="보유 포지션이 없습니다"
          description="현재 보유 중인 종목이 없습니다."
        />
      }
    >
      <DataTable
        data={props.positions}
        columns={columns}
        keyField="symbol"
        hover
        onRowClick={props.onRowClick}
        sortField="market_value"
        sortOrder="desc"
        emptyMessage="보유 포지션이 없습니다"
      />
    </Show>
  )
}

export default PositionsTable
