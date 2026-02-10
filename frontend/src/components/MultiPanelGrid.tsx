import { createSignal, createEffect, For, Show, onCleanup, type JSXElement } from 'solid-js'
import { Maximize2, Minimize2, X, GripVertical, Columns, Grid2x2, LayoutGrid, Square, Search, TrendingUp } from 'lucide-solid'

// ==================== 타입 ====================

export type LayoutMode = '1x1' | '1x2' | '2x1' | '2x2' | '1x3' | '3x1' | '2x3' | '3x2'

export interface PanelConfig {
  id: string
  symbol?: string
  symbolName?: string  // 심볼 이름 (티커와 함께 표시)
  timeframe?: string
  minimized?: boolean
}

export interface GridPanel {
  id: string
  row: number
  col: number
  width: number
  height: number
}

// 심볼 검색 결과 타입
export interface SymbolSearchItem {
  ticker: string
  name: string
  market?: string
}

interface MultiPanelGridProps {
  panels: PanelConfig[]
  layoutMode: LayoutMode
  renderPanel: (panel: PanelConfig, index: number) => JSXElement
  onPanelClose?: (id: string) => void
  onPanelMaximize?: (id: string) => void
  onLayoutChange?: (mode: LayoutMode) => void
  // 심볼 자동완성 관련 props
  availableSymbols?: string[]
  onSymbolChange?: (panelId: string, symbol: string, symbolName?: string) => void
  // 심볼 검색 API 콜백 (회사명 검색 지원)
  onSymbolSearch?: (query: string) => Promise<SymbolSearchItem[]>
}

// ==================== 유틸리티 ====================

const layoutConfigs: Record<LayoutMode, { cols: number; rows: number }> = {
  '1x1': { cols: 1, rows: 1 },
  '1x2': { cols: 2, rows: 1 },
  '2x1': { cols: 1, rows: 2 },
  '2x2': { cols: 2, rows: 2 },
  '1x3': { cols: 3, rows: 1 },
  '3x1': { cols: 1, rows: 3 },
  '2x3': { cols: 3, rows: 2 },
  '3x2': { cols: 2, rows: 3 },
}

const layoutIcons: Record<LayoutMode, typeof Square> = {
  '1x1': Square,
  '1x2': Columns,
  '2x1': Columns,
  '2x2': Grid2x2,
  '1x3': LayoutGrid,
  '3x1': LayoutGrid,
  '2x3': LayoutGrid,
  '3x2': LayoutGrid,
}

// ==================== 컴포넌트 ====================

