/**
 * 관심종목 그룹 선택 모달
 *
 * 종목을 관심종목 그룹에 추가할 때 사용하는 모달입니다.
 */
import { Show, For, createResource, createSignal, onMount, onCleanup } from 'solid-js'
import { X, Star, Plus, Check } from 'lucide-solid'
import {
  getWatchlists,
  addWatchlistItems,
  createWatchlist,
  findWatchlistsContainingSymbol,
  type WatchlistWithCount,
} from '../../api/client'
import { useToast } from '../Toast'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('WatchlistSelectModal')

interface WatchlistSelectModalProps {
  /** 표시 여부 */
  isOpen: boolean
  /** 추가할 종목 티커 */
  symbol: string
  /** 시장 */
  market: string
  /** 닫기 핸들러 */
  onClose: () => void
  /** 추가 성공 핸들러 */
  onSuccess?: () => void
}

export function WatchlistSelectModal(props: WatchlistSelectModalProps) {
  const toast = useToast()
  const [isCreating, setIsCreating] = createSignal(false)
  const [newGroupName, setNewGroupName] = createSignal('')
  const [loading, setLoading] = createSignal(false)

  // 관심종목 그룹 목록 로드
  const [watchlists] = createResource(
    () => props.isOpen,
    async (isOpen) => {
      if (!isOpen) return { watchlists: [], containingIds: new Set<string>() }
      try {
        const [listRes, containingRes] = await Promise.all([
          getWatchlists(),
          findWatchlistsContainingSymbol(props.symbol, props.market),
        ])
        const containingIds = new Set(containingRes.map((w) => w.id))
        return { watchlists: listRes.watchlists, containingIds }
      } catch {
        return { watchlists: [], containingIds: new Set<string>() }
      }
    }
  )

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      if (isCreating()) {
        setIsCreating(false)
      } else {
        props.onClose()
      }
    }
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
  })

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown)
  })

  // 관심종목 그룹에 추가
  const handleAddToWatchlist = async (watchlist: WatchlistWithCount) => {
    if (loading()) return
    setLoading(true)

    try {
      await addWatchlistItems(watchlist.id, [
        { symbol: props.symbol, market: props.market },
      ])
      toast.success(`${props.symbol}이(가) "${watchlist.name}"에 추가되었습니다`)
      props.onSuccess?.()
      props.onClose()
    } catch (err) {
      toast.error('관심종목 추가 실패')
      logError(err)
    } finally {
      setLoading(false)
    }
  }

  // 새 그룹 생성
  const handleCreateGroup = async () => {
    const name = newGroupName().trim()
    if (!name) {
      toast.warning('그룹 이름을 입력하세요')
      return
    }
    if (loading()) return
    setLoading(true)

    try {
      const newWatchlist = await createWatchlist(name)
      await addWatchlistItems(newWatchlist.id, [
        { symbol: props.symbol, market: props.market },
      ])
      toast.success(`"${name}" 그룹을 생성하고 ${props.symbol}을(를) 추가했습니다`)
      setNewGroupName('')
      setIsCreating(false)
      props.onSuccess?.()
      props.onClose()
    } catch (err: unknown) {
      const axiosErr = err as { response?: { status?: number } }
      if (axiosErr?.response?.status === 409) {
        toast.error('이미 존재하는 그룹 이름입니다')
      } else {
        toast.error('그룹 생성 실패')
      }
      logError(err)
    } finally {
      setLoading(false)
    }
  }

  return (
    <Show when={props.isOpen}>
      <div
        class="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm"
        onClick={(e) => e.target === e.currentTarget && props.onClose()}
      >
        <div class="bg-gray-900 border border-gray-700 rounded-xl shadow-2xl w-full max-w-md overflow-hidden">
          {/* 헤더 */}
          <div class="flex items-center justify-between px-5 py-4 border-b border-gray-700">
            <div class="flex items-center gap-2">
              <Star class="w-5 h-5 text-yellow-400" />
              <h3 class="text-lg font-semibold text-white">관심종목 추가</h3>
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
              <span class="text-xs px-1.5 py-0.5 bg-gray-700 rounded text-gray-300">
                {props.market}
              </span>
            </div>
          </div>

          {/* 그룹 목록 */}
          <div class="p-4 max-h-80 overflow-y-auto">
            <Show
              when={!watchlists.loading}
              fallback={
                <div class="text-center py-8 text-gray-400">
                  그룹 목록 로딩 중...
                </div>
              }
            >
              <Show
                when={(watchlists()?.watchlists?.length || 0) > 0}
                fallback={
                  <div class="text-center py-8 text-gray-500">
                    <Star class="w-8 h-8 mx-auto mb-2 opacity-50" />
                    <p>관심종목 그룹이 없습니다</p>
                    <p class="text-sm mt-1">새 그룹을 만들어보세요</p>
                  </div>
                }
              >
                <div class="space-y-2">
                  <For each={watchlists()?.watchlists}>
                    {(watchlist) => {
                      const isAlreadyAdded = () => watchlists()?.containingIds.has(watchlist.id)
                      return (
                        <button
                          type="button"
                          onClick={() => !isAlreadyAdded() && handleAddToWatchlist(watchlist)}
                          disabled={isAlreadyAdded() || loading()}
                          class={`w-full flex items-center justify-between px-4 py-3 rounded-lg transition-colors ${
                            isAlreadyAdded()
                              ? 'bg-green-900/20 border border-green-700/50 cursor-not-allowed'
                              : 'bg-gray-800/50 hover:bg-gray-700/50 border border-transparent'
                          }`}
                        >
                          <div class="flex items-center gap-3">
                            <div
                              class="w-3 h-3 rounded-full"
                              style={{ 'background-color': watchlist.color || '#6b7280' }}
                            />
                            <div class="text-left">
                              <div class="text-white font-medium">{watchlist.name}</div>
                              <div class="text-xs text-gray-400">{watchlist.item_count}종목</div>
                            </div>
                          </div>
                          <Show when={isAlreadyAdded()}>
                            <div class="flex items-center gap-1 text-green-400 text-sm">
                              <Check class="w-4 h-4" />
                              추가됨
                            </div>
                          </Show>
                        </button>
                      )
                    }}
                  </For>
                </div>
              </Show>
            </Show>
          </div>

          {/* 새 그룹 생성 */}
          <div class="px-4 pb-4">
            <Show
              when={isCreating()}
              fallback={
                <button
                  type="button"
                  onClick={() => setIsCreating(true)}
                  class="w-full flex items-center justify-center gap-2 px-4 py-3 bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 rounded-lg transition-colors"
                >
                  <Plus class="w-4 h-4" />
                  새 그룹 만들기
                </button>
              }
            >
              <div class="space-y-2">
                <input
                  type="text"
                  value={newGroupName()}
                  onInput={(e) => setNewGroupName(e.currentTarget.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleCreateGroup()}
                  placeholder="새 그룹 이름"
                  class="w-full px-4 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
                  autofocus
                />
                <div class="flex gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setIsCreating(false)
                      setNewGroupName('')
                    }}
                    class="flex-1 px-4 py-2 bg-gray-700 hover:bg-gray-600 text-gray-300 rounded-lg transition-colors"
                  >
                    취소
                  </button>
                  <button
                    type="button"
                    onClick={handleCreateGroup}
                    disabled={loading()}
                    class="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition-colors disabled:opacity-50"
                  >
                    {loading() ? '생성 중...' : '생성 및 추가'}
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  )
}

export default WatchlistSelectModal
