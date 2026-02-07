/**
 * 다중 타임프레임 선택 컴포넌트 (통합 버전)
 *
 * 전략 설정에서 Primary 타임프레임과 Secondary 타임프레임을 선택하는 UI를 제공합니다.
 * - 일반 사용: onPrimaryChange, onSecondaryChange 콜백
 * - SDUI 사용: onChange 콜백 (MultiTimeframeValue 객체)
 *
 * @example
 * ```tsx
 * // 일반 사용 (모달, 페이지)
 * <MultiTimeframeSelector
 *   primaryTimeframe="5m"
 *   secondaryTimeframes={["1h", "1d"]}
 *   onPrimaryChange={(tf) => setPrimary(tf)}
 *   onSecondaryChange={(tfs) => setSecondary(tfs)}
 * />
 *
 * // SDUI 사용 (폼 필드)
 * <MultiTimeframeSelector
 *   primaryTimeframe={value.primary}
 *   secondaryTimeframes={value.secondary}
 *   onChange={(v) => setValue(v)}
 * />
 * ```
 */

import {
  type Component,
  createMemo,
  For,
  Show,
} from 'solid-js';
import type { Timeframe } from '../../api/client';

// ==================== 타입 ====================

/** 다중 타임프레임 설정 값 (SDUI용) */
export interface MultiTimeframeValue {
  /** Primary 타임프레임 (전략 실행 기준) */
  primary: Timeframe;
  /** Secondary 타임프레임 목록 (추세 확인용) */
  secondary: Timeframe[];
}

export interface MultiTimeframeSelectorProps {
  /** Primary 타임프레임 */
  primaryTimeframe: Timeframe;
  /** Secondary 타임프레임 목록 */
  secondaryTimeframes: Timeframe[];
  /** Primary 변경 콜백 (일반 사용) */
  onPrimaryChange?: (tf: Timeframe) => void;
  /** Secondary 변경 콜백 (일반 사용) */
  onSecondaryChange?: (tfs: Timeframe[]) => void;
  /** 통합 변경 콜백 (SDUI 사용) */
  onChange?: (value: MultiTimeframeValue) => void;
  /** 최대 Secondary 타임프레임 수 */
  maxSecondary?: number;
  /** 비활성화 여부 */
  disabled?: boolean;
  /** 읽기 전용 */
  readOnly?: boolean;
  /** 에러 상태 */
  hasError?: boolean;
  /** HTML id */
  id?: string;
}

// ==================== 상수 ====================

/** 지원되는 타임프레임 (작은 것부터 큰 것 순서) */
const TIMEFRAMES: Array<{ value: Timeframe; label: string; minutes: number }> = [
  { value: '1m', label: '1분', minutes: 1 },
  { value: '5m', label: '5분', minutes: 5 },
  { value: '15m', label: '15분', minutes: 15 },
  { value: '30m', label: '30분', minutes: 30 },
  { value: '1h', label: '1시간', minutes: 60 },
  { value: '4h', label: '4시간', minutes: 240 },
  { value: '1d', label: '일봉', minutes: 1440 },
  { value: '1w', label: '주봉', minutes: 10080 },
  { value: '1M', label: '월봉', minutes: 43200 },
];

/** 타임프레임 라벨 맵 */
const TIMEFRAME_LABELS: Record<Timeframe, string> = {
  '1m': '1분',
  '5m': '5분',
  '15m': '15분',
  '30m': '30분',
  '1h': '1시간',
  '4h': '4시간',
  '1d': '일봉',
  '1w': '주봉',
  '1M': '월봉',
};

// ==================== 컴포넌트 ====================

/**
 * 다중 타임프레임 선택 컴포넌트
 *
 * Primary 타임프레임보다 큰 타임프레임만 Secondary로 선택 가능합니다.
 */
