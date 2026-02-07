/**
 * SDUI 기반 전략 편집 모달
 *
 * SDUIRenderer를 사용하여 백엔드 스키마 기반으로 전략 설정 UI를 자동 생성합니다.
 */
import { type Component, Show, For, createSignal, createEffect, onCleanup, createResource } from 'solid-js';
import { X } from 'lucide-solid';
import { SDUIRenderer } from './strategy/SDUIRenderer';
import { getStrategy, updateStrategyConfig, updateStrategyCredential, listCredentials } from '../api/client';
import type { ExchangeCredential } from '../api/client';
import { useToast } from './Toast';
import { Spinner } from './ui/Loading';
import { createLogger } from '../utils/logger';

const { error: logError } = createLogger('SDUIEditModal');

// ==================== Props ====================

export interface SDUIEditModalProps {
  /** 모달 열림 여부 */
  open: boolean;
  /** 편집할 전략 ID */
  strategyId: string | null;
  /** 전략 유형 (예: "grid", "rsi") - SDUI 스키마 조회에 사용 */
  strategyType: string | null;
  /** 모달 닫기 콜백 */
  onClose: () => void;
  /** 전략 업데이트 성공 시 콜백 */
  onSuccess: () => void;
}

// ==================== 컴포넌트 ====================

/**
 * SDUI 기반 전략 편집 모달
 *
 * @example
 * ```tsx
 * <SDUIEditModal
 *   open={showEditModal()}
 *   strategyId={editingStrategyId()}
 *   strategyType={editingStrategyType()}
 *   onClose={() => setShowEditModal(false)}
 *   onSuccess={() => refetchStrategies()}
 * />
 * ```
 */
