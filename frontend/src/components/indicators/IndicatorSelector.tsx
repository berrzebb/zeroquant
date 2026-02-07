/**
 * 기술적 지표 선택 컴포넌트.
 *
 * 사용자가 차트에 표시할 지표를 선택하고 파라미터를 설정할 수 있습니다.
 */

import { createSignal, createResource, For, Show, createEffect } from 'solid-js';
import {
  getAvailableIndicators,
  groupIndicatorsByCategory,
  isOverlayIndicator,
  type IndicatorInfo,
  type IndicatorConfig,
  type IndicatorCategory,
} from '../../api/indicators';

/** 선택된 지표 상태 */
export interface SelectedIndicator extends IndicatorConfig {
  /** 지표 정보 */
  info: IndicatorInfo;
  /** 활성화 여부 */
  enabled: boolean;
}

interface IndicatorSelectorProps {
  /** 심볼 (예: 005930, AAPL) */
  symbol: string;
  /** 선택된 지표가 변경될 때 호출 */
  onIndicatorsChange: (indicators: SelectedIndicator[]) => void;
  /** 초기 선택된 지표 목록 */
  initialIndicators?: SelectedIndicator[];
}

/** 카테고리 아이콘 */
const categoryIcons: Record<IndicatorCategory, string> = {
  '추세': '📈',
  '모멘텀': '⚡',
  '변동성': '📊',
};

/** 기본 색상 팔레트 */
const defaultColors = [
  '#2196F3', // Blue
  '#FF9800', // Orange
  '#4CAF50', // Green
  '#E91E63', // Pink
  '#9C27B0', // Purple
  '#00BCD4', // Cyan
  '#FF5722', // Deep Orange
  '#795548', // Brown
];

