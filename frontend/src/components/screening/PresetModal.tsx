/**
 * 프리셋 관리 모달 컴포넌트
 *
 * 스크리닝 필터 프리셋을 저장하고 삭제할 수 있는 모달입니다.
 */
import { Show, For, createResource, createSignal, onMount, onCleanup } from 'solid-js'
import { X, Save, Trash2, Filter, AlertTriangle } from 'lucide-solid'
import {
  getScreeningPresetsDetail,
  saveScreeningPreset,
  deleteScreeningPreset,
  type ScreeningPresetDetail,
} from '../../api/client'
import { useToast } from '../Toast'
import { createLogger } from '../../utils/logger'

const { error: logError } = createLogger('PresetModal')

interface PresetModalProps {
  /** 표시 여부 */
  isOpen: boolean
  /** 현재 필터 상태 (저장할 때 사용) */
  currentFilters: Record<string, unknown>
  /** 닫기 핸들러 */
  onClose: () => void
  /** 저장/삭제 성공 시 콜백 */
  onSuccess?: () => void
}

type ModalView = 'list' | 'save' | 'delete'

export function PresetModal(props: PresetModalProps) {
  const toast = useToast()
  const [view, setView] = createSignal<ModalView>('list')
  const [loading, setLoading] = createSignal(false)
  const [newPresetName, setNewPresetName] = createSignal('')
  const [newPresetDescription, setNewPresetDescription] = createSignal('')
  const [deleteTarget, setDeleteTarget] = createSignal<ScreeningPresetDetail | null>(null)

  // 프리셋 목록 로드
  const [presets, { refetch }] = createResource(
    () => props.isOpen,
    async (isOpen) => {
      if (!isOpen) return []
      try {
        const response = await getScreeningPresetsDetail()
        return response.presets || []
      } catch {
        return []
      }
    }
  )

  // ESC 키로 닫기
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      if (view() !== 'list') {
        setView('list')
        setDeleteTarget(null)
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

  // 프리셋 저장
  const handleSavePreset = async () => {
    const name = newPresetName().trim()
    if (!name) {
      toast.warning('프리셋 이름을 입력하세요')
      return
    }
    if (loading()) return
    setLoading(true)

    try {
      await saveScreeningPreset({
        name,
        description: newPresetDescription().trim() || undefined,
        filters: props.currentFilters,
      })
      toast.success(`프리셋 "${name}"이(가) 저장되었습니다`)
      setNewPresetName('')
      setNewPresetDescription('')
      setView('list')
      await refetch()
      props.onSuccess?.()
    } catch (err: any) {
      if (err?.response?.status === 409) {
        toast.error('이미 존재하는 프리셋 이름입니다')
      } else {
        toast.error('프리셋 저장 실패')
      }
      logError(err)
    } finally {
      setLoading(false)
    }
  }

  // 프리셋 삭제
  const handleDeletePreset = async () => {
    const target = deleteTarget()
    if (!target || loading()) return
    setLoading(true)

    try {
      await deleteScreeningPreset(target.id)
      toast.success(`프리셋 "${target.name}"이(가) 삭제되었습니다`)
      setDeleteTarget(null)
      setView('list')
      await refetch()
      props.onSuccess?.()
    } catch (err: any) {
      if (err?.response?.status === 400) {
        toast.error('기본 프리셋은 삭제할 수 없습니다')
      } else {
        toast.error('프리셋 삭제 실패')
      }
      logError(err)
    } finally {
      setLoading(false)
    }
  }

  // 삭제 확인 시작
  const startDelete = (preset: ScreeningPresetDetail) => {
    if (preset.is_default) {
      toast.warning('기본 프리셋은 삭제할 수 없습니다')
      return
    }
    setDeleteTarget(preset)
    setView('delete')
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
              <Filter class="w-5 h-5 text-purple-400" />
              <h3 class="text-lg font-semibold text-white">
                {view() === 'save' ? '프리셋 저장' : view() === 'delete' ? '프리셋 삭제' : '프리셋 관리'}
              </h3>
            </div>
            <button
              type="button"
              onClick={props.onClose}
              class="p-1 hover:bg-gray-800 rounded-lg transition-colors"
            >
              <X class="w-5 h-5 text-gray-400" />
            </button>
          </div>

          {/* 목록 뷰 */}
          <Show when={view() === 'list'}>
            <div class="p-4">
              {/* 새 프리셋 저장 버튼 */}
              <button
                type="button"
                onClick={() => setView('save')}
                class="w-full flex items-center justify-center gap-2 px-4 py-3 mb-4 bg-purple-600/20 hover:bg-purple-600/30 text-purple-400 rounded-lg transition-colors"
              >
                <Save class="w-4 h-4" />
                현재 필터를 프리셋으로 저장
              </button>

              {/* 프리셋 목록 */}
              <div class="max-h-80 overflow-y-auto">
                <Show
                  when={!presets.loading}
                  fallback={
                    <div class="text-center py-8 text-gray-400">
                      프리셋 목록 로딩 중...
                    </div>
                  }
                >
                  <Show
                    when={(presets()?.length || 0) > 0}
                    fallback={
                      <div class="text-center py-8 text-gray-500">
                        <Filter class="w-8 h-8 mx-auto mb-2 opacity-50" />
                        <p>저장된 프리셋이 없습니다</p>
                      </div>
                    }
                  >
                    <div class="space-y-2">
                      <For each={presets()}>
                        {(preset) => (
                          <div class="flex items-center justify-between px-4 py-3 bg-gray-800/50 rounded-lg">
                            <div class="flex-1 min-w-0">
                              <div class="flex items-center gap-2">
                                <span class="text-white font-medium truncate">
                                  {preset.name}
                                </span>
                                <Show when={preset.is_default}>
                                  <span class="text-xs px-1.5 py-0.5 bg-blue-900/50 text-blue-400 rounded">
                                    기본
                                  </span>
                                </Show>
                              </div>
                              <Show when={preset.description}>
                                <p class="text-xs text-gray-400 mt-0.5 truncate">
                                  {preset.description}
                                </p>
                              </Show>
                            </div>
                            <Show when={!preset.is_default}>
                              <button
                                type="button"
                                onClick={() => startDelete(preset)}
                                class="ml-2 p-2 text-gray-400 hover:text-red-400 hover:bg-red-900/20 rounded-lg transition-colors"
                              >
                                <Trash2 class="w-4 h-4" />
                              </button>
                            </Show>
                          </div>
                        )}
                      </For>
                    </div>
                  </Show>
                </Show>
              </div>
            </div>
          </Show>

          {/* 저장 뷰 */}
          <Show when={view() === 'save'}>
            <div class="p-4 space-y-4">
              <div>
                <label class="block text-sm text-gray-400 mb-1">프리셋 이름 *</label>
                <input
                  type="text"
                  value={newPresetName()}
                  onInput={(e) => setNewPresetName(e.currentTarget.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleSavePreset()}
                  placeholder="예: 저PER 고ROE 종목"
                  class="w-full px-4 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:border-purple-500"
                  autofocus
                />
              </div>
              <div>
                <label class="block text-sm text-gray-400 mb-1">설명 (선택)</label>
                <input
                  type="text"
                  value={newPresetDescription()}
                  onInput={(e) => setNewPresetDescription(e.currentTarget.value)}
                  placeholder="이 프리셋에 대한 간단한 설명"
                  class="w-full px-4 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white placeholder-gray-500 focus:outline-none focus:border-purple-500"
                />
              </div>
              <div class="flex gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => {
                    setView('list')
                    setNewPresetName('')
                    setNewPresetDescription('')
                  }}
                  class="flex-1 px-4 py-2 bg-gray-700 hover:bg-gray-600 text-gray-300 rounded-lg transition-colors"
                >
                  취소
                </button>
                <button
                  type="button"
                  onClick={handleSavePreset}
                  disabled={loading()}
                  class="flex-1 px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition-colors disabled:opacity-50"
                >
                  {loading() ? '저장 중...' : '저장'}
                </button>
              </div>
            </div>
          </Show>

          {/* 삭제 확인 뷰 */}
          <Show when={view() === 'delete'}>
            <div class="p-4 space-y-4">
              <div class="flex items-center gap-3 p-4 bg-red-900/20 border border-red-700/50 rounded-lg">
                <AlertTriangle class="w-6 h-6 text-red-400 shrink-0" />
                <div>
                  <p class="text-white font-medium">프리셋을 삭제하시겠습니까?</p>
                  <p class="text-sm text-gray-400 mt-1">
                    "{deleteTarget()?.name}" 프리셋이 영구적으로 삭제됩니다.
                  </p>
                </div>
              </div>
              <div class="flex gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setView('list')
                    setDeleteTarget(null)
                  }}
                  class="flex-1 px-4 py-2 bg-gray-700 hover:bg-gray-600 text-gray-300 rounded-lg transition-colors"
                >
                  취소
                </button>
                <button
                  type="button"
                  onClick={handleDeletePreset}
                  disabled={loading()}
                  class="flex-1 px-4 py-2 bg-red-600 hover:bg-red-500 text-white rounded-lg transition-colors disabled:opacity-50"
                >
                  {loading() ? '삭제 중...' : '삭제'}
                </button>
              </div>
            </div>
          </Show>

          {/* 푸터 (목록 뷰에서만) */}
          <Show when={view() === 'list'}>
            <div class="px-5 py-3 border-t border-gray-700 bg-gray-800/30">
              <button
                type="button"
                onClick={props.onClose}
                class="w-full px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded-lg transition-colors"
              >
                닫기
              </button>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  )
}

export default PresetModal
