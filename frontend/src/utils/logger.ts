/**
 * 프로덕션 최적화된 로깅 유틸리티
 *
 * 개발 모드에서만 로그를 출력하고, 프로덕션에서는 에러만 출력합니다.
 * 각 모듈별로 prefix가 자동으로 추가되어 디버깅이 용이합니다.
 *
 * @example
 * ```ts
 * import { createLogger } from '../utils/logger'
 * const { log, warn, error } = createLogger('MyComponent')
 *
 * log('데이터 로드 완료', data)  // [MyComponent] 데이터 로드 완료 ...
 * warn('캐시 만료됨')           // [MyComponent] 캐시 만료됨
 * error('API 호출 실패:', err)  // [MyComponent] API 호출 실패: ...
 * ```
 */

// 개발 모드 여부
const isDev = import.meta.env.DEV

// no-op 함수 (프로덕션에서 로그 비활성화)
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const noop = (..._args: unknown[]) => {}

export interface Logger {
  /** 일반 로그 (개발 모드에서만 출력) */
  log: typeof console.log
  /** 경고 로그 (개발 모드에서만 출력) */
  warn: typeof console.warn
  /** 에러 로그 (항상 출력) */
  error: typeof console.error
}

/**
 * 모듈별 로거 생성
 *
 * @param prefix - 로그 메시지 앞에 붙는 모듈 이름 (예: 'WebSocket', 'Backtest')
 * @returns Logger 객체 (log, warn, error 메서드)
 */
export function createLogger(prefix: string): Logger {
  const tag = `[${prefix}]`

  return {
    log: isDev ? console.log.bind(console, tag) : noop,
    warn: isDev ? console.warn.bind(console, tag) : noop,
    error: console.error.bind(console, tag), // 에러는 항상 출력
  }
}

// 기본 로거 (모듈 prefix 없음, 범용)
export const defaultLogger: Logger = {
  log: isDev ? console.log.bind(console) : noop,
  warn: isDev ? console.warn.bind(console) : noop,
  error: console.error.bind(console),
}
