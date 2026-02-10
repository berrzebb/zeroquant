/**
 * 다중 심볼 입력 컴포넌트
 *
 * 여러 종목을 검색/추가하고 드래그 앤 드롭으로 순서를 변경할 수 있습니다.
 * 심볼 정보(종목명, 거래소)를 함께 표시합니다.
 *
 * @example
 * ```tsx
 * <MultiSymbolInput
 *   value={symbols()}
 *   onChange={setSymbols}
 *   maxCount={10}
 *   enableDragDrop
 * />
 * ```
 */
import {
  type Component,
  createSignal,
  createEffect,
  on,
  Show,
  For,
  createMemo,
} from 'solid-js';
import { SymbolSearch } from '../../../../components/SymbolSearch';
import { getSymbolsBatch, type SymbolSearchResult } from '../../../../api/client';
import { createLogger } from '../../../../utils/logger';

const { warn } = createLogger('MultiSymbolInput');

// ==================== Props ====================

export interface MultiSymbolInputProps {
  /** HTML id */
  id?: string;
  /** 현재 선택된 심볼 배열 */
  value: string[];
  /** 값 변경 핸들러 */
  onChange: (value: string[]) => void;
  /** 최대 개수 제한 (0 = 무제한) */
  maxCount?: number;
  /** 드래그 앤 드롭 활성화 */
  enableDragDrop?: boolean;
  /** 읽기 전용 */
  readOnly?: boolean;
  /** 시장 필터 */
  market?: 'KR' | 'US' | 'CRYPTO' | 'ALL';
  /** 플레이스홀더 */
  placeholder?: string;
  /** 추가 클래스 */
  class?: string;
}

// ==================== 심볼 정보 캐시 ====================

/** 심볼 정보 캐시 (전역) */
const symbolInfoCache = new Map<string, SymbolSearchResult>();

// ==================== 컴포넌트 ====================

/**
 * 다중 심볼 입력 컴포넌트
 */
