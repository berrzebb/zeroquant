/**
 * 전략 추가 모달 (SDUI 기반)
 *
 * 새로운 SDUI API를 사용하여 전략 목록과 설정 폼을 렌더링합니다.
 */
import { createSignal, createResource, For, Show } from 'solid-js'
import { X, ChevronRight, Search, RefreshCw, AlertCircle, Clock } from 'lucide-solid'
import { createStrategy, listCredentials } from '../api/client'
import type { StrategyMetaItem, MultiTimeframeConfig, Timeframe, ExchangeCredential } from '../api/client'
import { SDUIRenderer } from './strategy/SDUIRenderer/SDUIRenderer'
import { useToast } from './Toast'
import { MultiTimeframeSelector } from './strategy/MultiTimeframeSelector'
import { createLogger } from '../utils/logger'

const { log, error: logError } = createLogger('AddStrategyModal')

// ==================== Props ====================

export interface AddStrategyModalProps {
  open: boolean
  onClose: () => void
  onSuccess: () => void
  /** 전략 메타데이터 목록 (SDUI API에서 가져온 데이터) */
  templates: StrategyMetaItem[]
  templatesLoading?: boolean
}

// ==================== 컴포넌트 ====================

export function AddStrategyModal(props: AddStrategyModalProps) {
  const toast = useToast()

  // 모달 상태
  const [modalStep, setModalStep] = createSignal<'select' | 'configure'>('select')
  const [selectedStrategy, setSelectedStrategy] = createSignal<StrategyMetaItem | null>(null)
  const [customName, setCustomName] = createSignal('')
  const [searchQuery, setSearchQuery] = createSignal('')
  const [selectedCategory, setSelectedCategory] = createSignal<string | null>(null)

  // SDUI 폼 값 (SDUIRenderer에서 전달받음)
  const [, setFormValues] = createSignal<Record<string, unknown>>({})

  // 다중 타임프레임 설정 상태
  const [multiTfConfig, setMultiTfConfig] = createSignal<MultiTimeframeConfig | null>(null)
  const [enableMultiTf, setEnableMultiTf] = createSignal(false)

  // 생성 상태
  const [isCreating, setIsCreating] = createSignal(false)
  const [createError, setCreateError] = createSignal<string | null>(null)

  // 거래소 계정 목록 로드
  const [credentials] = createResource(async () => {
    try {
      const response = await listCredentials()
      return response.credentials || []
    } catch {
      return [] as ExchangeCredential[]
    }
  })

  // 선택된 계정 (기본: 활성 계정)
  const [selectedCredentialId, setSelectedCredentialId] = createSignal<string | null>(null)
  // 선택된 시장 (KR, US, CRYPTO)
  const [selectedMarket, setSelectedMarket] = createSignal<string | null>(null)

  // 활성 계정 ID 찾기
  const activeCredential = () => credentials()?.find(c => c.is_active)

  // 거래소별 지원 시장 목록
  const getSupportedMarkets = (exchangeId: string): { value: string; label: string }[] => {
    switch (exchangeId) {
      case 'kis':
        return [
          { value: 'KR', label: '한국 주식' },
          { value: 'US', label: '미국 주식' },
        ]
      case 'binance':
      case 'coinbase':
        return [{ value: 'CRYPTO', label: '암호화폐' }]
      case 'mock':
        return [
          { value: 'KR', label: '한국 주식' },
          { value: 'US', label: '미국 주식' },
          { value: 'CRYPTO', label: '암호화폐' },
        ]
      default:
        return []
    }
  }

  // 현재 선택된 계정의 지원 시장
  const currentCredential = () => {
    const id = selectedCredentialId() || activeCredential()?.id
    return credentials()?.find(c => c.id === id)
  }
  const supportedMarkets = () => {
    const cred = currentCredential()
    return cred ? getSupportedMarkets(cred.exchange_id) : []
  }

  // 계정 변경 시 시장 자동 선택 (단일 옵션이면 자동 선택)
  const handleCredentialChange = (credId: string | null) => {
    setSelectedCredentialId(credId)
    const cred = credentials()?.find(c => c.id === credId)
    if (cred) {
      const markets = getSupportedMarkets(cred.exchange_id)
      if (markets.length === 1) {
        setSelectedMarket(markets[0].value)
      } else {
        setSelectedMarket(null)
      }
    }
  }

  // 카테고리 목록
  const categories = () => {
    const cats = new Set<string>()
    props.templates?.forEach(s => {
      if (s.category) cats.add(s.category)
    })
    return Array.from(cats)
  }

  // 필터링된 템플릿
  const filteredTemplates = () => {
    let templates = props.templates || []

    // 카테고리 필터
    if (selectedCategory()) {
      templates = templates.filter(s => s.category === selectedCategory())
    }

    // 검색 필터
    const query = searchQuery().toLowerCase()
    if (query) {
      templates = templates.filter(s =>
        s.name.toLowerCase().includes(query) ||
        s.description.toLowerCase().includes(query)
      )
    }

    return templates
  }

  // 전략 선택
  const selectStrategy = (template: StrategyMetaItem) => {
    setSelectedStrategy(template)
    setCustomName(template.name)
    setFormValues({})

    // 다중 타임프레임 설정 초기화
    if (template.isMultiTimeframe && template.secondaryTimeframes.length > 0) {
      setMultiTfConfig({
        primary: template.defaultTimeframe as Timeframe,
        secondary: template.secondaryTimeframes.map(tf => ({
          timeframe: tf as Timeframe,
          candle_count: 100,
        })),
      })
      setEnableMultiTf(true)
    } else {
      setMultiTfConfig(null)
      setEnableMultiTf(false)
    }

    setModalStep('configure')
  }

  // 폼 값 변경 핸들러 (SDUIRenderer에서 호출)
  const handleFormChange = (values: Record<string, unknown>) => {
    setFormValues(values)
  }

  // 전략 생성 (SDUIRenderer의 onSubmit에서 호출)
  const handleCreateStrategy = async (values: Record<string, unknown>) => {
    const template = selectedStrategy()
    if (!template) return

    setIsCreating(true)
    setCreateError(null)

    try {
      // 시장 결정 (선택된 값 또는 기본값)
      const market = selectedMarket() || supportedMarkets()[0]?.value || undefined

      const response = await createStrategy({
        strategy_type: template.id,
        name: customName() || template.name,
        parameters: values,
        // 다중 타임프레임 설정 (활성화된 경우만)
        multiTimeframeConfig: enableMultiTf() && multiTfConfig() ? multiTfConfig()! : undefined,
        // 선택된 계정 (없으면 활성 계정 사용)
        credentialId: selectedCredentialId() || activeCredential()?.id || null,
        // 대상 시장
        market,
      })

      log('Strategy created:', response)

      // 모달 닫기 및 상태 초기화
      closeModal()
      // 부모에게 전략 목록 새로고침 알림
      props.onSuccess()
      // 성공 토스트
      toast.success('전략 생성 완료', `"${customName() || template.name}" 전략이 생성되었습니다`)
    } catch (error) {
      logError('Failed to create strategy:', error)
      const errorMsg = error instanceof Error ? error.message : '전략 생성에 실패했습니다'
      setCreateError(errorMsg)
      toast.error('전략 생성 실패', errorMsg)
    } finally {
      setIsCreating(false)
    }
  }

  // 모달 닫기
  const closeModal = () => {
    props.onClose()
    // 상태 초기화
    setModalStep('select')
    setSelectedStrategy(null)
    setFormValues({})
    setCustomName('')
    setSearchQuery('')
    setSelectedCategory(null)
    setCreateError(null)
    setMultiTfConfig(null)
    setEnableMultiTf(false)
    setSelectedCredentialId(null)
    setSelectedMarket(null)
  }

  // 선택 단계로 돌아가기
  const goBack = () => {
    setModalStep('select')
    setSelectedStrategy(null)
    setFormValues({})
    setCustomName('')
    setMultiTfConfig(null)
    setEnableMultiTf(false)
  }

  return (
    <Show when={props.open}>
      <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
        {/* 배경 오버레이 */}
        <div
          class="absolute inset-0 bg-black/50"
          onClick={closeModal}
        />

        {/* 모달 컨텐츠 */}
        <div class="relative w-full max-w-4xl max-h-[90vh] bg-[var(--color-bg)] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
          {/* 헤더 */}
          <div class="flex items-center justify-between p-6 border-b border-[var(--color-surface-light)]">
            <div class="flex items-center gap-3">
              <Show when={modalStep() === 'configure'}>
                <button
                  onClick={goBack}
                  class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
                >
                  <ChevronRight class="w-5 h-5 rotate-180" />
                </button>
              </Show>
              <div>
                <h2 class="text-xl font-semibold text-[var(--color-text)]">
                  {modalStep() === 'select' ? '전략 선택' : selectedStrategy()?.name}
                </h2>
                <p class="text-sm text-[var(--color-text-muted)]">
                  {modalStep() === 'select'
                    ? '자동 매매에 사용할 전략을 선택하세요'
                    : '전략 파라미터를 설정하세요'}
                </p>
              </div>
            </div>
            <button
              onClick={closeModal}
              class="p-2 hover:bg-[var(--color-surface)] rounded-lg transition-colors"
            >
              <X class="w-5 h-5" />
            </button>
          </div>

          {/* 본문 */}
          <div class="flex-1 overflow-y-auto">
            {/* 1단계: 전략 선택 */}
            <Show when={modalStep() === 'select'}>
              <div class="p-6 space-y-6">
                {/* 검색 및 필터 */}
                <div class="flex gap-4">
                  {/* 검색 */}
                  <div class="flex-1 relative">
                    <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--color-text-muted)]" />
                    <input
                      type="text"
                      value={searchQuery()}
                      onInput={(e) => setSearchQuery(e.currentTarget.value)}
                      placeholder="전략 검색..."
                      class="w-full pl-10 pr-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                    />
                  </div>
                </div>

                {/* 카테고리 필터 */}
                <div class="flex flex-wrap gap-2">
                  <button
                    class={`px-3 py-1.5 rounded-lg text-sm transition-colors ${
                      selectedCategory() === null
                        ? 'bg-[var(--color-primary)] text-white'
                        : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                    }`}
                    onClick={() => setSelectedCategory(null)}
                  >
                    전체
                  </button>
                  <For each={categories()}>
                    {(category) => (
                      <button
                        class={`px-3 py-1.5 rounded-lg text-sm transition-colors ${
                          selectedCategory() === category
                            ? 'bg-[var(--color-primary)] text-white'
                            : 'bg-[var(--color-surface)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]'
                        }`}
                        onClick={() => setSelectedCategory(category)}
                      >
                        {category}
                      </button>
                    )}
                  </For>
                </div>

                {/* 전략 목록 */}
                <Show
                  when={!props.templatesLoading}
                  fallback={
                    <div class="flex items-center justify-center py-12">
                      <RefreshCw class="w-8 h-8 animate-spin text-[var(--color-primary)]" />
                    </div>
                  }
                >
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <For each={filteredTemplates()}>
                      {(template) => (
                        <button
                          class="text-left p-4 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-xl hover:border-[var(--color-primary)] hover:bg-[var(--color-surface-light)] transition-all group"
                          onClick={() => selectStrategy(template)}
                        >
                          <div class="flex items-start justify-between mb-2">
                            <h3 class="font-semibold text-[var(--color-text)] group-hover:text-[var(--color-primary)]">
                              {template.name}
                            </h3>
                            <div class="flex gap-1">
                              <Show when={template.isMultiTimeframe}>
                                <span class="px-2 py-0.5 text-xs bg-purple-500/20 text-purple-400 rounded">
                                  Multi-TF
                                </span>
                              </Show>
                              <Show when={template.category}>
                                <span class="px-2 py-0.5 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded">
                                  {template.category}
                                </span>
                              </Show>
                            </div>
                          </div>
                          <p class="text-sm text-[var(--color-text-muted)] mb-3 line-clamp-2">
                            {template.description}
                          </p>
                          <div class="flex flex-wrap gap-1">
                            <span class="px-2 py-0.5 text-xs bg-[var(--color-bg)] text-[var(--color-text-muted)] rounded">
                              기본 TF: {template.defaultTimeframe}
                            </span>
                            <Show when={template.supportedMarkets.length > 0}>
                              <span class="px-2 py-0.5 text-xs bg-[var(--color-bg)] text-[var(--color-text-muted)] rounded">
                                {template.supportedMarkets.join(', ')}
                              </span>
                            </Show>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                </Show>

                {/* 검색 결과 없음 */}
                <Show when={filteredTemplates().length === 0 && !props.templatesLoading}>
                  <div class="text-center py-12 text-[var(--color-text-muted)]">
                    <p class="mb-2">검색 결과가 없습니다</p>
                    <p class="text-sm">다른 검색어를 시도해보세요</p>
                  </div>
                </Show>
              </div>
            </Show>

            {/* 2단계: 파라미터 설정 (SDUI 기반) */}
            <Show when={modalStep() === 'configure' && selectedStrategy()}>
              <div class="p-6 space-y-6">
                {/* 전략 정보 카드 */}
                <div class="p-4 bg-[var(--color-surface)] rounded-lg space-y-3">
                  {/* 카테고리 배지 */}
                  <div class="flex items-center gap-2">
                    <Show when={selectedStrategy()?.isMultiTimeframe}>
                      <span class="px-2 py-1 text-xs bg-purple-500/20 text-purple-400 rounded-lg font-medium">
                        Multi-TF
                      </span>
                    </Show>
                    <Show when={selectedStrategy()?.category}>
                      <span class="px-2 py-1 text-xs bg-[var(--color-primary)]/20 text-[var(--color-primary)] rounded-lg font-medium">
                        {selectedStrategy()?.category}
                      </span>
                    </Show>
                  </div>

                  {/* 설명 */}
                  <p class="text-sm text-[var(--color-text-muted)]">
                    {selectedStrategy()?.description}
                  </p>
                </div>

                {/* 전략 이름 커스터마이징 */}
                <div>
                  <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                    전략 이름
                  </label>
                  <input
                    type="text"
                    value={customName()}
                    onInput={(e) => setCustomName(e.currentTarget.value)}
                    placeholder="전략 이름을 입력하세요"
                    class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                  />
                  <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                    동일한 전략을 다른 종목이나 설정으로 여러 개 등록할 수 있습니다.
                  </p>
                </div>

                {/* 거래소 계정 선택 */}
                <Show when={credentials() && credentials()!.length > 0}>
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div>
                      <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                        대상 계좌
                      </label>
                      <select
                        value={selectedCredentialId() || activeCredential()?.id || ''}
                        onChange={(e) => handleCredentialChange(e.currentTarget.value || null)}
                        class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                      >
                        <For each={credentials()}>
                          {(cred) => (
                            <option value={cred.id}>
                              {cred.exchange_name}
                              {cred.is_active && ' (활성)'}
                              {cred.exchange_id === 'mock' && ' 🧪'}
                            </option>
                          )}
                        </For>
                      </select>
                      <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                        전략이 실행될 거래소 계좌를 선택하세요.
                      </p>
                    </div>

                    {/* 시장 선택 */}
                    <Show when={supportedMarkets().length > 0}>
                      <div>
                        <label class="block text-sm font-medium text-[var(--color-text)] mb-2">
                          대상 시장
                        </label>
                        <select
                          value={selectedMarket() || supportedMarkets()[0]?.value || ''}
                          onChange={(e) => setSelectedMarket(e.currentTarget.value)}
                          class="w-full px-4 py-2.5 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg text-[var(--color-text)] focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
                        >
                          <For each={supportedMarkets()}>
                            {(market) => (
                              <option value={market.value}>{market.label}</option>
                            )}
                          </For>
                        </select>
                        <p class="mt-1 text-xs text-[var(--color-text-muted)]">
                          {supportedMarkets().length > 1
                            ? '전략이 거래할 시장을 선택하세요.'
                            : '해당 계좌에서 지원하는 시장입니다.'}
                        </p>
                      </div>
                    </Show>
                  </div>
                </Show>

                {/* 다중 타임프레임 설정 (지원 전략만) */}
                <Show when={selectedStrategy()?.isMultiTimeframe}>
                  <div class="p-4 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-lg">
                    <div class="flex items-center justify-between mb-4">
                      <div class="flex items-center gap-2">
                        <Clock class="w-5 h-5 text-[var(--color-primary)]" />
                        <span class="font-medium text-[var(--color-text)]">다중 타임프레임 설정</span>
                      </div>
                      <label class="flex items-center gap-2 cursor-pointer">
                        <input
                          type="checkbox"
                          checked={enableMultiTf()}
                          onChange={(e) => {
                            const enabled = e.currentTarget.checked
                            setEnableMultiTf(enabled)
                            if (enabled && !multiTfConfig()) {
                              // 기본값 설정
                              const defaultTf = selectedStrategy()?.defaultTimeframe as Timeframe || '1d'
                              setMultiTfConfig({
                                primary: defaultTf,
                                secondary: [],
                              })
                            }
                          }}
                          class="w-4 h-4 text-[var(--color-primary)] rounded focus:ring-[var(--color-primary)]"
                        />
                        <span class="text-sm text-[var(--color-text-muted)]">활성화</span>
                      </label>
                    </div>
                    <Show when={enableMultiTf()}>
                      <MultiTimeframeSelector
                        primaryTimeframe={multiTfConfig()?.primary || '1d'}
                        secondaryTimeframes={(multiTfConfig()?.secondary || []).map(s => s.timeframe)}
                        onPrimaryChange={(tf) => {
                          setMultiTfConfig(prev => prev ? {
                            ...prev,
                            primary: tf,
                            // Primary보다 작은 Secondary는 제거
                            secondary: prev.secondary.filter(s => {
                              const tfOrder: Timeframe[] = ['1m', '5m', '15m', '30m', '1h', '4h', '1d', '1w', '1M']
                              return tfOrder.indexOf(s.timeframe) > tfOrder.indexOf(tf)
                            }),
                          } : { primary: tf, secondary: [] })
                        }}
                        onSecondaryChange={(tfs) => {
                          setMultiTfConfig(prev => prev ? {
                            ...prev,
                            secondary: tfs.map(tf => ({ timeframe: tf, candle_count: 100 })),
                          } : { primary: '1d', secondary: tfs.map(tf => ({ timeframe: tf, candle_count: 100 })) })
                        }}
                        maxSecondary={3}
                      />
                      <p class="mt-3 text-xs text-[var(--color-text-muted)]">
                        Primary 타임프레임으로 전략이 실행되고, Secondary 타임프레임으로 추세를 확인합니다.
                      </p>
                    </Show>
                  </div>
                </Show>

                {/* SDUI 렌더러 (동적 폼) */}
                <SDUIRenderer
                  strategyId={selectedStrategy()!.id}
                  onChange={handleFormChange}
                  onSubmit={handleCreateStrategy}
                  onCancel={goBack}
                  submitLabel={isCreating() ? '생성 중...' : '전략 생성'}
                  cancelLabel="뒤로"
                  loadingMessage="전략 설정을 불러오는 중..."
                />
              </div>
            </Show>
          </div>

          {/* 에러 메시지 (푸터) */}
          <Show when={createError()}>
            <div class="flex items-center gap-2 p-4 border-t border-[var(--color-surface-light)] bg-red-500/10">
              <AlertCircle class="w-4 h-4 text-red-500" />
              <span class="text-sm text-red-500">{createError()}</span>
            </div>
          </Show>
        </div>
      </div>
    </Show>
  )
}
