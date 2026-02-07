/**
 * SDUI (Server-Driven UI) 타입 정의
 *
 * ts-rs 자동 생성 타입을 re-export합니다.
 * 하위 호환성을 위해 유지됩니다.
 */

// ts-rs 자동 생성 타입 re-export
export type {
  FieldSchema,
  FieldType,
  FragmentCategory,
  FragmentRef,
  SchemaFragment,
  StrategyUISchema,
} from './generated/sdui';

// 프론트엔드 전용 타입 (백엔드와 독립적)
import type { FieldSchema } from './generated/sdui';

/**
 * 렌더링용 섹션 정보 (Fragment + fields 결합)
 *
 * SDUISection 컴포넌트에서 사용합니다.
 */
export interface RenderableSection {
  /** 섹션 ID (Fragment ID 또는 'custom') */
  id: string;
  /** 섹션 이름 */
  name: string;
  /** 섹션 설명 */
  description?: string | null;
  /** 필수 여부 (접기 불가) */
  required: boolean;
  /** 접힘 가능 여부 */
  collapsible: boolean;
  /** 포함된 필드 목록 */
  fields: FieldSchema[];
  /** 표시 순서 */
  order: number;
}

// ==================== 유효성 검증 타입 ====================

/**
 * 필드별 에러 맵
 */
export type ValidationErrors = Record<string, string>;

/**
 * 유효성 검증 결과
 */
export interface ValidationResult {
  /** 유효 여부 */
  valid: boolean;

  /** 에러 맵 (필드명 → 에러 메시지) */
  errors: ValidationErrors;
}

// ==================== 조건 평가 타입 ====================

/**
 * 조건 연산자
 */
export type ConditionOperator = '==' | '!=' | '>' | '<' | '>=' | '<=';

/**
 * 파싱된 조건
 */
export interface ParsedCondition {
  /** 참조 필드명 */
  field: string;

  /** 연산자 */
  operator: ConditionOperator;

  /** 비교 값 */
  value: unknown;
}

// ==================== 유틸리티 타입 ====================

/**
 * 필드 값 타입 (field_type에 따라 달라짐)
 */
export type FieldValue =
  | number      // integer, number
  | boolean    // boolean
  | string     // string, select, symbol
  | string[];  // multi_select, symbols

/**
 * 전략 설정 값 맵
 */
export type StrategyValues = Record<string, FieldValue | unknown>;

/**
 * 필드 변경 이벤트
 */
export interface FieldChangeEvent {
  name: string;
  value: unknown;
  valid: boolean;
}
