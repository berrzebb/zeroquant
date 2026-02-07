/**
 * SDUI 유효성 검증 유틸리티
 *
 * 필드 스키마 기반 유효성 검증 함수들을 제공합니다.
 */
import type {
  FieldSchema,
  FieldType,
  ValidationErrors,
  ValidationResult,
  ParsedCondition,
  ConditionOperator,
  RenderableSection,
} from '../../../types/sdui';
import { createLogger } from '../../../utils/logger';

const { warn } = createLogger('SDUIValidation');

// ==================== 에러 메시지 ====================

const ERROR_MESSAGES = {
  required: '필수 입력 항목입니다.',
  min: (min: number) => `최소 ${min} 이상이어야 합니다.`,
  max: (max: number) => `최대 ${max} 이하이어야 합니다.`,
  invalidNumber: '유효한 숫자를 입력해주세요.',
  invalidInteger: '정수를 입력해주세요.',
  invalidOption: '유효한 옵션을 선택해주세요.',
  invalidSymbol: '유효한 심볼을 입력해주세요.',
  emptyArray: '최소 1개 이상 선택해주세요.',
};

// ==================== 단일 필드 검증 ====================

/**
 * 단일 필드 유효성 검증
 *
 * @param field 필드 스키마
 * @param value 필드 값
 * @returns 에러 메시지 또는 null (유효한 경우)
 *
 * @example
 * ```typescript
 * const error = validateField(
 *   { name: 'rsi_period', field_type: 'integer', min: 1, max: 100, required: true },
 *   14
 * );
 * // null (유효)
 *
 * const error2 = validateField(
 *   { name: 'rsi_period', field_type: 'integer', min: 1, max: 100, required: true },
 *   150
 * );
 * // "최대 100 이하이어야 합니다."
 * ```
 */
export function validateField(
  field: FieldSchema,
  value: unknown
): string | null {
  // 1. required 검증
  if (field.required && isEmpty(value)) {
    return ERROR_MESSAGES.required;
  }

  // 값이 없고 필수가 아니면 통과
  if (isEmpty(value)) {
    return null;
  }

  // 2. 타입별 검증
  switch (field.field_type) {
    case 'integer':
      return validateInteger(value, field);

    case 'number':
      return validateNumber(value, field);

    case 'boolean':
      return validateBoolean(value);

    case 'string':
      return null; // 문자열은 특별한 검증 없음

    case 'select':
      return validateSelect(value, field);

    case 'multi_select':
      return validateMultiSelect(value, field);

    case 'symbol':
      return validateSymbol(value);

    case 'symbols':
      return validateSymbols(value);

    default:
      return null;
  }
}

/**
 * 값이 비어있는지 확인
 */
function isEmpty(value: unknown): boolean {
  if (value === undefined || value === null) return true;
  if (typeof value === 'string' && value.trim() === '') return true;
  if (Array.isArray(value) && value.length === 0) return true;
  return false;
}

/**
 * 정수 검증
 */
function validateInteger(value: unknown, field: FieldSchema): string | null {
  const num = typeof value === 'string' ? parseInt(value, 10) : value;

  if (typeof num !== 'number' || isNaN(num)) {
    return ERROR_MESSAGES.invalidInteger;
  }

  if (!Number.isInteger(num)) {
    return ERROR_MESSAGES.invalidInteger;
  }

  if (field.min !== undefined && num < field.min) {
    return ERROR_MESSAGES.min(field.min);
  }

  if (field.max !== undefined && num > field.max) {
    return ERROR_MESSAGES.max(field.max);
  }

  return null;
}

/**
 * 숫자 검증
 */
function validateNumber(value: unknown, field: FieldSchema): string | null {
  const num = typeof value === 'string' ? parseFloat(value) : value;

  if (typeof num !== 'number' || isNaN(num)) {
    return ERROR_MESSAGES.invalidNumber;
  }

  if (field.min !== undefined && num < field.min) {
    return ERROR_MESSAGES.min(field.min);
  }

  if (field.max !== undefined && num > field.max) {
    return ERROR_MESSAGES.max(field.max);
  }

  return null;
}

/**
 * 불리언 검증
 */
function validateBoolean(value: unknown): string | null {
  if (typeof value !== 'boolean') {
    return '유효한 값이 아닙니다.';
  }
  return null;
}

/**
 * Select 검증
 */
function validateSelect(value: unknown, field: FieldSchema): string | null {
  if (!field.options || field.options.length === 0) {
    return null;
  }

  const validValues = field.options.map((opt) => opt.value);
  if (!validValues.includes(String(value))) {
    return ERROR_MESSAGES.invalidOption;
  }

  return null;
}

/**
 * MultiSelect 검증
 */
function validateMultiSelect(value: unknown, field: FieldSchema): string | null {
  if (!Array.isArray(value)) {
    return '배열 형태여야 합니다.';
  }

  if (value.length === 0 && field.required) {
    return ERROR_MESSAGES.emptyArray;
  }

  if (!field.options || field.options.length === 0) {
    return null;
  }

  const validValues = field.options.map((opt) => opt.value);
  for (const v of value) {
    if (!validValues.includes(String(v))) {
      return ERROR_MESSAGES.invalidOption;
    }
  }

  return null;
}

/**
 * Symbol 검증
 */
function validateSymbol(value: unknown): string | null {
  if (typeof value !== 'string' || value.trim() === '') {
    return ERROR_MESSAGES.invalidSymbol;
  }
  return null;
}

/**
 * Symbols (다중 심볼) 검증
 */