export function IndicatorSelector(props: IndicatorSelectorProps) {
  // API에서 사용 가능한 지표 목록 가져오기
  const [indicators] = createResource(getAvailableIndicators);

  // 선택된 지표 상태
  const [selectedIndicators, setSelectedIndicators] = createSignal<SelectedIndicator[]>(
    props.initialIndicators || []
  );

  // 펼쳐진 카테고리 상태
  const [expandedCategories, setExpandedCategories] = createSignal<Set<IndicatorCategory>>(
    new Set(['추세', '모멘텀', '변동성'])
  );

  // 파라미터 편집 중인 지표 ID
  const [editingIndicator, setEditingIndicator] = createSignal<string | null>(null);

  // 색상 인덱스 (자동 할당용)
  let colorIndex = 0;

  // 선택된 지표가 변경될 때 부모에게 알림
  createEffect(() => {
    const selected = selectedIndicators();
    props.onIndicatorsChange(selected);
  });

  // 카테고리 토글
  const toggleCategory = (category: IndicatorCategory) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  };

  // 지표 선택/해제
  const toggleIndicator = (indicator: IndicatorInfo) => {
    setSelectedIndicators((prev) => {
      const existing = prev.find((s) => s.type === indicator.id);
      if (existing) {
        // 이미 선택된 경우 제거
        return prev.filter((s) => s.type !== indicator.id);
      } else {
        // 새로 선택
        const newIndicator: SelectedIndicator = {
          type: indicator.id,
          params: { ...indicator.defaultParams },
          color: defaultColors[colorIndex++ % defaultColors.length],
          name: indicator.name,
          info: indicator,
          enabled: true,
        };
        return [...prev, newIndicator];
      }
    });
  };

  // 지표 활성화/비활성화 토글
  const toggleIndicatorEnabled = (indicatorId: string) => {
    setSelectedIndicators((prev) =>
      prev.map((s) =>
        s.type === indicatorId ? { ...s, enabled: !s.enabled } : s
      )
    );
  };

  // 지표 파라미터 업데이트
  const updateIndicatorParam = (indicatorId: string, paramName: string, value: number) => {
    setSelectedIndicators((prev) =>
      prev.map((s) =>
        s.type === indicatorId
          ? { ...s, params: { ...s.params, [paramName]: value } }
          : s
      )
    );
  };

  // 지표 색상 업데이트
  const updateIndicatorColor = (indicatorId: string, color: string) => {
    setSelectedIndicators((prev) =>
      prev.map((s) =>
        s.type === indicatorId ? { ...s, color } : s
      )
    );
  };

  // 선택된 지표인지 확인
  const isSelected = (indicatorId: string) =>
    selectedIndicators().some((s) => s.type === indicatorId);

  // 선택된 지표 가져오기
  const getSelectedIndicator = (indicatorId: string) =>
    selectedIndicators().find((s) => s.type === indicatorId);

  return (
    <div class="bg-gray-800 rounded-lg p-4 text-white">
      <h3 class="text-lg font-semibold mb-4 flex items-center gap-2">
        <span>📉</span>
        <span>기술적 지표</span>
        <span class="text-sm text-gray-400 font-normal">
          ({selectedIndicators().filter((s) => s.enabled).length}개 선택)
        </span>
      </h3>

      {/* 로딩 상태 */}
      <Show when={indicators.loading}>
        <div class="flex items-center justify-center py-8">
          <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500"></div>
        </div>
      </Show>

      {/* 에러 상태 */}
      <Show when={indicators.error}>
        <div class="text-red-400 py-4">
          지표 목록을 불러오는데 실패했습니다.
        </div>
      </Show>

      {/* 지표 목록 */}
      <Show when={indicators()}>
        {(indicatorList) => {
          const grouped = () => groupIndicatorsByCategory(indicatorList());

          return (
            <div class="space-y-3">
              <For each={Object.entries(grouped()) as [IndicatorCategory, IndicatorInfo[]][]}>
                {([category, categoryIndicators]) => (
                  <div class="border border-gray-700 rounded-lg overflow-hidden">
                    {/* 카테고리 헤더 */}
                    <button
                      class="w-full flex items-center justify-between p-3 bg-gray-750 hover:bg-gray-700 transition-colors"
                      onClick={() => toggleCategory(category)}
                    >
                      <span class="flex items-center gap-2">
                        <span>{categoryIcons[category]}</span>
                        <span class="font-medium">{category}</span>
                        <span class="text-xs text-gray-400">
                          ({categoryIndicators.length})
                        </span>
                      </span>
                      <span class="text-gray-400">
                        {expandedCategories().has(category) ? '▼' : '▶'}
                      </span>
                    </button>

                    {/* 카테고리 내 지표 목록 */}
                    <Show when={expandedCategories().has(category)}>
                      <div class="p-2 space-y-2">
                        <For each={categoryIndicators}>
                          {(indicator) => {
                            const selected = () => getSelectedIndicator(indicator.id);
                            const isEditing = () => editingIndicator() === indicator.id;

                            return (
                              <div
                                class={`rounded-lg p-3 transition-colors ${
                                  isSelected(indicator.id)
                                    ? 'bg-blue-900/30 border border-blue-600'
                                    : 'bg-gray-750 hover:bg-gray-700 border border-transparent'
                                }`}
                              >
                                {/* 지표 헤더 */}
                                <div class="flex items-center justify-between">
                                  <label class="flex items-center gap-3 cursor-pointer flex-1">
                                    <input
                                      type="checkbox"
                                      checked={isSelected(indicator.id)}
                                      onChange={() => toggleIndicator(indicator)}
                                      class="w-4 h-4 rounded border-gray-600 text-blue-600 focus:ring-blue-500"
                                    />
                                    <div>
                                      <div class="font-medium flex items-center gap-2">
                                        {indicator.name}
                                        <span
                                          class={`text-xs px-1.5 py-0.5 rounded ${
                                            isOverlayIndicator(indicator.id)
                                              ? 'bg-blue-500/20 text-blue-400'
                                              : 'bg-purple-500/20 text-purple-400'
                                          }`}
                                        >
                                          {isOverlayIndicator(indicator.id) ? '오버레이' : '별도패널'}
                                        </span>
                                      </div>
                                      <div class="text-xs text-gray-400">{indicator.description}</div>
                                    </div>
                                  </label>

                                  {/* 선택된 경우 설정 버튼 */}
                                  <Show when={isSelected(indicator.id)}>
                                    <div class="flex items-center gap-2">
                                      {/* 활성화 토글 */}
                                      <button
                                        class={`p-1 rounded ${
                                          selected()?.enabled
                                            ? 'text-green-400'
                                            : 'text-gray-500'
                                        }`}
                                        onClick={() => toggleIndicatorEnabled(indicator.id)}
                                        title={selected()?.enabled ? '비활성화' : '활성화'}
                                      >
                                        {selected()?.enabled ? '👁️' : '👁️‍🗨️'}
                                      </button>

                                      {/* 설정 버튼 */}
                                      <button
                                        class={`p-1 rounded ${
                                          isEditing()
                                            ? 'text-blue-400 bg-blue-900/30'
                                            : 'text-gray-400 hover:text-white'
                                        }`}
                                        onClick={() =>
                                          setEditingIndicator(
                                            isEditing() ? null : indicator.id
                                          )
                                        }
                                        title="파라미터 설정"
                                      >
                                        ⚙️
                                      </button>

                                      {/* 색상 표시 */}
                                      <div
                                        class="w-4 h-4 rounded-full border border-gray-600"
                                        style={{ "background-color": selected()?.color }}
                                      />
                                    </div>
                                  </Show>
                                </div>

                                {/* 파라미터 편집 패널 */}
                                <Show when={isSelected(indicator.id) && isEditing()}>
                                  <div class="mt-3 pt-3 border-t border-gray-700 space-y-3">
                                    {/* 파라미터 입력 */}
                                    <For each={Object.entries(indicator.defaultParams)}>
                                      {([paramName, defaultValue]) => (
                                        <div class="flex items-center gap-3">
                                          <label class="text-sm text-gray-300 w-24">
                                            {getParamLabel(paramName)}
                                          </label>
                                          <input
                                            type="number"
                                            value={selected()?.params[paramName] ?? defaultValue}
                                            onInput={(e) =>
                                              updateIndicatorParam(
                                                indicator.id,
                                                paramName,
                                                parseInt(e.currentTarget.value) || defaultValue
                                              )
                                            }
                                            class="flex-1 bg-gray-700 border border-gray-600 rounded px-2 py-1 text-sm text-white focus:border-blue-500 focus:outline-none"
                                            min="1"
                                            max="500"
                                          />
                                        </div>
                                      )}
                                    </For>

                                    {/* 색상 선택 */}
                                    <div class="flex items-center gap-3">
                                      <label class="text-sm text-gray-300 w-24">색상</label>
                                      <div class="flex gap-2">
                                        <For each={defaultColors}>
                                          {(color) => (
                                            <button
                                              class={`w-6 h-6 rounded-full border-2 transition-transform ${
                                                selected()?.color === color
                                                  ? 'border-white scale-110'
                                                  : 'border-transparent hover:scale-105'
                                              }`}
                                              style={{ "background-color": color }}
                                              onClick={() => updateIndicatorColor(indicator.id, color)}
                                            />
                                          )}
                                        </For>
                                      </div>
                                    </div>
                                  </div>
                                </Show>
                              </div>
                            );
                          }}
                        </For>
                      </div>
                    </Show>
                  </div>
                )}
              </For>
            </div>
          );
        }}
      </Show>

      {/* 선택된 지표 요약 */}
      <Show when={selectedIndicators().length > 0}>
        <div class="mt-4 pt-4 border-t border-gray-700">
          <div class="text-sm text-gray-400 mb-2">선택된 지표:</div>
          <div class="flex flex-wrap gap-2">
            <For each={selectedIndicators()}>
              {(indicator) => (
                <span
                  class={`inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs ${
                    indicator.enabled
                      ? 'bg-blue-900/50 text-blue-300'
                      : 'bg-gray-700 text-gray-400'
                  }`}
                >
                  <span
                    class="w-2 h-2 rounded-full"
                    style={{ "background-color": indicator.color }}
                  />
                  {indicator.name}
                  <button
                    class="ml-1 text-gray-400 hover:text-white"
                    onClick={() => toggleIndicator(indicator.info)}
                  >
                    ×
                  </button>
                </span>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}

/** 파라미터 이름을 한글 레이블로 변환 */
function getParamLabel(paramName: string): string {
  const labels: Record<string, string> = {
    period: '기간',
    sma_period: 'SMA 기간',
    ema_period: 'EMA 기간',
    rsi_period: 'RSI 기간',
    fast_period: '단기 EMA',
    slow_period: '장기 EMA',
    signal_period: '시그널',
    bb_period: 'BB 기간',
    std_dev: '표준편차',
    k_period: '%K 기간',
    d_period: '%D 기간',
    atr_period: 'ATR 기간',
  };
  return labels[paramName] || paramName;
}

export default IndicatorSelector;