export function MultiPanelGrid(props: MultiPanelGridProps) {
  const [maximizedPanel, setMaximizedPanel] = createSignal<string | null>(null)
  const [draggingPanel, setDraggingPanel] = createSignal<string | null>(null)
  const [dragOverPanel, setDragOverPanel] = createSignal<string | null>(null)

  // 심볼 자동완성 상태 (패널별)
  const [editingPanelId, setEditingPanelId] = createSignal<string | null>(null)
  const [searchQuery, setSearchQuery] = createSignal('')
  const [selectedIndex, setSelectedIndex] = createSignal(-1)
  // API 검색 결과 상태
  const [searchResults, setSearchResults] = createSignal<SymbolSearchItem[]>([])
  const [isSearching, setIsSearching] = createSignal(false)

  const config = () => layoutConfigs[props.layoutMode]
  const maxPanels = () => config().cols * config().rows

  // 검색 실행 (debounced)
  let searchTimeout: ReturnType<typeof setTimeout> | null = null

  // 컴포넌트 언마운트 시 타이머 정리
  onCleanup(() => {
    if (searchTimeout) clearTimeout(searchTimeout)
  })

  const performSearch = async (query: string) => {
    if (!query.trim()) {
      setSearchResults([])
      return
    }

    // API 검색이 있으면 사용
    if (props.onSymbolSearch) {
      setIsSearching(true)
      try {
        const results = await props.onSymbolSearch(query)
        setSearchResults(results)
      } catch {
        setSearchResults([])
      } finally {
        setIsSearching(false)
      }
    } else if (props.availableSymbols) {
      // 로컬 필터링 폴백
      const queryUpper = query.toUpperCase()
      const filtered = props.availableSymbols
        .filter(s => s.toUpperCase().includes(queryUpper))
        .slice(0, 8)
        .map(ticker => ({ ticker, name: ticker }))
      setSearchResults(filtered)
    }
  }

  // 검색어 변경 시 debounced 검색
  const handleSearchInput = (value: string) => {
    setSearchQuery(value)
    setSelectedIndex(-1)

    if (searchTimeout) clearTimeout(searchTimeout)
    searchTimeout = setTimeout(() => performSearch(value), 200)
  }

  // 자동완성 심볼 목록 (API 결과 또는 로컬 필터링)
  const filteredSymbols = () => searchResults()

  // 심볼 선택 처리
  const handleSymbolSelect = (panelId: string, symbol: string, symbolName?: string) => {
    props.onSymbolChange?.(panelId, symbol, symbolName)
    setEditingPanelId(null)
    setSearchQuery('')
    setSelectedIndex(-1)
  }

  // 키보드 네비게이션
  const handleKeyDown = (e: KeyboardEvent, panelId: string) => {
    const symbols = filteredSymbols()
    const len = symbols.length

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setSelectedIndex(prev => len > 0 ? (prev + 1) % len : -1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setSelectedIndex(prev => len > 0 ? (prev - 1 + len) % len : -1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const idx = selectedIndex()
      if (idx >= 0 && idx < len) {
        handleSymbolSelect(panelId, symbols[idx].ticker, symbols[idx].name)
      } else if (searchQuery().trim()) {
        // 검색어가 있으면 그대로 사용
        handleSymbolSelect(panelId, searchQuery().trim().toUpperCase())
      }
    } else if (e.key === 'Escape') {
      setEditingPanelId(null)
      setSearchQuery('')
      setSelectedIndex(-1)
      setSearchResults([])
    }
  }

  // 패널 순서 (드래그로 재정렬 가능)
  const [panelOrder, setPanelOrder] = createSignal<string[]>(props.panels.map(p => p.id))

  // 패널 목록이 변경되면 순서 업데이트
  createEffect(() => {
    const currentIds = props.panels.map(p => p.id)
    const order = panelOrder()

    // 새로운 패널 추가
    const newIds = currentIds.filter(id => !order.includes(id))
    // 삭제된 패널 제거
    const filteredOrder = order.filter(id => currentIds.includes(id))

    if (newIds.length > 0 || filteredOrder.length !== order.length) {
      setPanelOrder([...filteredOrder, ...newIds])
    }
  })

  // 정렬된 패널 목록
  const orderedPanels = () => {
    const order = panelOrder()
    return props.panels
      .slice()
      .sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id))
      .slice(0, maxPanels())
  }

  // 드래그 시작
  const handleDragStart = (e: DragEvent, panelId: string) => {
    if (!e.dataTransfer) return
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', panelId)
    setDraggingPanel(panelId)
  }

  // 드래그 오버
  const handleDragOver = (e: DragEvent, panelId: string) => {
    e.preventDefault()
    if (draggingPanel() && draggingPanel() !== panelId) {
      setDragOverPanel(panelId)
    }
  }

  // 드래그 종료
  const handleDragEnd = () => {
    setDraggingPanel(null)
    setDragOverPanel(null)
  }

  // 드롭 처리 (패널 순서 변경)
  const handleDrop = (e: DragEvent, targetId: string) => {
    e.preventDefault()
    const sourceId = draggingPanel()
    if (!sourceId || sourceId === targetId) {
      handleDragEnd()
      return
    }

    setPanelOrder(prev => {
      const newOrder = [...prev]
      const sourceIndex = newOrder.indexOf(sourceId)
      const targetIndex = newOrder.indexOf(targetId)

      if (sourceIndex !== -1 && targetIndex !== -1) {
        // 스왑
        newOrder[sourceIndex] = targetId
        newOrder[targetIndex] = sourceId
      }

      return newOrder
    })

    handleDragEnd()
  }

  // 최대화 토글
  const toggleMaximize = (panelId: string) => {
    setMaximizedPanel(prev => prev === panelId ? null : panelId)
    props.onPanelMaximize?.(panelId)
  }

  // 그리드 스타일 계산
  const gridStyle = () => {
    const { cols, rows } = config()
    return {
      display: 'grid',
      'grid-template-columns': `repeat(${cols}, 1fr)`,
      'grid-template-rows': `repeat(${rows}, 1fr)`,
      gap: '12px',
      height: '100%',
    }
  }

  return (
    <div class="h-full flex flex-col">
      {/* 레이아웃 선택 바 */}
      <div class="flex items-center gap-2 mb-3">
        <span class="text-sm text-[var(--color-text-muted)]">레이아웃:</span>
        <div class="flex gap-1 bg-[var(--color-surface)] rounded-lg p-1">
          <For each={['1x1', '1x2', '2x2', '2x3'] as LayoutMode[]}>
            {(mode) => {
              const Icon = layoutIcons[mode]
              return (
                <button
                  onClick={() => props.onLayoutChange?.(mode)}
                  class={`p-1.5 rounded transition ${
                    props.layoutMode === mode
                      ? 'bg-[var(--color-primary)] text-white'
                      : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)]'
                  }`}
                  title={mode}
                >
                  <Icon class="w-4 h-4" />
                </button>
              )
            }}
          </For>
        </div>
        <span class="text-xs text-[var(--color-text-muted)] ml-2">
          {orderedPanels().length}/{maxPanels()} 패널
        </span>
      </div>

      {/* 그리드 컨테이너 */}
      <div class="flex-1 min-h-0" style={maximizedPanel() ? undefined : gridStyle()}>
        <Show when={maximizedPanel()}>
          {/* 최대화된 패널 */}
          {(() => {
            const panel = props.panels.find(p => p.id === maximizedPanel())
            if (!panel) return null
            const index = props.panels.indexOf(panel)
            return (
              <div class="h-full bg-[var(--color-surface)] rounded-xl overflow-hidden flex flex-col">
                <div class="flex items-center justify-between px-3 py-2 bg-[var(--color-surface-light)] border-b border-[var(--color-bg)]">
                  <span class="text-sm font-medium text-[var(--color-text)]">
                    {panel.symbolName ? `${panel.symbol} (${panel.symbolName})` : panel.symbol || '심볼 없음'}
                  </span>
                  <div class="flex items-center gap-1">
                    <button
                      onClick={() => toggleMaximize(panel.id)}
                      class="p-1 hover:bg-[var(--color-surface)] rounded"
                    >
                      <Minimize2 class="w-4 h-4 text-[var(--color-text-muted)]" />
                    </button>
                  </div>
                </div>
                <div class="flex-1 overflow-auto p-3">
                  {props.renderPanel(panel, index)}
                </div>
              </div>
            )
          })()}
        </Show>

        <Show when={!maximizedPanel()}>
          {/* 그리드 패널들 */}
          <For each={orderedPanels()}>
            {(panel, index) => (
              <div
                class={`bg-[var(--color-surface)] rounded-xl overflow-hidden flex flex-col transition-all
                        ${draggingPanel() === panel.id ? 'opacity-50 scale-95' : ''}
                        ${dragOverPanel() === panel.id ? 'ring-2 ring-[var(--color-primary)]' : ''}`}
                onDragOver={(e) => handleDragOver(e, panel.id)}
                onDrop={(e) => handleDrop(e, panel.id)}
                onDragLeave={() => setDragOverPanel(null)}
              >
                {/* 패널 헤더 */}
                <div
                  class="flex items-center justify-between px-3 py-2 bg-[var(--color-surface-light)]
                         border-b border-[var(--color-bg)]"
                >
                  <div class="flex items-center gap-2 flex-1 min-w-0">
                    <div
                      class="cursor-move"
                      draggable={true}
                      onDragStart={(e) => handleDragStart(e, panel.id)}
                      onDragEnd={handleDragEnd}
                    >
                      <GripVertical class="w-4 h-4 text-[var(--color-text-muted)]" />
                    </div>

                    {/* 심볼 자동완성 영역 */}
                    <Show
                      when={editingPanelId() === panel.id}
                      fallback={
                        <button
                          onClick={() => {
                            setEditingPanelId(panel.id)
                            setSearchQuery(panel.symbol || '')
                            setSelectedIndex(-1)
                          }}
                          class="flex items-center gap-1.5 text-sm font-medium text-[var(--color-text)]
                                 hover:text-[var(--color-primary)] transition px-2 py-0.5 rounded
                                 hover:bg-[var(--color-surface)]"
                        >
                          <Search class="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
                          <span>{panel.symbolName ? `${panel.symbol} (${panel.symbolName})` : panel.symbol || '심볼 검색...'}</span>
                        </button>
                      }
                    >
                      <div class="relative flex-1">
                        <div class="flex items-center gap-1">
                          <Search class="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
                          <input
                            type="text"
                            value={searchQuery()}
                            onInput={(e) => handleSearchInput(e.currentTarget.value)}
                            onKeyDown={(e) => handleKeyDown(e, panel.id)}
                            onBlur={() => setTimeout(() => {
                              setEditingPanelId(null)
                              setSearchQuery('')
                              setSearchResults([])
                            }, 200)}
                            placeholder="심볼/회사명 검색..."
                            autofocus
                            class="w-full px-2 py-0.5 text-sm bg-[var(--color-bg)] text-[var(--color-text)]
                                   rounded border border-[var(--color-primary)] outline-none"
                          />
                        </div>

                        {/* 자동완성 드롭다운 */}
                        <Show when={searchQuery().trim() && (filteredSymbols().length > 0 || isSearching())}>
                          <div class="absolute top-full left-0 right-0 mt-1 bg-[var(--color-surface)]
                                      border border-[var(--color-surface-light)] rounded-lg shadow-xl z-50
                                      max-h-48 overflow-auto">
                            <For each={filteredSymbols()}>
                              {(item, idx) => (
                                <button
                                  onMouseDown={(e) => {
                                    e.preventDefault()
                                    handleSymbolSelect(panel.id, item.ticker, item.name)
                                  }}
                                  class={`w-full px-3 py-2 text-left text-sm flex items-center gap-2
                                          transition hover:bg-[var(--color-surface-light)]
                                          ${idx() === selectedIndex()
                                            ? 'bg-[var(--color-primary)]/20 text-[var(--color-primary)]'
                                            : 'text-[var(--color-text)]'}`}
                                >
                                  <TrendingUp class="w-3.5 h-3.5 text-[var(--color-primary)] flex-shrink-0" />
                                  <span class="font-mono font-medium">{item.ticker}</span>
                                  <span class="text-xs text-[var(--color-text-muted)] truncate">{item.name}</span>
                                  <Show when={item.market}>
                                    <span class="ml-auto text-xs px-1 py-0.5 rounded bg-[var(--color-bg)] text-[var(--color-text-muted)]">
                                      {item.market}
                                    </span>
                                  </Show>
                                </button>
                              )}
                            </For>
                          </div>
                        </Show>
                      </div>
                    </Show>

                    <Show when={panel.timeframe && editingPanelId() !== panel.id}>
                      <span class="text-xs px-1.5 py-0.5 bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded">
                        {panel.timeframe}
                      </span>
                    </Show>
                  </div>
                  <div class="flex items-center gap-1">
                    <button
                      onClick={() => toggleMaximize(panel.id)}
                      class="p-1 hover:bg-[var(--color-surface)] rounded"
                      title="최대화"
                    >
                      <Maximize2 class="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
                    </button>
                    <Show when={props.onPanelClose}>
                      <button
                        onClick={() => props.onPanelClose?.(panel.id)}
                        class="p-1 hover:bg-red-500/20 rounded"
                        title="닫기"
                      >
                        <X class="w-3.5 h-3.5 text-red-400" />
                      </button>
                    </Show>
                  </div>
                </div>

                {/* 패널 컨텐츠 */}
                <div class="flex-1 overflow-auto p-2 min-h-0">
                  {props.renderPanel(panel, index())}
                </div>
              </div>
            )}
          </For>

          {/* 빈 슬롯 */}
          <For each={Array(Math.max(0, maxPanels() - orderedPanels().length)).fill(null)}>
            {() => (
              <div class="bg-[var(--color-surface)]/50 rounded-xl border-2 border-dashed border-[var(--color-surface-light)]
                          flex items-center justify-center text-[var(--color-text-muted)]">
                <div class="text-center">
                  <Grid2x2 class="w-8 h-8 mx-auto mb-2 opacity-50" />
                  <p class="text-sm">빈 패널</p>
                  <p class="text-xs opacity-70">심볼을 추가하세요</p>
                </div>
              </div>
            )}
          </For>
        </Show>
      </div>
    </div>
  )
}