export const MultiSymbolInput: Component<MultiSymbolInputProps> = (props) => {
  const maxCount = () => props.maxCount ?? 0;
  const enableDragDrop = () => props.enableDragDrop ?? true;

  const [draggedIndex, setDraggedIndex] = createSignal<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = createSignal<number | null>(null);

  // 심볼 정보 로컬 상태 (캐시된 정보 반영용)
  const [symbolInfoMap, setSymbolInfoMap] = createSignal<Map<string, SymbolSearchResult>>(new Map());
  const [isLoadingInfo, setIsLoadingInfo] = createSignal(false);

  // 심볼 정보 가져오기
  const getSymbolInfo = (ticker: string): SymbolSearchResult | undefined => {
    return symbolInfoMap().get(ticker.toUpperCase()) || symbolInfoCache.get(ticker.toUpperCase());
  };

  // 최대 개수에 도달했는지 확인
  const isMaxReached = createMemo(() => {
    const max = maxCount();
    return max > 0 && (props.value || []).length >= max;
  });

  // 남은 추가 가능 개수
  const remainingCount = createMemo(() => {
    const max = maxCount();
    if (max === 0) return null;
    return max - (props.value || []).length;
  });

  // props.value 변경 시 심볼 정보 로드 (명시적 의존성)
  createEffect(on(() => props.value, (tickers) => {
    const tickerList = tickers || [];
    if (tickerList.length === 0) return;

    // 캐시에 없는 티커만 조회
    const missingTickers = tickerList.filter(
      (t) => !symbolInfoCache.has(t.toUpperCase())
    );

    if (missingTickers.length === 0) {
      // 모든 정보가 캐시에 있으면 로컬 상태 업데이트
      const newMap = new Map<string, SymbolSearchResult>();
      tickerList.forEach((t) => {
        const info = symbolInfoCache.get(t.toUpperCase());
        if (info) newMap.set(t.toUpperCase(), info);
      });
      setSymbolInfoMap(newMap);
      return;
    }

    // API로 심볼 정보 조회
    setIsLoadingInfo(true);
    getSymbolsBatch(missingTickers)
      .then((results) => {
        // 캐시 업데이트
        results.forEach((info) => {
          symbolInfoCache.set(info.ticker.toUpperCase(), info);
        });

        // 로컬 상태 업데이트
        const newMap = new Map<string, SymbolSearchResult>();
        tickerList.forEach((t) => {
          const info = symbolInfoCache.get(t.toUpperCase());
          if (info) newMap.set(t.toUpperCase(), info);
        });
        setSymbolInfoMap(newMap);
      })
      .catch((error) => {
        warn('심볼 정보 조회 실패:', error);
      })
      .finally(() => {
        setIsLoadingInfo(false);
      });
  }));

  // 새 심볼 추가
  const handleAddSymbol = (symbol: string, info?: SymbolSearchResult) => {
    if (props.readOnly || !symbol.trim()) return;

    const upperSymbol = symbol.trim().toUpperCase();
    const currentValue = props.value || [];

    // 중복 체크
    if (currentValue.includes(upperSymbol)) {
      return;
    }

    // 최대 개수 체크
    if (isMaxReached()) {
      return;
    }

    // 심볼 정보가 있으면 캐시에 저장
    if (info) {
      symbolInfoCache.set(upperSymbol, info);
      setSymbolInfoMap((prev) => {
        const newMap = new Map(prev);
        newMap.set(upperSymbol, info);
        return newMap;
      });
    }

    props.onChange([...currentValue, upperSymbol]);
  };

  // 심볼 제거
  const handleRemove = (symbol: string) => {
    if (props.readOnly) return;
    const newValue = (props.value || []).filter((v) => v !== symbol);
    props.onChange(newValue);
  };

  // 드래그 시작
  const handleDragStart = (index: number) => (e: DragEvent) => {
    if (!enableDragDrop() || props.readOnly) return;
    setDraggedIndex(index);
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', index.toString());
    }
  };

  // 드래그 종료
  const handleDragEnd = () => {
    setDraggedIndex(null);
    setDragOverIndex(null);
  };

  // 드래그 오버
  const handleDragOver = (index: number) => (e: DragEvent) => {
    e.preventDefault();
    if (!enableDragDrop() || props.readOnly) return;
    setDragOverIndex(index);
  };

  // 드래그 리브
  const handleDragLeave = () => {
    setDragOverIndex(null);
  };

  // 드롭
  const handleDrop = (dropIndex: number) => (e: DragEvent) => {
    e.preventDefault();
    if (!enableDragDrop() || props.readOnly) return;

    const fromIndex = draggedIndex();
    if (fromIndex === null || fromIndex === dropIndex) {
      setDraggedIndex(null);
      setDragOverIndex(null);
      return;
    }

    const currentValue = [...(props.value || [])];
    const [removed] = currentValue.splice(fromIndex, 1);
    currentValue.splice(dropIndex, 0, removed);

    props.onChange(currentValue);
    setDraggedIndex(null);
    setDragOverIndex(null);
  };

  // 순서 위로 이동
  const handleMoveUp = (index: number) => {
    if (props.readOnly || index === 0) return;
    const currentValue = [...(props.value || [])];
    [currentValue[index - 1], currentValue[index]] = [currentValue[index], currentValue[index - 1]];
    props.onChange(currentValue);
  };

  // 순서 아래로 이동
  const handleMoveDown = (index: number) => {
    if (props.readOnly || index >= (props.value || []).length - 1) return;
    const currentValue = [...(props.value || [])];
    [currentValue[index], currentValue[index + 1]] = [currentValue[index + 1], currentValue[index]];
    props.onChange(currentValue);
  };

  // 심볼 표시 문자열 생성
  const getDisplayText = (ticker: string): { ticker: string; name?: string; market?: string } => {
    const info = getSymbolInfo(ticker);
    if (info) {
      return {
        ticker: info.ticker,
        name: info.name,
        market: info.market,
      };
    }
    return { ticker };
  };

  return (
    <div class={props.class}>
      {/* 선택된 심볼 태그 목록 */}
      <Show when={(props.value || []).length > 0}>
        <div class="flex flex-wrap gap-2 mb-3">
          <For each={props.value || []}>
            {(symbol, index) => {
              const isDragged = () => draggedIndex() === index();
              const isDragOver = () => dragOverIndex() === index();
              const displayInfo = () => getDisplayText(symbol);

              return (
                <div
                  class={`
                    inline-flex items-center gap-1.5 px-2.5 py-1.5 text-sm rounded-lg
                    transition-all
                    ${isDragged() ? 'opacity-50 scale-95' : ''}
                    ${isDragOver() ? 'ring-2 ring-blue-500 ring-offset-1' : ''}
                    ${enableDragDrop() && !props.readOnly ? 'cursor-grab active:cursor-grabbing' : ''}
                    bg-blue-50 dark:bg-blue-900/50 border border-blue-200 dark:border-blue-700
                  `}
                  draggable={enableDragDrop() && !props.readOnly}
                  onDragStart={handleDragStart(index())}
                  onDragEnd={handleDragEnd}
                  onDragOver={handleDragOver(index())}
                  onDragLeave={handleDragLeave}
                  onDrop={handleDrop(index())}
                >
                  {/* 순서 번호 */}
                  <span class="text-xs text-blue-500 dark:text-blue-400 font-semibold min-w-[18px] text-center">
                    {index() + 1}
                  </span>

                  {/* 심볼 정보 */}
                  <div class="flex items-center gap-1">
                    {/* 티커 */}
                    <span class="font-semibold text-blue-800 dark:text-blue-200">
                      {displayInfo().ticker}
                    </span>

                    {/* 종목명 */}
                    <Show when={displayInfo().name}>
                      <span class="text-blue-600 dark:text-blue-300 text-xs">
                        {displayInfo().name}
                      </span>
                    </Show>

                    {/* 거래소 */}
                    <Show when={displayInfo().market}>
                      <span class="text-[10px] px-1 py-0.5 rounded bg-blue-200 dark:bg-blue-800 text-blue-700 dark:text-blue-300">
                        {displayInfo().market}
                      </span>
                    </Show>
                  </div>

                  {/* 순서 변경 버튼 (드래그 비활성화 시) */}
                  <Show when={!enableDragDrop() && !props.readOnly && (props.value || []).length > 1}>
                    <div class="flex flex-col ml-1">
                      <button
                        type="button"
                        onClick={() => handleMoveUp(index())}
                        disabled={index() === 0}
                        class="text-xs text-blue-600 dark:text-blue-400 hover:text-blue-800 disabled:opacity-30"
                        title="위로 이동"
                      >
                        ▲
                      </button>
                      <button
                        type="button"
                        onClick={() => handleMoveDown(index())}
                        disabled={index() >= (props.value || []).length - 1}
                        class="text-xs text-blue-600 dark:text-blue-400 hover:text-blue-800 disabled:opacity-30"
                        title="아래로 이동"
                      >
                        ▼
                      </button>
                    </div>
                  </Show>

                  {/* 제거 버튼 */}
                  <Show when={!props.readOnly}>
                    <button
                      type="button"
                      onClick={() => handleRemove(symbol)}
                      class="ml-0.5 w-5 h-5 flex items-center justify-center rounded-full text-blue-500 dark:text-blue-400 hover:bg-red-100 dark:hover:bg-red-900 hover:text-red-500 transition-colors"
                      title={`${displayInfo().name || symbol} 제거`}
                    >
                      ×
                    </button>
                  </Show>
                </div>
              );
            }}
          </For>

          {/* 로딩 표시 */}
          <Show when={isLoadingInfo()}>
            <div class="inline-flex items-center gap-1 px-2 py-1 text-xs text-gray-500">
              <div class="w-3 h-3 border-2 border-gray-300 border-t-blue-500 rounded-full animate-spin" />
              정보 로딩 중...
            </div>
          </Show>
        </div>
      </Show>

      {/* 심볼 추가 (자동완성) */}
      <Show when={!props.readOnly}>
        <Show
          when={!isMaxReached()}
          fallback={
            <div class="px-3 py-2 text-sm text-orange-600 dark:text-orange-400 bg-orange-50 dark:bg-orange-900/20 rounded-md">
              최대 {maxCount()}개까지만 추가할 수 있습니다.
            </div>
          }
        >
          <div class="flex gap-2">
            <div class="flex-1">
              <SymbolSearch
                onSelect={(symbol) => {
                  if (symbol) {
                    handleAddSymbol(symbol);
                  }
                }}
                placeholder={props.placeholder || '종목 코드 또는 이름으로 검색하여 추가'}
                size="md"
              />
            </div>
          </div>
        </Show>

        {/* 힌트 텍스트 */}
        <div class="mt-1 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
          <span>
            {enableDragDrop()
              ? '드래그하여 순서를 변경할 수 있습니다'
              : '검색 후 선택하면 자동으로 추가됩니다'}
          </span>
          <Show when={maxCount() > 0}>
            <span class={remainingCount()! <= 2 ? 'text-orange-500' : ''}>
              {remainingCount()}개 추가 가능
            </span>
          </Show>
        </div>
      </Show>

      {/* 빈 상태 */}
      <Show when={(props.value || []).length === 0 && props.readOnly}>
        <div class="text-sm text-gray-500 dark:text-gray-400 italic">
          선택된 종목이 없습니다
        </div>
      </Show>
    </div>
  );
};

export default MultiSymbolInput;
