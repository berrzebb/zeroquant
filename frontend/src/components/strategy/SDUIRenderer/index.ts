/**
 * SDUI 렌더러 모듈
 *
 * Server-Driven UI 기반 전략 설정 폼을 자동 생성합니다.
 */

// 메인 컴포넌트
export { SDUIRenderer, type SDUIRendererProps } from './SDUIRenderer';

// 하위 컴포넌트
export { SDUISection, type SDUISectionProps } from './SDUISection';
export { SDUIField, type SDUIFieldProps } from './SDUIField';

// 유효성 검증 유틸리티
export {
  validateField,
  validateAllFields,
  evaluateCondition,
  getDefaultValueForType,
  coerceValue,
} from './SDUIValidation';

// 기본 export
export { default } from './SDUIRenderer';