// ==================== 리사이즈 핸들 컴포넌트 ====================

interface ResizeHandleProps {
  direction: 'horizontal' | 'vertical' | 'both'
  onResize: (deltaX: number, deltaY: number) => void
}

export function ResizeHandle(props: ResizeHandleProps) {
  const [isDragging, setIsDragging] = createSignal(false)
  let startX = 0
  let startY = 0

  const handleMouseDown = (e: MouseEvent) => {
    e.preventDefault()
    setIsDragging(true)
    startX = e.clientX
    startY = e.clientY

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
  }

  const handleMouseMove = (e: MouseEvent) => {
    if (!isDragging()) return
    const deltaX = e.clientX - startX
    const deltaY = e.clientY - startY
    props.onResize(deltaX, deltaY)
    startX = e.clientX
    startY = e.clientY
  }

  const handleMouseUp = () => {
    setIsDragging(false)
    document.removeEventListener('mousemove', handleMouseMove)
    document.removeEventListener('mouseup', handleMouseUp)
  }

  onCleanup(() => {
    document.removeEventListener('mousemove', handleMouseMove)
    document.removeEventListener('mouseup', handleMouseUp)
  })

  const cursorClass = () => {
    switch (props.direction) {
      case 'horizontal': return 'cursor-col-resize'
      case 'vertical': return 'cursor-row-resize'
      case 'both': return 'cursor-nwse-resize'
    }
  }

  return (
    <div
      class={`absolute bg-transparent hover:bg-[var(--color-primary)]/30 transition ${cursorClass()}
              ${props.direction === 'horizontal' ? 'w-2 h-full right-0 top-0' : ''}
              ${props.direction === 'vertical' ? 'h-2 w-full bottom-0 left-0' : ''}
              ${props.direction === 'both' ? 'w-4 h-4 right-0 bottom-0' : ''}`}
      onMouseDown={handleMouseDown}
    />
  )
}