export const MultiTimeframeSelector: Component<MultiTimeframeSelectorProps> = (props) => {
  const maxSecondary = () => props.maxSecondary ?? 3;
  const isDisabled = () => props.disabled || props.readOnly;

  // Primary 타임프레임의 분 단위 값
  const primaryMinutes = createMemo(() => {
    const tf = TIMEFRAMES.find((t) => t.value === props.primaryTimeframe);
    return tf?.minutes ?? 5;
  });

  // Secondary로 선택 가능한 타임프레임 (Primary보다 큰 것만)
  const availableSecondary = createMemo(() => {
    return TIMEFRAMES.filter((tf) => tf.minutes > primaryMinutes());
  });

  // Primary 변경 핸들러
  const handlePrimaryChange = (value: Timeframe) => {
    if (isDisabled()) return;

    const newPrimaryMinutes = TIMEFRAMES.find((t) => t.value === value)?.minutes ?? 5;

    // Secondary 중 새 Primary보다 작거나 같은 것은 제거
    const validSecondary = props.secondaryTimeframes.filter((s) => {
      const sMinutes = TIMEFRAMES.find((t) => t.value === s)?.minutes ?? 0;
      return sMinutes > newPrimaryMinutes;
    });

    // 콜백 호출
    if (props.onPrimaryChange) {
      props.onPrimaryChange(value);
      // Secondary가 변경되면 onSecondaryChange도 호출
      if (validSecondary.length !== props.secondaryTimeframes.length && props.onSecondaryChange) {
        props.onSecondaryChange(validSecondary);
      }
    }
    if (props.onChange) {
      props.onChange({ primary: value, secondary: validSecondary });
    }
  };

  // Secondary 토글 핸들러
  const handleSecondaryToggle = (value: Timeframe) => {
    if (isDisabled()) return;

    const current = new Set(props.secondaryTimeframes);
    let newSecondary: Timeframe[];

    if (current.has(value)) {
      current.delete(value);
      newSecondary = Array.from(current) as Timeframe[];
    } else if (current.size < maxSecondary()) {
      current.add(value);
      // 타임프레임 순서대로 정렬
      newSecondary = Array.from(current).sort((a, b) => {
        const aMin = TIMEFRAMES.find((t) => t.value === a)?.minutes ?? 0;
        const bMin = TIMEFRAMES.find((t) => t.value === b)?.minutes ?? 0;
        return aMin - bMin;
      }) as Timeframe[];
    } else {
      return; // 최대 개수 도달
    }

    // 콜백 호출
    if (props.onSecondaryChange) {
      props.onSecondaryChange(newSecondary);
    }
    if (props.onChange) {
      props.onChange({ primary: props.primaryTimeframe, secondary: newSecondary });
    }
  };

  return (
    <div
      id={props.id}
      class={`
        space-y-4 rounded-lg border p-4
        ${props.hasError ? 'border-red-500' : 'border-gray-700'}
        ${isDisabled() ? 'opacity-60' : ''}
      `}
    >
      {/* Primary 타임프레임 선택 */}
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-2">
          Primary 타임프레임
          <span class="ml-1 text-xs text-gray-500">(전략 실행 기준)</span>
        </label>
        <div class="flex flex-wrap gap-2">
          <For each={TIMEFRAMES}>
            {(tf) => (
              <button
                type="button"
                onClick={() => handlePrimaryChange(tf.value)}
                disabled={isDisabled()}
                class={`
                  px-3 py-1.5 rounded-lg text-sm font-medium transition-colors
                  ${props.primaryTimeframe === tf.value
                    ? 'bg-blue-600 text-white'
                    : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                  }
                  ${isDisabled() ? 'cursor-not-allowed' : 'cursor-pointer'}
                `}
              >
                {tf.label}
              </button>
            )}
          </For>
        </div>
        <p class="mt-1 text-xs text-gray-400">
          신호 생성 및 백테스트의 기준이 되는 타임프레임
        </p>
      </div>

      {/* Secondary 타임프레임 선택 */}
      <div>
        <label class="block text-sm font-medium text-gray-300 mb-2">
          Secondary 타임프레임
          <span class="ml-1 text-xs text-gray-500">(추세 확인용, 최대 {maxSecondary()}개)</span>
        </label>

        <Show
          when={availableSecondary().length > 0}
          fallback={
            <p class="text-sm text-gray-400">
              가장 큰 타임프레임이 선택되어 Secondary를 추가할 수 없습니다.
            </p>
          }
        >
          <div class="flex flex-wrap gap-2">
            <For each={availableSecondary()}>
              {(tf) => {
                const isSelected = () => props.secondaryTimeframes.includes(tf.value);
                const cannotAdd = () =>
                  !isSelected() && props.secondaryTimeframes.length >= maxSecondary();

                return (
                  <button
                    type="button"
                    onClick={() => handleSecondaryToggle(tf.value)}
                    disabled={isDisabled() || cannotAdd()}
                    title={cannotAdd() ? `최대 ${maxSecondary()}개까지만 선택할 수 있습니다` : undefined}
                    class={`
                      px-3 py-1.5 rounded-lg text-sm font-medium transition-colors
                      ${isSelected()
                        ? 'bg-green-600 text-white'
                        : 'bg-gray-700 text-gray-300 hover:bg-gray-600'
                      }
                      ${isDisabled() || cannotAdd()
                        ? 'opacity-50 cursor-not-allowed'
                        : 'cursor-pointer'
                      }
                    `}
                  >
                    {tf.label}
                  </button>
                );
              }}
            </For>
          </div>
        </Show>

        <p class="mt-2 text-xs text-gray-400">
          추세 확인 및 필터링에 사용되는 상위 타임프레임
        </p>
      </div>

      {/* 선택된 타임프레임 요약 */}
      <Show when={props.secondaryTimeframes.length > 0}>
        <div class="p-3 bg-gray-800 rounded-lg border border-gray-700">
          <h4 class="text-sm font-medium text-gray-300 mb-2">
            다중 타임프레임 구성
          </h4>
          <div class="flex items-center gap-2 text-sm flex-wrap">
            <span class="px-2 py-1 bg-green-600/20 text-green-400 rounded">
              Primary: {TIMEFRAME_LABELS[props.primaryTimeframe] || props.primaryTimeframe}
            </span>
            <span class="text-gray-500">+</span>
            <For each={props.secondaryTimeframes}>
              {(s) => (
                <span class="px-2 py-1 bg-blue-600/20 text-blue-400 rounded">
                  {TIMEFRAME_LABELS[s] || s}
                </span>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default MultiTimeframeSelector;
