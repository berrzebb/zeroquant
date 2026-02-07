/**
 * SDUI 렌더러 메인 컴포넌트
 *
 * 백엔드 스키마 기반으로 전략 설정 UI를 자동 생성합니다.
 */
import {
  type Component,
  createSignal,
  createEffect,
  Show,
  For,
  batch,
} from 'solid-js';
import { createStore, reconcile } from 'solid-js/store';
import type {
  StrategyUISchema,
  RenderableSection,
  ValidationErrors,
  StrategyValues,
} from '../../../types/sdui';
import { useStrategySchema, applyDefaults } from '../../../hooks/useStrategySchema';
import { Spinner } from '../../ui/Loading';
import { ErrorState } from '../../ui/StateDisplay';
import { Button } from '../../ui/Form';
import { SDUISection } from './SDUISection';
import { validateAllFields, validateField } from './SDUIValidation';

// ==================== Props ====================

export interface SDUIRendererProps {
  /** 전략 ID (예: "grid", "rsi") */
  strategyId: string;

  /** 초기값 (편집 모드에서 사용) */
  initialValues?: Record<string, unknown>;

  /** 값 변경 콜백 (실시간) */
  onChange?: (values: Record<string, unknown>) => void;

  /** 제출 콜백 */
  onSubmit?: (values: Record<string, unknown>) => void;

  /** 취소 콜백 */
  onCancel?: () => void;

  /** 읽기 전용 모드 */
  readOnly?: boolean;

  /** 제출 버튼 텍스트 */
  submitLabel?: string;

  /** 취소 버튼 텍스트 */
  cancelLabel?: string;

  /** 로딩 메시지 */
  loadingMessage?: string;

  /** 커스텀 클래스 */
  class?: string;
}

// ==================== 컴포넌트 ====================

/**
 * SDUI 렌더러
 *
 * 전략 ID를 받아 백엔드 스키마를 조회하고,
 * Fragment 기반 섹션 UI를 자동으로 렌더링합니다.
 *
 * @example
 * ```tsx
 * // 새 전략 생성
 * <SDUIRenderer
 *   strategyId="grid"
 *   onSubmit={(values) => createStrategy(values)}
 *   onCancel={() => closeModal()}
 * />
 *
 * // 기존 전략 편집
 * <SDUIRenderer
 *   strategyId="grid"
 *   initialValues={existingConfig}
 *   onSubmit={(values) => updateStrategy(values)}
 *   onCancel={() => closeModal()}
 * />
 *
 * // 읽기 전용
 * <SDUIRenderer
 *   strategyId="grid"
 *   initialValues={config}
 *   readOnly
 * />
 * ```
 */
