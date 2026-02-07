/**
 * 포지션 상세 모달 컴포넌트
 *
 * 보유 현황 테이블에서 종목을 클릭했을 때 표시되는 상세 정보 모달입니다.
 */
import { Show, onMount, onCleanup, createResource } from 'solid-js'
import { X, TrendingUp, TrendingDown, BarChart2, DollarSign, Percent, Package, Layers } from 'lucide-solid'
import type { JournalPosition, CandleData, FifoCostBasisResponse } from '../../api/client'
import { getKlines, getFifoCostBasis } from '../../api/client'
import { EChart, CHART_COLORS } from '../ui'
import { formatCurrency, formatPercent, formatQuantity, getPnLColor } from '../../utils/format'
import type { EChartsOption } from 'echarts'

interface PositionDetailModalProps {
  /** 표시 여부 */
  isOpen: boolean
  /** 포지션 데이터 */
  position: JournalPosition | null
  /** 닫기 핸들러 */
  onClose: () => void
}

export function PositionDetailModal(props: PositionDetailModalProps) {
  let modalRef: HTMLDivElement | undefined

  // 캔들 데이터 로드
  const [candleData] = createResource(
    () => props.isOpen && props.position ? props.position.symbol : null,
    async (symbol): Promise<CandleData[]> => {
      if (!symbol) return []
      try {
        const response = await getKlines({ symbol, timeframe: '1d', limit: 60 })
        return response.data || []
      } catch {
        return []
      }
    }
  )

  // FIFO 원가 데이터 로드
  const [fifoData] = createResource(
    () => props.isOpen && props.position ? { symbol: props.position.symbol, currentPrice: props.position.current_price } : null,
    async (params): Promise<FifoCostBasisResponse | null> => {
      if (!params) return null
      try {
        return await getFifoCostBasis(params.symbol, 'KR', params.currentPrice || undefined)
      } catch {
        return null
      }
    }
  )

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') props.onClose()
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
  })

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown)
  })

  // 미니 라인 차트 옵션
  const lineChartOption = (): EChartsOption => {
    const data = candleData() || []
    if (data.length === 0) return {}

    const prices = data.map((d) => d.close)
    const dates = data.map((d) => d.time.split('T')[0])

    const startPrice = prices[0] || 0
    const endPrice = prices[prices.length - 1] || 0
    const isUp = endPrice >= startPrice
    const lineColor = isUp ? CHART_COLORS.success : CHART_COLORS.danger

    return {
      grid: { left: 10, right: 10, top: 10, bottom: 20 },
      xAxis: {
        type: 'category',
        data: dates,
        show: false,
      },
      yAxis: {
        type: 'value',
        show: false,
        scale: true,
      },
      series: [
        {
          type: 'line',
          data: prices,
          smooth: true,
          symbol: 'none',
          lineStyle: { color: lineColor, width: 2 },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0, y: 0, x2: 0, y2: 1,
              colorStops: [
                { offset: 0, color: `${lineColor}40` },
                { offset: 1, color: `${lineColor}00` },
              ],
            },
          },
        },
      ],
    }
  }

  // 손익률 계산
  const pnlPercent = () => {
    if (!props.position?.unrealized_pnl_pct) return 0
    return parseFloat(props.position.unrealized_pnl_pct)
  }

  return (
    <Show when={props.isOpen && props.position}>
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
        onClick={(e) => e.target === e.currentTarget && props.onClose()}
      >
        <div
          ref={modalRef}
          class="bg-gray-900 rounded-2xl w-full max-w-lg mx-4 shadow-2xl border border-gray-700/50 overflow-hidden"
        >
          {/* 헤더 */}
          <div class="flex items-center justify-between p-5 border-b border-gray-700/50">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 rounded-xl bg-blue-500/20 flex items-center justify-center">
                <Package class="w-5 h-5 text-blue-400" />
              </div>
              <div>
                <h2 class="text-lg font-bold text-white">{props.position?.symbol}</h2>
                <p class="text-sm text-gray-400">{props.position?.symbol_name || props.position?.exchange}</p>
              </div>
            </div>
            <button
              onClick={props.onClose}
              class="p-2 rounded-lg hover:bg-gray-800 transition text-gray-400 hover:text-white"
            >
              <X class="w-5 h-5" />
            </button>
          </div>

          {/* 컨텐츠 */}
          <div class="p-5 space-y-5">
            {/* 손익 요약 */}
            <div class="grid grid-cols-2 gap-4">
              <div class="bg-gray-800/50 rounded-xl p-4">
                <div class="flex items-center gap-2 text-sm text-gray-400 mb-1">
                  <DollarSign class="w-4 h-4" />
                  평가손익
                </div>
                <div class={`text-xl font-bold ${getPnLColor(props.position?.unrealized_pnl)}`}>
                  {props.position?.unrealized_pnl ? formatCurrency(props.position.unrealized_pnl) : '-'}
                </div>
              </div>
              <div class="bg-gray-800/50 rounded-xl p-4">
                <div class="flex items-center gap-2 text-sm text-gray-400 mb-1">
                  <Percent class="w-4 h-4" />
                  수익률
                </div>
                <div class={`text-xl font-bold flex items-center gap-2 ${getPnLColor(pnlPercent())}`}>
                  {pnlPercent() >= 0 ? <TrendingUp class="w-5 h-5" /> : <TrendingDown class="w-5 h-5" />}
                  {props.position?.unrealized_pnl_pct ? formatPercent(props.position.unrealized_pnl_pct) : '-'}
                </div>
              </div>
            </div>

            {/* 미니 차트 */}
            <div class="bg-gray-800/30 rounded-xl p-4">
              <div class="text-sm text-gray-400 mb-2">최근 60일 가격 추이</div>
              <Show
                when={!candleData.loading && (candleData() || []).length > 0}
                fallback={
                  <div class="h-24 flex items-center justify-center text-gray-500 text-sm">
                    {candleData.loading ? '차트 로딩 중...' : '차트 데이터 없음'}
                  </div>
                }
              >
                <EChart option={lineChartOption()} height={100} />
              </Show>
            </div>

            {/* 상세 정보 */}
            <div class="bg-gray-800/30 rounded-xl p-4">
              <div class="flex items-center gap-2 text-sm text-gray-400 mb-3">
                <BarChart2 class="w-4 h-4" />
                상세 정보
              </div>
              <div class="grid grid-cols-2 gap-y-3 text-sm">
                <div>
                  <div class="text-gray-500">보유 수량</div>
                  <div class="text-white font-medium">{formatQuantity(props.position?.quantity || 0)}</div>
                </div>
                <div>
                  <div class="text-gray-500">평균 단가</div>
                  <div class="text-white font-medium">{formatCurrency(props.position?.entry_price || 0)}</div>
                </div>
                <div>
                  <div class="text-gray-500">현재가</div>
                  <div class="text-white font-medium">
                    {props.position?.current_price ? formatCurrency(props.position.current_price) : '-'}
                  </div>
                </div>
                <div>
                  <div class="text-gray-500">평가금액</div>
                  <div class="text-white font-medium">
                    {props.position?.market_value ? formatCurrency(props.position.market_value) : '-'}
                  </div>
                </div>
                <div>
                  <div class="text-gray-500">매입금액</div>
                  <div class="text-white font-medium">
                    {props.position?.cost_basis ? formatCurrency(props.position.cost_basis) : '-'}
                  </div>
                </div>
                <div>
                  <div class="text-gray-500">포트폴리오 비중</div>
                  <div class="text-white font-medium">
                    {props.position?.weight_pct ? `${props.position.weight_pct}%` : '-'}
                  </div>
                </div>
              </div>
            </div>

            {/* FIFO 원가 정보 */}
            <Show when={fifoData() && !fifoData.loading}>
              <div class="bg-gray-800/30 rounded-xl p-4">
                <div class="flex items-center gap-2 text-sm text-gray-400 mb-3">
                  <Layers class="w-4 h-4" />
                  FIFO 원가 분석
                </div>
                <div class="grid grid-cols-2 gap-y-3 text-sm">
                  <div>
                    <div class="text-gray-500">FIFO 평균 원가</div>
                    <div class="text-white font-medium">{formatCurrency(fifoData()!.average_cost)}</div>
                  </div>
                  <div>
                    <div class="text-gray-500">FIFO 총 비용</div>
                    <div class="text-white font-medium">{formatCurrency(fifoData()!.total_cost_basis)}</div>
                  </div>
                  <div>
                    <div class="text-gray-500">총 실현 손익</div>
                    <div class={`font-medium ${getPnLColor(fifoData()!.total_realized_pnl)}`}>
                      {formatCurrency(fifoData()!.total_realized_pnl)}
                    </div>
                  </div>
                  <div>
                    <div class="text-gray-500">총 매도 금액</div>
                    <div class="text-white font-medium">{formatCurrency(fifoData()!.total_sales)}</div>
                  </div>
                  <div>
                    <div class="text-gray-500">매수/매도 거래 수</div>
                    <div class="text-white font-medium">
                      <span class="text-green-400">{fifoData()!.buy_count}</span>
                      {' / '}
                      <span class="text-red-400">{fifoData()!.sell_count}</span>
                    </div>
                  </div>
                  <div>
                    <div class="text-gray-500">보유 로트 수</div>
                    <div class="text-white font-medium">{fifoData()!.lot_count}개</div>
                  </div>
                  <Show when={fifoData()!.unrealized_pnl}>
                    <div>
                      <div class="text-gray-500">FIFO 미실현 손익</div>
                      <div class={`font-medium ${getPnLColor(fifoData()!.unrealized_pnl!)}`}>
                        {formatCurrency(fifoData()!.unrealized_pnl!)}
                      </div>
                    </div>
                    <div>
                      <div class="text-gray-500">FIFO 손익률</div>
                      <div class={`font-medium ${getPnLColor(fifoData()!.unrealized_pnl_pct!)}`}>
                        {fifoData()!.unrealized_pnl_pct}%
                      </div>
                    </div>
                  </Show>
                </div>
              </div>
            </Show>
            <Show when={fifoData.loading}>
              <div class="bg-gray-800/30 rounded-xl p-4 text-center text-gray-500 text-sm">
                FIFO 원가 로딩 중...
              </div>
            </Show>

            {/* 거래소 정보 */}
            <div class="flex items-center justify-between text-sm text-gray-500">
              <span>거래소: {props.position?.exchange || '-'}</span>
              <Show when={props.position?.updated_at}>
                <span>업데이트: {new Date(props.position!.updated_at!).toLocaleString('ko-KR')}</span>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </Show>
  )
}

export default PositionDetailModal
