/**
 * Card 컴포넌트
 *
 * 재사용 가능한 카드 레이아웃 컴포넌트입니다.
 */
import { type Component, type JSX, createMemo } from 'solid-js'

interface CardProps {
  children: JSX.Element
  className?: string
  variant?: 'default' | 'outlined' | 'elevated'
  padding?: 'none' | 'sm' | 'md' | 'lg'
  onClick?: () => void
}

const variantStyles = {
  default: 'bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
  outlined: 'bg-transparent border-2 border-gray-300 dark:border-gray-600',
  elevated: 'bg-white dark:bg-gray-800 shadow-lg border border-gray-100 dark:border-gray-700',
}

const paddingStyles = {
  none: '',
  sm: 'p-3',
  md: 'p-4',
  lg: 'p-6',
}

export const Card: Component<CardProps> = (props) => {
  const variant = createMemo(() => variantStyles[props.variant || 'default'])
  const padding = createMemo(() => paddingStyles[props.padding || 'md'])

  return (
    <div
      class={`
        rounded-lg ${variant()} ${padding()}
        ${props.onClick ? 'cursor-pointer hover:border-blue-300 dark:hover:border-blue-600 transition-colors' : ''}
        ${props.className || ''}
      `}
      onClick={props.onClick}
    >
      {props.children}
    </div>
  )
}

interface CardHeaderProps {
  children: JSX.Element
  className?: string
  class?: string
  action?: JSX.Element
  onClick?: () => void
}

export const CardHeader: Component<CardHeaderProps> = (props) => {
  return (
    <div
      class={`flex items-center justify-between mb-4 ${props.className || ''} ${props.class || ''}`}
      onClick={props.onClick}
    >
      <div class="flex-1">{props.children}</div>
      {props.action && <div class="ml-4">{props.action}</div>}
    </div>
  )
}

interface CardContentProps {
  children: JSX.Element
  className?: string
}

export const CardContent: Component<CardContentProps> = (props) => {
  return <div class={props.className || ''}>{props.children}</div>
}

interface CardFooterProps {
  children: JSX.Element
  className?: string
  align?: 'left' | 'center' | 'right' | 'between'
}

const alignStyles = {
  left: 'justify-start',
  center: 'justify-center',
  right: 'justify-end',
  between: 'justify-between',
}

export const CardFooter: Component<CardFooterProps> = (props) => {
  const align = createMemo(() => alignStyles[props.align || 'right'])

  return (
    <div class={`flex items-center ${align()} mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 ${props.className || ''}`}>
      {props.children}
    </div>
  )
}

export default Card
