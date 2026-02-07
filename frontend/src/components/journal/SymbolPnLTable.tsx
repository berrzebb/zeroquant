/**
 * 종목별 손익 테이블 컴포넌트
 *
 * VirtualizedTable을 사용하여 대량 데이터에서도 60fps 스크롤 성능을 유지합니다.
 */
import { Show, createMemo } from 'solid-js'
import { BarChart3 } from 'lucide-solid'
import { SymbolDisplay } from '../SymbolDisplay'
import { VirtualizedTable, type VirtualColumn } from '../ui'
import { formatCurrency, formatQuantity, getPnLColor } from '../../utils/format'
import type { SymbolPnLItem } from '../../api/client'

interface SymbolPnLTableProps {
  symbols: SymbolPnLItem[]
  /** 테이블 높이 (기본: 500px) */
  height?: number
}

// 행 높이 (px) - VirtualizedTable 필수 설정
const ROW_HEIGHT = 56

export function SymbolPnLTable(props: SymbolPnLTableProps) {
  // 컬럼 정의
  const columns = createMemo((): VirtualColumn<SymbolPnLItem>[] => [
    {
      key: 'symbol',
      header: '종목',
      width: 200,
      render: (item) => (
        <SymbolDisplay
          ticker={item.symbol}
          symbolName={item.symbol_name}
          mode="full"
          size="sm"
          autoFetch={true}
        />
      ),
    },
    {
      key: 'total_trades',
      header: '거래수',
      width: 80,
      align: 'right',
      render: (item) => (
        <span class="text-gray-300">{item.total_trades}</span>
      ),
    },
    {
      key: 'total_buy_qty',
      header: '매수수량',
      width: 100,
      align: 'right',
      render: (item) => (
        <span class="text-green-400">{formatQuantity(item.total_buy_qty)}</span>
      ),
    },
    {
      key: 'total_sell_qty',
      header: '매도수량',
      width: 100,
      align: 'right',
      render: (item) => (
        <span class="text-red-400">{formatQuantity(item.total_sell_qty)}</span>
      ),
    },
    {
      key: 'total_buy_value',
      header: '매수금액',
      width: 130,
      align: 'right',
      render: (item) => (
        <span class="text-gray-300">{formatCurrency(item.total_buy_value)}</span>
      ),
    },
    {
      key: 'total_sell_value',
      header: '매도금액',
      width: 130,
      align: 'right',
      render: (item) => (
        <span class="text-gray-300">{formatCurrency(item.total_sell_value)}</span>
      ),
    },
    {
      key: 'total_fees',
      header: '수수료',
      width: 100,
      align: 'right',
      render: (item) => (
        <span class="text-gray-500">{formatCurrency(item.total_fees)}</span>
      ),
    },
    {
      key: 'realized_pnl',
      header: '실현손익',
      width: 130,
      align: 'right',
      render: (item) => (
        <span class={`font-medium ${getPnLColor(item.realized_pnl)}`}>
          {formatCurrency(item.realized_pnl)}
        </span>
      ),
    },
  ])

  // 테이블 높이 계산 (최소 200px, 기본 500px)
  const tableHeight = createMemo(() => {
    const baseHeight = props.height ?? 500
    // 데이터가 적으면 필요한 높이만 사용
    const contentHeight = props.symbols.length * ROW_HEIGHT
    return Math.min(baseHeight, Math.max(200, contentHeight))
  })

  return (
    <Show
      when={props.symbols.length > 0}
      fallback={
        <div class="py-12 text-center text-gray-500">
          <BarChart3 class="w-12 h-12 mx-auto mb-3 opacity-50" />
          <p>종목별 거래 내역이 없습니다</p>
        </div>
      }
    >
      <VirtualizedTable
        data={props.symbols}
        columns={columns()}
        rowHeight={ROW_HEIGHT}
        height={tableHeight()}
        getRowKey={(item) => item.symbol}
        emptyMessage="종목별 거래 내역이 없습니다"
        overscan={3}
      />
    </Show>
  )
}

export default SymbolPnLTable
