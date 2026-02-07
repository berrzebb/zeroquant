import { createSignal, createResource, For, Show, onMount } from 'solid-js'
import { A } from '@solidjs/router'
import { X, Trash2, Star, ChevronRight, Loader2, FolderPlus } from 'lucide-solid'
import {
  getWatchlists,
  getWatchlistDetail,
  createWatchlist,
  deleteWatchlist,
  removeWatchlistItem,
} from '../../api/client'

interface WatchlistViewModalProps {
  open: boolean
  onClose: () => void
  onAddClick?: () => void // 스크리닝에서 종목 추가하러 가기
}

export function WatchlistViewModal(props: WatchlistViewModalProps) {
  const [selectedGroupId, setSelectedGroupId] = createSignal<string | null>(null)
  const [showCreateForm, setShowCreateForm] = createSignal(false)
  const [newGroupName, setNewGroupName] = createSignal('')
  const [isCreating, setIsCreating] = createSignal(false)

  // 관심종목 그룹 목록
  const [watchlists, { refetch: refetchWatchlists }] = createResource(
    () => props.open,
    async (isOpen) => {
      if (!isOpen) return null
      return getWatchlists()
    }
  )

  // 선택된 그룹의 상세 정보
  const [groupDetail, { refetch: refetchDetail }] = createResource(
    () => selectedGroupId(),
    async (id) => {
      if (!id) return null
      return getWatchlistDetail(id)
    }
  )

  // 첫 번째 그룹 자동 선택
  const selectFirstGroup = () => {
    const data = watchlists()
    if (data?.watchlists?.length && !selectedGroupId()) {
      setSelectedGroupId(data.watchlists[0].id)
    }
  }

  // 모달 열릴 때 자동 선택
  onMount(() => {
    if (props.open) selectFirstGroup()
  })

  // watchlists 로드 완료 시 자동 선택
  const handleWatchlistsLoad = () => {
    if (watchlists() && !selectedGroupId()) {
      selectFirstGroup()
    }
  }

  // 그룹 생성
  const handleCreateGroup = async () => {
    if (!newGroupName().trim() || isCreating()) return
    setIsCreating(true)
    try {
      const created = await createWatchlist(newGroupName())
      setNewGroupName('')
      setShowCreateForm(false)
      await refetchWatchlists()
      setSelectedGroupId(created.id)
    } catch (e) {
      console.error('그룹 생성 실패:', e)
    } finally {
      setIsCreating(false)
    }
  }

  // 그룹 삭제
  const handleDeleteGroup = async (id: string, e: Event) => {
    e.stopPropagation()
    if (!confirm('이 그룹을 삭제하시겠습니까?')) return
    try {
      await deleteWatchlist(id)
      if (selectedGroupId() === id) {
        setSelectedGroupId(null)
      }
      await refetchWatchlists()
      selectFirstGroup()
    } catch (e) {
      console.error('그룹 삭제 실패:', e)
    }
  }

  // 종목 삭제
  const handleRemoveItem = async (symbol: string, market: string) => {
    const groupId = selectedGroupId()
    if (!groupId) return
    try {
      await removeWatchlistItem(groupId, symbol, market)
      refetchDetail()
      refetchWatchlists()
    } catch (e) {
      console.error('종목 삭제 실패:', e)
    }
  }

  return (
    <Show when={props.open}>
      <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={props.onClose}>
        <div
          class="bg-[var(--color-surface)] rounded-xl w-full max-w-4xl max-h-[80vh] flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* 헤더 */}
          <div class="flex items-center justify-between p-4 border-b border-[var(--color-surface-light)]">
            <div class="flex items-center gap-3">
              <Star class="w-5 h-5 text-yellow-400" />
              <h2 class="text-lg font-semibold text-[var(--color-text)]">관심종목</h2>
            </div>
            <button
              onClick={props.onClose}
              class="p-2 rounded-lg hover:bg-[var(--color-surface-light)] text-[var(--color-text-muted)]"
            >
              <X class="w-5 h-5" />
            </button>
          </div>

          {/* 본문 */}
          <div class="flex flex-1 min-h-0">
            {/* 왼쪽: 그룹 목록 */}
            <div class="w-56 border-r border-[var(--color-surface-light)] flex flex-col">
              <div class="p-3 border-b border-[var(--color-surface-light)] flex items-center justify-between">
                <span class="text-sm font-medium text-[var(--color-text-muted)]">그룹</span>
                <button
                  onClick={() => setShowCreateForm(true)}
                  class="p-1.5 rounded hover:bg-[var(--color-surface-light)] text-[var(--color-primary)]"
                  title="새 그룹"
                >
                  <FolderPlus class="w-4 h-4" />
                </button>
              </div>

              <div class="flex-1 overflow-auto p-2">
                {/* 그룹 생성 폼 */}
                <Show when={showCreateForm()}>
                  <div class="mb-2 p-2 bg-[var(--color-surface-light)] rounded-lg">
                    <input
                      type="text"
                      value={newGroupName()}
                      onInput={(e) => setNewGroupName(e.currentTarget.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleCreateGroup()}
                      placeholder="그룹 이름"
                      class="w-full px-2 py-1.5 text-sm bg-[var(--color-background)] text-[var(--color-text)] rounded border border-[var(--color-surface-light)] mb-2"
                      autofocus
                    />
                    <div class="flex gap-2">
                      <button
                        onClick={() => setShowCreateForm(false)}
                        class="flex-1 px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-background)] rounded"
                      >
                        취소
                      </button>
                      <button
                        onClick={handleCreateGroup}
                        disabled={!newGroupName().trim() || isCreating()}
                        class="flex-1 px-2 py-1 text-xs bg-[var(--color-primary)] text-white rounded disabled:opacity-50"
                      >
                        {isCreating() ? '...' : '생성'}
                      </button>
                    </div>
                  </div>
                </Show>

                <Show when={watchlists.loading}>
                  <div class="flex justify-center py-4">
                    <Loader2 class="w-5 h-5 animate-spin text-[var(--color-text-muted)]" />
                  </div>
                </Show>

                <Show when={!watchlists.loading}>
                  {handleWatchlistsLoad()}
                  <For each={watchlists()?.watchlists}>
                    {(group) => (
                      <div
                        class={`flex items-center gap-2 p-2.5 rounded-lg cursor-pointer transition-colors group mb-1 ${
                          selectedGroupId() === group.id
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'hover:bg-[var(--color-surface-light)] text-[var(--color-text)]'
                        }`}
                        onClick={() => setSelectedGroupId(group.id)}
                      >
                        <div
                          class="w-2.5 h-2.5 rounded-full flex-shrink-0"
                          style={{ "background-color": group.color || '#3B82F6' }}
                        />
                        <div class="flex-1 min-w-0">
                          <div class="text-sm font-medium truncate">{group.name}</div>
                          <div class={`text-xs ${selectedGroupId() === group.id ? 'text-white/70' : 'text-[var(--color-text-muted)]'}`}>
                            {group.item_count}개
                          </div>
                        </div>
                        <button
                          onClick={(e) => handleDeleteGroup(group.id, e)}
                          class={`p-1 rounded opacity-0 group-hover:opacity-100 transition-opacity ${
                            selectedGroupId() === group.id ? 'hover:bg-white/20' : 'hover:bg-red-500/20 hover:text-red-400'
                          }`}
                        >
                          <Trash2 class="w-3.5 h-3.5" />
                        </button>
                      </div>
                    )}
                  </For>
                </Show>

                <Show when={!watchlists.loading && !watchlists()?.watchlists?.length}>
                  <div class="text-center py-6 text-[var(--color-text-muted)] text-sm">
                    <Star class="w-8 h-8 mx-auto mb-2 opacity-30" />
                    <p>그룹이 없습니다</p>
                  </div>
                </Show>
              </div>
            </div>

            {/* 오른쪽: 종목 목록 */}
            <div class="flex-1 flex flex-col min-w-0">
              <Show when={selectedGroupId() && groupDetail()}>
                {/* 그룹 헤더 */}
                <div class="p-3 border-b border-[var(--color-surface-light)] flex items-center justify-between">
                  <div>
                    <h3 class="font-medium text-[var(--color-text)]">{groupDetail()?.name}</h3>
                    <p class="text-xs text-[var(--color-text-muted)]">
                      {groupDetail()?.items?.length || 0}개 종목
                    </p>
                  </div>
                </div>

                {/* 종목 테이블 */}
                <div class="flex-1 overflow-auto">
                  <Show when={groupDetail.loading}>
                    <div class="flex justify-center py-8">
                      <Loader2 class="w-6 h-6 animate-spin text-[var(--color-text-muted)]" />
                    </div>
                  </Show>

                  <Show when={!groupDetail.loading && groupDetail()?.items?.length}>
                    <table class="w-full text-sm">
                      <thead class="sticky top-0 bg-[var(--color-surface)]">
                        <tr class="border-b border-[var(--color-surface-light)]">
                          <th class="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-muted)]">종목</th>
                          <th class="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-muted)]">시장</th>
                          <th class="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-muted)]">메모</th>
                          <th class="text-left px-3 py-2 text-xs font-medium text-[var(--color-text-muted)]">추가일</th>
                          <th class="w-10"></th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={groupDetail()?.items}>
                          {(item) => (
                            <tr class="border-b border-[var(--color-surface-light)] hover:bg-[var(--color-surface-light)]/50">
                              <td class="px-3 py-2">
                                <A
                                  href={`/symbol/${item.symbol}`}
                                  onClick={props.onClose}
                                  class="font-medium text-[var(--color-text)] hover:text-[var(--color-primary)] flex items-center gap-1"
                                >
                                  {item.symbol}
                                  <ChevronRight class="w-3 h-3 text-[var(--color-text-muted)]" />
                                </A>
                              </td>
                              <td class="px-3 py-2">
                                <span class={`px-1.5 py-0.5 rounded text-xs font-medium ${
                                  item.market === 'KR' ? 'bg-blue-500/20 text-blue-400' :
                                  item.market === 'US' ? 'bg-green-500/20 text-green-400' :
                                  'bg-yellow-500/20 text-yellow-400'
                                }`}>
                                  {item.market}
                                </span>
                              </td>
                              <td class="px-3 py-2 text-[var(--color-text-muted)]">
                                {item.note || '-'}
                              </td>
                              <td class="px-3 py-2 text-[var(--color-text-muted)]">
                                {new Date(item.created_at).toLocaleDateString('ko-KR')}
                              </td>
                              <td class="px-3 py-2">
                                <button
                                  onClick={() => handleRemoveItem(item.symbol, item.market)}
                                  class="p-1 rounded hover:bg-red-500/20 text-[var(--color-text-muted)] hover:text-red-400"
                                >
                                  <Trash2 class="w-3.5 h-3.5" />
                                </button>
                              </td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </Show>

                  <Show when={!groupDetail.loading && !groupDetail()?.items?.length}>
                    <div class="flex flex-col items-center justify-center py-12 text-[var(--color-text-muted)]">
                      <Star class="w-12 h-12 mb-3 opacity-20" />
                      <p class="mb-1">종목이 없습니다</p>
                      <p class="text-xs">테이블에서 종목을 추가해보세요</p>
                    </div>
                  </Show>
                </div>
              </Show>

              <Show when={!selectedGroupId()}>
                <div class="flex-1 flex items-center justify-center text-[var(--color-text-muted)]">
                  <div class="text-center">
                    <Star class="w-12 h-12 mx-auto mb-3 opacity-20" />
                    <p>그룹을 선택하세요</p>
                  </div>
                </div>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </Show>
  )
}