export const SDUIRenderer: Component<SDUIRendererProps> = (props) => {
  // 스키마 조회
  const { schema, sections, loading, error, refetch } = useStrategySchema(
    () => props.strategyId
  );

  // 값 상태 (스토어 사용 - 중첩 객체 최적화)
  const [values, setValues] = createStore<Record<string, unknown>>({});

  // 에러 상태
  const [errors, setErrors] = createStore<ValidationErrors>({});

  // 제출 중 상태
  const [submitting, setSubmitting] = createSignal(false);

  // 스키마 로드 완료 시 초기값 설정
  createEffect(() => {
    const currentSchema = schema();
    const currentSections = sections();

    if (currentSchema && currentSections.length > 0) {
      // 기본값 적용
      const defaults = applyDefaults(currentSections, currentSchema.defaults);

      // 초기값과 병합 (초기값 우선)
      const mergedValues = {
        ...defaults,
        ...(props.initialValues || {}),
      };

      // 값 설정
      setValues(reconcile(mergedValues));
    }
  });

  // 값 변경 시 onChange 콜백 호출
  createEffect(() => {
    if (props.onChange && Object.keys(values).length > 0) {
      props.onChange({ ...values });
    }
  });

  /**
   * 필드 값 변경 핸들러
   */
  const handleFieldChange = (fieldName: string, value: unknown) => {
    batch(() => {
      // 값 업데이트
      setValues(fieldName, value);

      // 실시간 유효성 검증 (해당 필드만)
      const field = findFieldByName(sections(), fieldName);
      if (field) {
        const fieldError = validateField(field, value);
        if (fieldError) {
          setErrors(fieldName, fieldError);
        } else {
          setErrors(fieldName, undefined as unknown as string);
        }
      }
    });
  };

  /**
   * 제출 핸들러
   */
  const handleSubmit = async (e: Event) => {
    e.preventDefault();

    if (!props.onSubmit || props.readOnly) return;

    // 전체 유효성 검증
    const result = validateAllFields(sections(), values);

    if (!result.valid) {
      // 에러 설정
      setErrors(reconcile(result.errors));

      // 첫 번째 에러 필드로 스크롤
      const firstErrorField = Object.keys(result.errors)[0];
      if (firstErrorField) {
        const element = document.getElementById(firstErrorField);
        element?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
      return;
    }

    // 에러 클리어
    setErrors(reconcile({}));

    // 제출
    setSubmitting(true);
    try {
      await props.onSubmit({ ...values });
    } finally {
      setSubmitting(false);
    }
  };

  /**
   * 취소 핸들러
   */
  const handleCancel = () => {
    props.onCancel?.();
  };

  /**
   * 리셋 핸들러
   */
  const handleReset = () => {
    const currentSchema = schema();
    const currentSections = sections();

    if (currentSchema && currentSections.length > 0) {
      const defaults = applyDefaults(currentSections, currentSchema.defaults);
      setValues(reconcile(defaults));
      setErrors(reconcile({}));
    }
  };

  return (
    <div class={`sdui-renderer ${props.class || ''}`}>
      {/* 로딩 상태 */}
      <Show when={loading()}>
        <div class="flex flex-col items-center justify-center py-12">
          <Spinner size="lg" />
          <p class="mt-4 text-gray-500 dark:text-gray-400">
            {props.loadingMessage || '스키마를 불러오는 중...'}
          </p>
        </div>
      </Show>

      {/* 에러 상태 */}
      <Show when={error() && !loading()}>
        <ErrorState
          title="스키마 로드 실패"
          description={error()!}
          action={{
            label: '다시 시도',
            onClick: () => refetch(),
          }}
        />
      </Show>

      {/* 메인 폼 */}
      <Show when={schema() && !loading() && !error()}>
        <form onSubmit={handleSubmit}>
          {/* 전략 헤더 */}
          <div class="mb-6">
            <h2 class="text-xl font-bold text-gray-900 dark:text-white">
              {schema()!.name}
            </h2>
            <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
              {schema()!.description}
            </p>
            <div class="mt-2">
              <span class="inline-block px-2 py-1 text-xs font-medium bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200 rounded">
                {getCategoryLabel(schema()!.category)}
              </span>
            </div>
          </div>

          {/* 섹션 목록 */}
          <div class="space-y-4">
            <For each={sections()}>
              {(section) => (
                <SDUISection
                  section={section}
                  values={values}
                  errors={errors}
                  onChange={handleFieldChange}
                  readOnly={props.readOnly}
                />
              )}
            </For>
          </div>

          {/* 액션 버튼 */}
          <Show when={!props.readOnly}>
            <div class="flex items-center justify-between mt-8 pt-6 border-t border-gray-200 dark:border-gray-700">
              <div>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={handleReset}
                  disabled={submitting()}
                >
                  초기화
                </Button>
              </div>

              <div class="flex gap-3">
                <Show when={props.onCancel}>
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={handleCancel}
                    disabled={submitting()}
                  >
                    {props.cancelLabel || '취소'}
                  </Button>
                </Show>

                <Show when={props.onSubmit}>
                  <Button
                    type="submit"
                    variant="primary"
                    disabled={submitting()}
                  >
                    {submitting() ? (
                      <span class="flex items-center gap-2">
                        <Spinner size="sm" color="white" />
                        저장 중...
                      </span>
                    ) : (
                      props.submitLabel || '저장'
                    )}
                  </Button>
                </Show>
              </div>
            </div>
          </Show>
        </form>
      </Show>
    </div>
  );
};

// ==================== 유틸리티 ====================

/**
 * 필드명으로 필드 찾기
 */
function findFieldByName(
  sections: RenderableSection[],
  fieldName: string
) {
  for (const section of sections) {
    const field = section.fields.find((f) => f.name === fieldName);
    if (field) return field;
  }
  return undefined;
}

/**
 * 카테고리 라벨 (백엔드 StrategyCategory enum 기준)
 */
function getCategoryLabel(category: string): string {
  const labels: Record<string, string> = {
    Realtime: '실시간',
    Intraday: '당일',
    Daily: '스윙',
    Monthly: '장기',
    // 레거시 카테고리 (하위 호환성)
    trend: '추세추종',
    mean_reversion: '평균회귀',
    momentum: '모멘텀',
    volatility: '변동성',
    arbitrage: '차익거래',
    hybrid: '복합',
    ml: '머신러닝',
  };
  return labels[category] || category;
}

export default SDUIRenderer;
