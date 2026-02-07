/**
 * 페이지 헤더 컴포넌트
 *
 * 모든 페이지에서 일관된 헤더 레이아웃을 제공합니다.
 * 제목, 설명, 액션 버튼을 포함합니다.
 */
import { type Component, type JSX, Show } from 'solid-js'

export interface PageHeaderProps {
  /** 페이지 제목 */
  title: string
  /** 제목 앞 아이콘 (이모지 또는 컴포넌트) */
  icon?: string | JSX.Element
  /** 설명 텍스트 */
  description?: string
  /** 우측 액션 버튼/컴포넌트 */
  actions?: JSX.Element
  /** 추가 CSS 클래스 */
  className?: string
}

/**
 * 페이지 헤더
 *
 * 모든 페이지 상단에 일관된 형식의 헤더를 표시합니다.
 *
 * @example
 * ```tsx
 * <PageHeader
 *   title="Global Ranking"
 *   icon="🏆"
 *   description="종목 랭킹을 확인합니다."
 *   actions={<Button onClick={refetch}>새로고침</Button>}
 * />
 * ```
 */
export const PageHeader: Component<PageHeaderProps> = (props) => {
  return (
    <div class={`flex items-start justify-between ${props.className || ''}`}>
      <div>
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
          <Show when={props.icon}>
            <span>{typeof props.icon === 'string' ? props.icon : props.icon}</span>
          </Show>
          {props.title}
        </h1>
        <Show when={props.description}>
          <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">{props.description}</p>
        </Show>
      </div>
      <Show when={props.actions}>
        <div class="flex items-center gap-2">{props.actions}</div>
      </Show>
    </div>
  )
}

export default PageHeader
