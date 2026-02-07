/**
 * 통합 전략 분석 패널 컴포넌트
 *
 * 전략 성과 테이블 + 투자 인사이트를 표시합니다.
 */
import { For, Show } from 'solid-js'
import {
  Target,
  Activity,
  TrendingUp,
  BarChart3,
  Layers,
  Calendar,
  Lightbulb,
} from 'lucide-solid'
import { formatCurrency, getPnLColor } from '../../utils/format'
import type { TradingInsightsResponse, StrategyPerformanceItem } from '../../api/client'

interface StrategyInsightsPanelProps {
  insights: TradingInsightsResponse | null
  strategies: StrategyPerformanceItem[]
}

export function StrategyInsightsPanel(props: StrategyInsightsPanelProps) {
  return (
    <div class="space-y-6">
      {/* 핵심 지표 카드 (인사이트) */}
      <Show when={props.insights}>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div class="bg-gray-700/50 rounded-lg p-4">
            <div class="flex items-center gap-2 text-gray-400 text-sm mb-2">
              <Target class="w-4 h-4" />
              손익비 (Profit Factor)
            </div>
            <div class={`text-2xl font-bold ${props.insights!.profit_factor && parseFloat(props.insights!.profit_factor!) >= 1 ? 'text-green-400' : 'text-red-400'}`}>
              {props.insights!.profit_factor || 'N/A'}
            </div>
            <div class="text-xs text-gray-500 mt-1">1.0 이상이면 양호</div>
          </div>

          <div class="bg-gray-700/50 rounded-lg p-4">
            <div class="flex items-center gap-2 text-gray-400 text-sm mb-2">
              <Activity class="w-4 h-4" />
              승률
            </div>
            <div class={`text-2xl font-bold ${parseFloat(props.insights!.win_rate_pct) >= 50 ? 'text-green-400' : 'text-red-400'}`}>
              {props.insights!.win_rate_pct}%
            </div>
            <div class="text-xs text-gray-500 mt-1">
              승 {props.insights!.winning_trades} / 패 {props.insights!.losing_trades}
            </div>
          </div>

          <div class="bg-gray-700/50 rounded-lg p-4">
            <div class="flex items-center gap-2 text-gray-400 text-sm mb-2">
              <TrendingUp class="w-4 h-4" />
              총 실현손익
            </div>
            <div class={`text-2xl font-bold ${getPnLColor(props.insights!.total_realized_pnl)}`}>
              {formatCurrency(props.insights!.total_realized_pnl)}
            </div>
            <div class="text-xs text-gray-500 mt-1">
              수수료 {formatCurrency(props.insights!.total_fees)}
            </div>
          </div>

          <div class="bg-gray-700/50 rounded-lg p-4">
            <div class="flex items-center gap-2 text-gray-400 text-sm mb-2">
              <BarChart3 class="w-4 h-4" />
              거래 통계
            </div>
            <div class="text-2xl font-bold text-white">
              {props.insights!.total_trades}회
            </div>
            <div class="text-xs text-gray-500 mt-1">
              {props.insights!.unique_symbols}개 종목
            </div>
          </div>
        </div>

        {/* 상세 분석 */}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* 승패 분석 */}
          <div class="bg-gray-700/30 rounded-lg p-5">
            <h3 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <Layers class="w-5 h-5 text-blue-400" />
              승패 분석
            </h3>
            <div class="space-y-3">
              <div class="flex justify-between">
                <span class="text-gray-400">평균 수익</span>
                <span class="text-green-400 font-medium">{formatCurrency(props.insights!.avg_win)}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">평균 손실</span>
                <span class="text-red-400 font-medium">{formatCurrency(props.insights!.avg_loss)}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">최대 수익</span>
                <span class="text-green-400 font-medium">{props.insights!.largest_win ? formatCurrency(props.insights!.largest_win!) : '-'}</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">최대 손실</span>
                <span class="text-red-400 font-medium">{props.insights!.largest_loss ? formatCurrency(props.insights!.largest_loss!) : '-'}</span>
              </div>
            </div>
          </div>

          {/* 거래 활동 */}
          <div class="bg-gray-700/30 rounded-lg p-5">
            <h3 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
              <Calendar class="w-5 h-5 text-purple-400" />
              거래 활동
            </h3>
            <div class="space-y-3">
              <div class="flex justify-between">
                <span class="text-gray-400">거래 기간</span>
                <span class="text-white font-medium">{props.insights!.trading_period_days || 0}일</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">실거래일</span>
                <span class="text-white font-medium">{props.insights!.active_trading_days}일</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">매수 거래</span>
                <span class="text-green-400 font-medium">{props.insights!.buy_trades}회</span>
              </div>
              <div class="flex justify-between">
                <span class="text-gray-400">매도 거래</span>
                <span class="text-red-400 font-medium">{props.insights!.sell_trades}회</span>
              </div>
            </div>
          </div>
        </div>

        {/* 전략 튜닝 가이드 */}
        <div class="bg-gradient-to-r from-blue-900/30 to-purple-900/30 rounded-lg p-5 border border-blue-500/20">
          <h3 class="text-lg font-semibold text-white mb-3 flex items-center gap-2">
            <Lightbulb class="w-5 h-5 text-yellow-400" />
            전략 튜닝 권장사항
          </h3>
          <div class="space-y-2 text-sm">
            <Show when={props.insights!.profit_factor && parseFloat(props.insights!.profit_factor!) < 1}>
              <p class="text-orange-400">
                ⚠️ 손익비가 1.0 미만입니다. 손절 기준을 더 엄격하게 설정하거나 익절 시점을 늦추는 것을 고려하세요.
              </p>
            </Show>
            <Show when={parseFloat(props.insights!.win_rate_pct) < 40}>
              <p class="text-orange-400">
                ⚠️ 승률이 40% 미만입니다. 진입 시그널의 정확도를 높이거나, 추세 확인 조건을 추가해보세요.
              </p>
            </Show>
            <Show when={parseFloat(props.insights!.avg_loss) > parseFloat(props.insights!.avg_win) * 2}>
              <p class="text-orange-400">
                ⚠️ 평균 손실이 평균 수익의 2배를 초과합니다. 손절 라인을 더 타이트하게 조정하세요.
              </p>
            </Show>
            <Show when={props.insights!.profit_factor && parseFloat(props.insights!.profit_factor!) >= 1.5 && parseFloat(props.insights!.win_rate_pct) >= 50}>
              <p class="text-green-400">
                ✅ 전략이 양호한 상태입니다. 현재 파라미터를 유지하거나, 포지션 크기를 점진적으로 늘려볼 수 있습니다.
              </p>
            </Show>
            <Show when={(!props.insights!.profit_factor || parseFloat(props.insights!.profit_factor!) >= 1) && parseFloat(props.insights!.win_rate_pct) >= 40 && !(props.insights!.profit_factor && parseFloat(props.insights!.profit_factor!) >= 1.5 && parseFloat(props.insights!.win_rate_pct) >= 50)}>
              <p class="text-blue-400">
                ℹ️ 전략이 안정적입니다. 더 많은 거래 데이터를 수집하여 통계적 유의성을 높이세요.
              </p>
            </Show>
          </div>
        </div>
      </Show>

      {/* 전략별 성과 테이블 */}
      <div class="bg-gray-700/30 rounded-lg p-5">
        <h3 class="text-lg font-semibold text-white mb-4 flex items-center gap-2">
          <Target class="w-5 h-5 text-green-400" />
          전략별 성과
        </h3>
        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-gray-700">
                <th class="text-left py-3 px-4 font-medium text-gray-400">전략</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">거래수</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">승률</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">손익비</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">실현손익</th>
                <th class="text-right py-3 px-4 font-medium text-gray-400">승/패</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.strategies}>
                {(strategy: StrategyPerformanceItem) => (
                  <tr class="border-b border-gray-800 hover:bg-gray-800/50">
                    <td class="py-3 px-4">
                      <div class="font-medium text-white">{strategy.strategy_name}</div>
                      <div class="text-xs text-gray-500">{strategy.strategy_id}</div>
                    </td>
                    <td class="text-right py-3 px-4 text-gray-300">{strategy.total_trades}</td>
                    <td class="text-right py-3 px-4">
                      <span class={parseFloat(strategy.win_rate_pct) >= 50 ? 'text-green-400' : 'text-red-400'}>
                        {strategy.win_rate_pct}%
                      </span>
                    </td>
                    <td class="text-right py-3 px-4">
                      <span class={strategy.profit_factor && parseFloat(strategy.profit_factor) >= 1 ? 'text-green-400' : 'text-red-400'}>
                        {strategy.profit_factor || '-'}
                      </span>
                    </td>
                    <td class={`text-right py-3 px-4 font-medium ${getPnLColor(strategy.realized_pnl)}`}>
                      {formatCurrency(strategy.realized_pnl)}
                    </td>
                    <td class="text-right py-3 px-4 text-gray-300">
                      <span class="text-green-400">{strategy.winning_trades}</span>
                      {' / '}
                      <span class="text-red-400">{strategy.losing_trades}</span>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
          <Show when={props.strategies.length === 0}>
            <div class="py-8 text-center text-gray-500">
              <Target class="w-10 h-10 mx-auto mb-2 opacity-50" />
              <p>전략별 성과 데이터가 없습니다</p>
            </div>
          </Show>
        </div>
      </div>

      {/* 인사이트 없을 때 */}
      <Show when={!props.insights}>
        <div class="py-12 text-center text-gray-500">
          <Lightbulb class="w-12 h-12 mx-auto mb-3 opacity-50" />
          <p>인사이트 데이터가 없습니다</p>
        </div>
      </Show>
    </div>
  )
}

export default StrategyInsightsPanel