function validateSymbols(value: unknown): string | null {
  if (!Array.isArray(value)) {
    return '배열 형태여야 합니다.';
  }

  for (const v of value) {
    if (typeof v !== 'string' || v.trim() === '') {
      return ERROR_MESSAGES.invalidSymbol;
    }
  }

  return null;
}

// ==================== 전체 필드 검증 ====================

/**
 * 모든 필드 유효성 검증
 *
 * @param sections 섹션 목록
 * @param values 값 맵
 * @returns 검증 결과 (valid + errors)
 *
 * @example
 * ```typescript
 * const result = validateAllFields(sections, values);
 * if (!result.valid) {
 *   console.log('Errors:', result.errors);
 * }
 * ```
 */
export function validateAllFields(
  sections: RenderableSection[],
  values: Record<string, unknown>
): ValidationResult {
  const errors: ValidationErrors = {};
  let valid = true;

  for (const section of sections) {
    for (const field of section.fields) {
      // 조건부 필드는 조건이 충족될 때만 검증
      if (field.condition) {
        if (!evaluateCondition(field.condition, values)) {
          continue;
        }
      }

      const error = validateField(field, values[field.name]);
      if (error) {
        errors[field.name] = error;
        valid = false;
      }
    }
  }

  return { valid, errors };
}

// ==================== 조건 평가 ====================

/**
 * 조건식 평가
 *
 * @param condition 조건식 문자열 (예: "enable_trailing_stop == true")
 * @param values 값 맵
 * @returns 조건 충족 여부
 *
 * @example
 * ```typescript
 * evaluateCondition("position_sizing_method == 'kelly'", { position_sizing_method: 'kelly' });
 * // true
 *
 * evaluateCondition("enable_stop_loss == true", { enable_stop_loss: false });
 * // false
 * ```
 */
export function evaluateCondition(
  condition: string,
  values: Record<string, unknown>
): boolean {
  try {
    const parsed = parseCondition(condition);
    if (!parsed) return true; // 파싱 실패 시 기본 표시

    const fieldValue = values[parsed.field];
    return compareValues(fieldValue, parsed.operator, parsed.value);
  } catch {
    // 파싱/평가 실패 시 기본 표시
    warn(`Failed to evaluate condition: ${condition}`);
    return true;
  }
}

/**
 * 조건식 파싱
 *
 * 지원 형식:
 * - "field == value"
 * - "field != value"
 * - "field > value"
 * - "field < value"
 * - "field >= value"
 * - "field <= value"
 */
function parseCondition(condition: string): ParsedCondition | null {
  // 정규식: field operator value
  const operators: ConditionOperator[] = ['==', '!=', '>=', '<=', '>', '<'];

  for (const op of operators) {
    const parts = condition.split(op);
    if (parts.length === 2) {
      const field = parts[0].trim();
      let value: unknown = parts[1].trim();

      // 값 타입 변환
      value = parseValue(value as string);

      return { field, operator: op, value };
    }
  }

  return null;
}

/**
 * 문자열 값을 적절한 타입으로 변환
 */
function parseValue(valueStr: string): unknown {
  // 문자열 리터럴 (따옴표로 감싸진 경우)
  if ((valueStr.startsWith("'") && valueStr.endsWith("'")) ||
      (valueStr.startsWith('"') && valueStr.endsWith('"'))) {
    return valueStr.slice(1, -1);
  }

  // 불리언
  if (valueStr === 'true') return true;
  if (valueStr === 'false') return false;

  // null/undefined
  if (valueStr === 'null') return null;
  if (valueStr === 'undefined') return undefined;

  // 숫자
  const num = parseFloat(valueStr);
  if (!isNaN(num)) return num;

  // 기본: 문자열
  return valueStr;
}

/**
 * 값 비교
 */
function compareValues(
  fieldValue: unknown,
  operator: ConditionOperator,
  targetValue: unknown
): boolean {
  switch (operator) {
    case '==':
      return fieldValue === targetValue;
    case '!=':
      return fieldValue !== targetValue;
    case '>':
      return (fieldValue as number) > (targetValue as number);
    case '<':
      return (fieldValue as number) < (targetValue as number);
    case '>=':
      return (fieldValue as number) >= (targetValue as number);
    case '<=':
      return (fieldValue as number) <= (targetValue as number);
    default:
      return true;
  }
}

// ==================== 유틸리티 ====================

/**
 * 필드 타입에 따른 기본 값 반환
 */
export function getDefaultValueForType(fieldType: FieldType): unknown {
  switch (fieldType) {
    case 'integer':
    case 'number':
      return 0;
    case 'boolean':
      return false;
    case 'string':
    case 'symbol':
      return '';
    case 'select':
      return '';
    case 'multi_select':
    case 'symbols':
      return [];
    default:
      return null;
  }
}

/**
 * 값을 필드 타입에 맞게 변환
 */
export function coerceValue(
  value: unknown,
  fieldType: FieldType
): unknown {
  if (value === undefined || value === null) {
    return getDefaultValueForType(fieldType);
  }

  switch (fieldType) {
    case 'integer':
      return typeof value === 'string' ? parseInt(value, 10) : Math.floor(Number(value));

    case 'number':
      return typeof value === 'string' ? parseFloat(value) : Number(value);

    case 'boolean':
      return Boolean(value);

    case 'string':
    case 'symbol':
      return String(value);

    case 'select':
      return String(value);

    case 'multi_select':
    case 'symbols':
      return Array.isArray(value) ? value : [];

    default:
      return value;
  }
}
