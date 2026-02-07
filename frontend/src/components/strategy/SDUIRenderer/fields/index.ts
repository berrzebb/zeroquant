/**
 * SDUI 필드 컴포넌트 모듈
 *
 * 특수 필드 타입을 위한 컴포넌트들을 제공합니다.
 *
 * Note: SymbolAutocomplete는 SymbolSearch로 대체되어 제거됨
 * - SymbolSearch: 단일/복수 선택, 로컬 필터링, 테마 지원
 * - 사용: import { SymbolSearch } from '@/components/SymbolSearch'
 *
 * Note: MultiTimeframeField는 MultiTimeframeSelector로 통합됨
 * - 통합 컴포넌트: MultiTimeframeSelector (onPrimaryChange/onSecondaryChange 또는 onChange 지원)
 * - 사용: import { MultiTimeframeSelector } from '@/components/strategy'
 */

export {
  MultiSymbolInput,
  type MultiSymbolInputProps,
} from './MultiSymbolInput';

// MultiTimeframeSelector에서 re-export (하위 호환성)
export {
  MultiTimeframeSelector as MultiTimeframeField,
  type MultiTimeframeSelectorProps as MultiTimeframeFieldProps,
  type MultiTimeframeValue,
} from '../../MultiTimeframeSelector';