export const SDUIEditModal: Component<SDUIEditModalProps> = (props) => {
  const toast = useToast();

  // 상태
  const [strategyName, setStrategyName] = createSignal('');
  const [initialValues, setInitialValues] = createSignal<Record<string, unknown>>({});
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // 거래소 계정 상태
  const [currentCredentialId, setCurrentCredentialId] = createSignal<string | null>(null);
  const [currentMarket, setCurrentMarket] = createSignal<string | null>(null);
  const [selectedCredentialId, setSelectedCredentialId] = createSignal<string | null>(null);
  const [selectedMarket, setSelectedMarket] = createSignal<string | null>(null);

  // 거래소 계정 목록 로드
  const [credentials] = createResource(async () => {
    try {
      const response = await listCredentials();
      return response.credentials || [];
    } catch {
      return [] as ExchangeCredential[];
    }
  });

  // 거래소별 지원 시장 목록
  const getSupportedMarkets = (exchangeId: string): { value: string; label: string }[] => {
    switch (exchangeId) {
      case 'kis':
        return [
          { value: 'KR', label: '한국 주식' },
          { value: 'US', label: '미국 주식' },
        ];
      case 'binance':
      case 'coinbase':
        return [{ value: 'CRYPTO', label: '암호화폐' }];
      case 'mock':
        return [
          { value: 'KR', label: '한국 주식' },
          { value: 'US', label: '미국 주식' },
          { value: 'CRYPTO', label: '암호화폐' },
        ];
      default:
        return [];
    }
  };

  // 현재 선택된 계정의 지원 시장
  const currentCredential = () => {
    const id = selectedCredentialId() || currentCredentialId();
    return credentials()?.find(c => c.id === id);
  };
  const supportedMarkets = () => {
    const cred = currentCredential();
    return cred ? getSupportedMarkets(cred.exchange_id) : [];
  };

  // 계정 변경 시 시장 자동 선택
  const handleCredentialChange = (credId: string | null) => {
    setSelectedCredentialId(credId);
    const cred = credentials()?.find(c => c.id === credId);
    if (cred) {
      const markets = getSupportedMarkets(cred.exchange_id);
      if (markets.length === 1) {
        setSelectedMarket(markets[0].value);
      } else {
        // 현재 시장이 해당 거래소에서 지원되면 유지
        const currentMkt = selectedMarket() || currentMarket();
        if (currentMkt && markets.some(m => m.value === currentMkt)) {
          setSelectedMarket(currentMkt);
        } else {
          setSelectedMarket(null);
        }
      }
    }
  };

  // 전략 데이터 로드
  const loadStrategy = async () => {
    const strategyId = props.strategyId;
    if (!strategyId) return;

    setLoading(true);
    setError(null);

    try {
      const detail = await getStrategy(strategyId);
      setStrategyName(detail.name);
      setInitialValues(detail.config as Record<string, unknown>);

      // 거래소 계정 및 시장 정보 설정
      setCurrentCredentialId(detail.credentialId || null);
      setCurrentMarket(detail.market || null);
      setSelectedCredentialId(detail.credentialId || null);
      setSelectedMarket(detail.market || null);
    } catch (err) {
      logError('Failed to load strategy:', err);
      const errorMsg = err instanceof Error ? err.message : '전략 정보를 불러오는데 실패했습니다';
      setError(errorMsg);
      toast.error('전략 로드 실패', errorMsg);
    } finally {
      setLoading(false);
    }
  };

  // 모달 열릴 때 전략 로드
  createEffect(() => {
    if (props.open && props.strategyId) {
      loadStrategy();
    }
  });

  // ESC 키로 모달 닫기
  createEffect(() => {
    if (!props.open) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleClose();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
  });

  // 모달 닫기
  const handleClose = () => {
    setStrategyName('');
    setInitialValues({});
    setError(null);
    setCurrentCredentialId(null);
    setCurrentMarket(null);
    setSelectedCredentialId(null);
    setSelectedMarket(null);
    props.onClose();
  };

  // 제출 핸들러
  const handleSubmit = async (values: Record<string, unknown>) => {
    const strategyId = props.strategyId;
    if (!strategyId) return;

    try {
      // 설정에 이름 포함
      const configWithName = {
        ...values,
        name: strategyName(),
      };

      await updateStrategyConfig(strategyId, configWithName);

      // credential/market 변경이 있는 경우 별도 업데이트
      const credentialChanged = selectedCredentialId() !== currentCredentialId();
      const marketChanged = selectedMarket() !== currentMarket();

      if (credentialChanged || marketChanged) {
        await updateStrategyCredential(strategyId, {
          credentialId: selectedCredentialId() || undefined,
          market: selectedMarket() || undefined,
        });
      }

      toast.success('전략 업데이트 완료', `"${strategyName()}" 설정이 저장되었습니다`);
      handleClose();
      props.onSuccess();
    } catch (err) {
      logError('Failed to update strategy:', err);
      const errorMsg = err instanceof Error ? err.message : '전략 업데이트에 실패했습니다';
      toast.error('전략 업데이트 실패', errorMsg);
    }
  };

  return (
    <Show when={props.open}>
      <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
        {/* 배경 오버레이 */}
        <div
          class="absolute inset-0 bg-black/50"
          onClick={handleClose}
        />

        {/* 모달 콘텐츠 */}
        <div class="relative w-full max-w-3xl max-h-[90vh] bg-white dark:bg-gray-800 rounded-2xl shadow-2xl overflow-hidden flex flex-col">
          {/* 헤더 */}
          <div class="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-700">
            <div>
              <h2 class="text-xl font-semibold text-gray-900 dark:text-white">
                전략 설정
              </h2>
              <Show when={strategyName()}>
                <p class="text-sm text-gray-500 dark:text-gray-400">
                  {strategyName()} 파라미터 수정
                </p>
              </Show>
            </div>

            {/* 닫기 버튼 */}
            <button
              type="button"
              onClick={handleClose}
              class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-700"
            >
              <X class="w-5 h-5" />
            </button>
          </div>

          {/* 본문 */}
          <div class="flex-1 overflow-y-auto p-6">
            {/* 로딩 상태 */}
            <Show when={loading()}>
              <div class="flex flex-col items-center justify-center py-12">
                <Spinner size="lg" />
                <p class="mt-4 text-gray-500 dark:text-gray-400">
                  전략 정보를 불러오는 중...
                </p>
              </div>
            </Show>

            {/* 에러 상태 */}
            <Show when={error() && !loading()}>
              <div class="flex flex-col items-center justify-center py-12 text-red-500">
                <p class="text-lg font-medium">로드 실패</p>
                <p class="mt-2 text-sm">{error()}</p>
                <button
                  type="button"
                  onClick={loadStrategy}
                  class="mt-4 px-4 py-2 bg-red-100 dark:bg-red-900 text-red-600 dark:text-red-300 rounded-lg hover:bg-red-200"
                >
                  다시 시도
                </button>
              </div>
            </Show>

            {/* SDUI 렌더러 */}
            <Show when={!loading() && !error() && props.strategyType}>
              {/* 전략 이름 입력 */}
              <div class="mb-6">
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  전략 이름 <span class="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  value={strategyName()}
                  onInput={(e) => setStrategyName(e.currentTarget.value)}
                  placeholder="전략 이름 입력"
                  class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md dark:bg-gray-700 dark:text-white focus:ring-blue-500 focus:border-blue-500"
                />
              </div>

              {/* 거래소 계정 및 시장 선택 */}
              <Show when={credentials() && credentials()!.length > 0}>
                <div class="mb-6 grid grid-cols-1 md:grid-cols-2 gap-4">
                  {/* 거래소 계정 선택 */}
                  <div>
                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      대상 계좌
                    </label>
                    <select
                      value={selectedCredentialId() || ''}
                      onChange={(e) => handleCredentialChange(e.currentTarget.value || null)}
                      class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md dark:bg-gray-700 dark:text-white focus:ring-blue-500 focus:border-blue-500"
                    >
                      <option value="">선택 안함 (기본 활성 계정)</option>
                      <For each={credentials()}>
                        {(cred) => (
                          <option value={cred.id}>
                            {cred.exchange_name || cred.exchange_id}
                            {cred.is_active && ' (활성)'}
                            {cred.exchange_id === 'mock' && ' 🧪'}
                          </option>
                        )}
                      </For>
                    </select>
                    <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                      전략이 실행될 거래소 계좌를 선택하세요.
                    </p>
                  </div>

                  {/* 시장 선택 */}
                  <Show when={supportedMarkets().length > 0}>
                    <div>
                      <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                        대상 시장
                      </label>
                      <select
                        value={selectedMarket() || supportedMarkets()[0]?.value || ''}
                        onChange={(e) => setSelectedMarket(e.currentTarget.value)}
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md dark:bg-gray-700 dark:text-white focus:ring-blue-500 focus:border-blue-500"
                      >
                        <For each={supportedMarkets()}>
                          {(market) => (
                            <option value={market.value}>{market.label}</option>
                          )}
                        </For>
                      </select>
                      <p class="mt-1 text-xs text-gray-500 dark:text-gray-400">
                        {supportedMarkets().length > 1
                          ? '전략이 거래할 시장을 선택하세요.'
                          : '해당 계좌에서 지원하는 시장입니다.'}
                      </p>
                    </div>
                  </Show>
                </div>
              </Show>

              {/* SDUI 폼 */}
              <SDUIRenderer
                strategyId={props.strategyType!}
                initialValues={initialValues()}
                onSubmit={handleSubmit}
                onCancel={handleClose}
                submitLabel="저장"
                cancelLabel="취소"
              />
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default SDUIEditModal;
