/**
 * 전략 연결 모달 컴포넌트
 *
 * 종목을 전략에 연결(추가)할 때 사용하는 모달입니다.
 */
import { Show, For, createResource, createSignal, onMount, onCleanup } from 'solid-js'
import { X, Link2, Play, Square, Check, AlertCircle } from 'lucide-solid'
import {
  getStrategies,
  updateStrategySymbols,
} from '../../api/client'
import type { Strategy } from '../../types'
import { useToast } from '../Toast'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('StrategyLinkModal')

interface StrategyLinkModalProps {
  /** 표시 여부 */
  isOpen: boolean
  /** 연결할 종목 티커 */
  symbol: string
  /** 닫기 핸들러 */
  onClose: () => void
  /** 연결 성공 핸들러 */
  onSuccess?: () => void
}

export function StrategyLinkModal(props: StrategyLinkModalProps) {
  const toast = useToast()
  const [loading, setLoading] = createSignal<string | null>(null)

  // 전략 목록 로드
  const [strategies, { refetch }] = createResource(
    () => props.isOpen,
    async (isOpen) => {
      if (!isOpen) return []
      try {
        const list = await getStrategies()
        // 각 전략에 symbol이 이미 포함되어 있는지 확인
        return list.map((s) => ({
          ...s,
          hasSymbol: s.symbols?.includes(props.symbol) || false,
        }))
      } catch {
        return []
      }
    }
  )

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      props.onClose()
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
  })

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown)
  })

  // 전략에 심볼 추가
  const handleLinkToStrategy = async (strategy: Strategy & { hasSymbol: boolean }) => {
    if (loading() || strategy.hasSymbol) return
    setLoading(strategy.id)

    try {
      // 기존 심볼에 새 심볼 추가
      const newSymbols = [...(strategy.symbols || []), props.symbol]
      await updateStrategySymbols(strategy.id, newSymbols)

      toast.success(`${props.symbol}이(가) "${strategy.name}"에 연결되었습니다`)
      await refetch()
      props.onSuccess?.()
    } catch (err) {
      toast.error('전략 연결 실패')
      logError(err)
    } finally {
      setLoading(null)
    }
  }

  // 전략에서 심볼 제거
  const handleUnlinkFromStrategy = async (strategy: Strategy & { hasSymbol: boolean }) => {
    if (loading() || !strategy.hasSymbol) return
    setLoading(strategy.id)

    try {
      // 기존 심볼에서 해당 심볼 제거
      const newSymbols = (strategy.symbols || []).filter((s) => s !== props.symbol)
      await updateStrategySymbols(strategy.id, newSymbols)

      toast.success(`${props.symbol}이(가) "${strategy.name}"에서 제거되었습니다`)
      await refetch()
    } catch (err) {
      toast.error('전략 연결 해제 실패')
      logError(err)
    } finally {
      setLoading(null)
    }
  }

  // 상태 배지
  const StatusBadge = (status: string) => {
    if (status === 'Running') {
      return (
        <span class="flex items-center gap-1 text-xs text-green-400 bg-green-900/30 px-2 py-0.5 rounded">
          <Play class="w-3 h-3" />
          실행중
        </span>
      )
    } else if (status === 'Error') {
      return (
        <span class="flex items-center gap-1 text-xs text-red-400 bg-red-900/30 px-2 py-0.5 rounded">
          <AlertCircle class="w-3 h-3" />
          오류
        </span>
      )
    }
    return (
      <span class="flex items-center gap-1 text-xs text-gray-400 bg-gray-700/50 px-2 py-0.5 rounded">
        <Square class="w-3 h-3" />
        중지됨
      </span>
    )
  }

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm"
        onClick={(e) => e.target === e.currentTarget && props.onClose()}
      >
        <div class="bg-gray-900 border border-gray-700 rounded-xl shadow-2xl w-full max-w-lg overflow-hidden">
          {/* 헤더 */}
          <div class="flex items-center justify-between px-5 py-4 border-b border-gray-700">
            <div class="flex items-center gap-2">
              <Link2 class="w-5 h-5 text-blue-400" />
              <h3 class="text-lg font-semibold text-white">전략 연결</h3>
            </div>
            <button
              type="button"
              onClick={props.onClose}
              class="p-1 hover:bg-gray-800 rounded-lg transition-colors"
            >
              <X class="w-5 h-5 text-gray-400" />
            </button>
          </div>

          {/* 종목 정보 */}
          <div class="px-5 py-3 bg-gray-800/50 border-b border-gray-700">
            <div class="flex items-center gap-2">
              <span class="font-mono text-white font-medium">{props.symbol}</span>
            </div>
            <p class="text-xs text-gray-400 mt-1">
              아래 전략에 이 종목을 추가하거나 제거할 수 있습니다
            </p>
          </div>

          {/* 전략 목록 */}
          <div class="p-4 max-h-96 overflow-y-auto">
            <Show
              when={!strategies.loading}
              fallback={
                <div class="text-center py-8 text-gray-400">
                  전략 목록 로딩 중...
                </div>
              }
            >
              <Show
                when={(strategies()?.length || 0) > 0}
                fallback={
                  <div class="text-center py-8 text-gray-500">
                    <Link2 class="w-8 h-8 mx-auto mb-2 opacity-50" />
                    <p>등록된 전략이 없습니다</p>
                    <p class="text-sm mt-1">전략 페이지에서 새 전략을 만들어보세요</p>
                  </div>
                }
              >
                <div class="space-y-2">
                  <For each={strategies()}>
                    {(strategy) => {
                      const isLoading = () => loading() === strategy.id
                      return (
                        <div
                          class={`w-full flex items-center justify-between px-4 py-3 rounded-lg border transition-colors ${
                            strategy.hasSymbol
                              ? 'bg-blue-900/20 border-blue-700/50'
                              : 'bg-gray-800/50 border-transparent hover:bg-gray-700/50'
                          }`}
                        >
                          <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-2">
                              <span class="text-white font-medium truncate">
                                {strategy.name}
                              </span>
                              {StatusBadge(strategy.status)}
                            </div>
                            <div class="flex items-center gap-2 mt-1 text-xs text-gray-400">
                              <span class="bg-gray-700 px-1.5 py-0.5 rounded">
                                {strategy.strategyType}
                              </span>
                              <span>{strategy.market}</span>
                              <span>•</span>
                              <span>{strategy.symbols?.length || 0}종목</span>
                            </div>
                          </div>
                          <div class="ml-3 shrink-0">
                            <Show
                              when={strategy.hasSymbol}
                              fallback={
                                <button
                                  type="button"
                                  onClick={() => handleLinkToStrategy(strategy)}
                                  disabled={isLoading()}
                                  class="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded-lg transition-colors disabled:opacity-50"
                                >
                                  {isLoading() ? '추가 중...' : '추가'}
                                </button>
                              }
                            >
                              <button
                                type="button"
                                onClick={() => handleUnlinkFromStrategy(strategy)}
                                disabled={isLoading()}
                                class="flex items-center gap-1 px-3 py-1.5 bg-gray-600 hover:bg-gray-500 text-white text-sm rounded-lg transition-colors disabled:opacity-50"
                              >
                                <Check class="w-3 h-3" />
                                {isLoading() ? '제거 중...' : '연결됨'}
                              </button>
                            </Show>
                          </div>
                        </div>
                      )
                    }}
                  </For>
                </div>
              </Show>
            </Show>
          </div>

          {/* 푸터 */}
          <div class="px-5 py-3 border-t border-gray-700 bg-gray-800/30">
            <button
              type="button"
              onClick={props.onClose}
              class="w-full px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors"
            >
              닫기
            </button>
          </div>
        </div>
      </div>
    </Show>
  )
}

export default StrategyLinkModal
