/**
 * SDUI 섹션 컴포넌트
 *
 * Fragment 기반 섹션을 렌더링합니다.
 * 접힘/펼침 기능과 조건부 필드 표시를 지원합니다.
 */
import {
  type Component,
  createSignal,
  Show,
  For,
  createMemo,
} from 'solid-js';
import { ChevronDown, ChevronRight } from 'lucide-solid';
import type { RenderableSection, ValidationErrors } from '../../../types/sdui';
import { Card, CardHeader, CardContent } from '../../ui';
import { SDUIField } from './SDUIField';
import { evaluateCondition } from './SDUIValidation';

// ==================== Props ====================

export interface SDUISectionProps {
  /** 섹션 데이터 */
  section: RenderableSection;

  /** 값 맵 */
  values: Record<string, unknown>;

  /** 에러 맵 */
  errors: ValidationErrors;

  /** 값 변경 핸들러 */
  onChange: (fieldName: string, value: unknown) => void;

  /** 읽기 전용 */
  readOnly?: boolean;

  /** 초기 접힘 상태 (기본: required가 아니면 접힘) */
  initialCollapsed?: boolean;
}

// ==================== 컴포넌트 ====================

/**
 * SDUI 섹션 컴포넌트
 *
 * @example
 * ```tsx
 * <SDUISection
 *   section={section}
 *   values={values()}
 *   errors={errors()}
 *   onChange={handleFieldChange}
 * />
 * ```
 */
export const SDUISection: Component<SDUISectionProps> = (props) => {
  // 접힘 상태
  const [collapsed, setCollapsed] = createSignal(
    props.initialCollapsed ?? !props.section.required
  );

  // 섹션의 필수 여부에 따른 접힘 가능 여부
  const canCollapse = createMemo(() => props.section.collapsible);

  // 조건부 및 hidden 필드 필터링
  const visibleFields = createMemo(() => {
    return props.section.fields.filter((field) => {
      // hidden 필드는 표시하지 않음
      if (field.hidden) return false;
      // 조건이 없으면 표시
      if (!field.condition) return true;
      // 조건 평가
      return evaluateCondition(field.condition, props.values);
    });
  });

  // 섹션에 에러가 있는지 확인
  const hasErrors = createMemo(() => {
    return visibleFields().some((field) => props.errors[field.name]);
  });

  // 토글 핸들러
  const toggleCollapsed = () => {
    if (canCollapse()) {
      setCollapsed(!collapsed());
    }
  };

  return (
    <Card class="mb-4">
      {/* 섹션 헤더 */}
      <CardHeader
        class={`
          ${canCollapse() ? 'cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-700' : ''}
          ${hasErrors() ? 'border-l-4 border-l-red-500' : ''}
        `}
        onClick={toggleCollapsed}
      >
        <div class="flex items-center justify-between">
          {/* 제목 영역 */}
          <div class="flex items-center gap-2">
            {/* 접힘 아이콘 */}
            <Show when={canCollapse()}>
              <span class="text-gray-400">
                <Show when={collapsed()} fallback={<ChevronDown class="w-5 h-5" />}>
                  <ChevronRight class="w-5 h-5" />
                </Show>
              </span>
            </Show>

            {/* 제목 */}
            <h3 class="text-lg font-medium text-gray-900 dark:text-white">
              {props.section.name}
            </h3>

            {/* 필수 마크 */}
            <Show when={props.section.required}>
              <span class="text-xs text-red-500 font-medium">필수</span>
            </Show>

            {/* 에러 인디케이터 */}
            <Show when={hasErrors() && collapsed()}>
              <span class="text-xs text-red-500 bg-red-100 dark:bg-red-900 px-2 py-0.5 rounded">
                입력 오류
              </span>
            </Show>
          </div>

          {/* 필드 카운트 */}
          <span class="text-sm text-gray-500">
            {visibleFields().length}개 필드
          </span>
        </div>

        {/* 설명 */}
        <Show when={props.section.description && !collapsed()}>
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
            {props.section.description}
          </p>
        </Show>
      </CardHeader>

      {/* 섹션 내용 (접히지 않았을 때만 표시) */}
      <Show when={!collapsed()}>
        <CardContent>
          <div class="grid gap-4 sm:grid-cols-1 md:grid-cols-2">
            <For each={visibleFields()}>
              {(field) => (
                <SDUIField
                  field={field}
                  value={props.values[field.name]}
                  error={props.errors[field.name]}
                  onChange={(value) => props.onChange(field.name, value)}
                  readOnly={props.readOnly}
                />
              )}
            </For>
          </div>
        </CardContent>
      </Show>
    </Card>
  );
};

export default SDUISection;
