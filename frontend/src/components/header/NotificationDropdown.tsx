/**
 * 알림 드롭다운 컴포넌트
 *
 * 최근 알림 히스토리를 표시하고 관리합니다.
 * 헤더의 벨 아이콘 클릭 시 표시됩니다.
 */
import { createSignal, createResource, Show, For, onCleanup } from 'solid-js'
import type { Component, JSX } from 'solid-js'
import { Bell, Check, X, AlertTriangle, TrendingUp, TrendingDown, Info } from 'lucide-solid'
import { getAlertHistory, markAlertAsRead, type AlertHistoryItem } from '../../api/client'

// ==================== 타입 정의 ====================

export interface NotificationDropdownProps {
  /** 추가 클래스 */
  class?: string
  /** 추가 스타일 */
  style?: JSX.CSSProperties
}

// ==================== 유틸리티 함수 ====================

/** 알림 타입에 따른 아이콘 반환 */
function getAlertIcon(type: string): Component<{ class?: string }> {
  switch (type.toUpperCase()) {
    case 'BUY':
    case 'ENTRY':
      return TrendingUp
    case 'SELL':
    case 'EXIT':
      return TrendingDown
    case 'WARNING':
    case 'ERROR':
      return AlertTriangle
    default:
      return Info
  }
}

/** 알림 타입에 따른 색상 반환 */
function getAlertColor(type: string): string {
  switch (type.toUpperCase()) {
    case 'BUY':
    case 'ENTRY':
      return 'text-green-500'
    case 'SELL':
    case 'EXIT':
      return 'text-red-500'
    case 'WARNING':
      return 'text-yellow-500'
    case 'ERROR':
      return 'text-red-600'
    default:
      return 'text-blue-500'
  }
}

/** 시간을 상대적으로 표시 */
function formatRelativeTime(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMin = Math.floor(diffMs / 60000)
  const diffHour = Math.floor(diffMs / 3600000)
  const diffDay = Math.floor(diffMs / 86400000)

  if (diffMin < 1) return '방금'
  if (diffMin < 60) return `${diffMin}분 전`
  if (diffHour < 24) return `${diffHour}시간 전`
  if (diffDay < 7) return `${diffDay}일 전`
  return date.toLocaleDateString('ko-KR')
}

// ==================== 서브 컴포넌트 ====================

/** 단일 알림 아이템 */
const NotificationItem: Component<{
  alert: AlertHistoryItem
  onRead: (id: string) => void
}> = (props) => {
  const Icon = getAlertIcon(props.alert.alertType)
  const colorClass = getAlertColor(props.alert.alertType)

  return (
    <div
      class={`flex items-start gap-3 p-3 hover:bg-[var(--color-surface-light)] rounded-lg transition-colors cursor-pointer ${
        props.alert.status === 'READ' ? 'opacity-60' : ''
      }`}
      onClick={() => props.onRead(props.alert.id)}
    >
      <div class={`flex-shrink-0 p-2 rounded-full bg-[var(--color-surface-light)] ${colorClass}`}>
        <Icon class="w-4 h-4" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2">
          <span class="font-medium text-sm text-[var(--color-text)]">
            {props.alert.symbol || props.alert.strategyId || '알림'}
          </span>
          <Show when={props.alert.status !== 'READ'}>
            <span class="w-2 h-2 bg-blue-500 rounded-full" />
          </Show>
        </div>
        <p class="text-sm text-[var(--color-text-muted)] truncate">
          {props.alert.message}
        </p>
        <span class="text-xs text-[var(--color-text-muted)]">
          {formatRelativeTime(props.alert.sentAt)} · {props.alert.channel}
        </span>
      </div>
    </div>
  )
}

/** 빈 상태 */
const EmptyNotifications: Component = () => (
  <div class="flex flex-col items-center justify-center py-8 text-[var(--color-text-muted)]">
    <Bell class="w-12 h-12 mb-2 opacity-50" />
    <p class="text-sm">새로운 알림이 없습니다</p>
  </div>
)

// ==================== 메인 컴포넌트 ====================

/**
 * 알림 드롭다운
 *
 * 최근 알림 히스토리를 표시합니다.
 * 5분마다 자동 갱신됩니다.
 */
export const NotificationDropdown: Component<NotificationDropdownProps> = (props) => {
  const [isOpen, setIsOpen] = createSignal(false)

  // 알림 히스토리 로드
  const [alerts, { refetch }] = createResource(
    () => true,
    async () => {
      try {
        const response = await getAlertHistory({ limit: 20, offset: 0 })
        return response
      } catch {
        // API가 없거나 에러 시 빈 배열 반환
        return { alerts: [], total: 0, unreadCount: 0 }
      }
    }
  )

  // 5분마다 자동 갱신
  const refreshInterval = setInterval(() => {
    if (!isOpen()) {
      refetch()
    }
  }, 300000)

  onCleanup(() => clearInterval(refreshInterval))

  // 읽지 않은 알림 수
  const unreadCount = () => alerts()?.unreadCount || 0

  // 알림 읽음 처리
  const handleMarkAsRead = async (id: string) => {
    try {
      await markAlertAsRead(id)
      refetch()
    } catch {
      // 에러 무시
    }
  }

  // 드롭다운 외부 클릭 시 닫기
  const handleOutsideClick = (e: MouseEvent) => {
    const target = e.target as HTMLElement
    if (!target.closest('.notification-dropdown')) {
      setIsOpen(false)
    }
  }

  // 외부 클릭 이벤트 등록
  if (typeof document !== 'undefined') {
    document.addEventListener('click', handleOutsideClick)
    onCleanup(() => document.removeEventListener('click', handleOutsideClick))
  }

  return (
    <div class={`relative notification-dropdown ${props.class || ''}`} style={props.style}>
      {/* 벨 버튼 */}
      <button
        class="relative p-2 rounded-lg hover:bg-[var(--color-surface-light)] transition-colors"
        onClick={(e) => {
          e.stopPropagation()
          setIsOpen(!isOpen())
          if (!isOpen()) refetch()
        }}
        aria-label="알림"
      >
        <Bell class="w-5 h-5 text-[var(--color-text-muted)]" />
        <Show when={unreadCount() > 0}>
          <span class="absolute top-1 right-1 min-w-[16px] h-4 px-1 text-xs font-bold text-white bg-red-500 rounded-full flex items-center justify-center">
            {unreadCount() > 99 ? '99+' : unreadCount()}
          </span>
        </Show>
      </button>

      {/* 드롭다운 패널 */}
      <Show when={isOpen()}>
        <div class="absolute right-0 top-full mt-2 w-80 bg-[var(--color-surface)] border border-[var(--color-surface-light)] rounded-xl shadow-xl z-50">
          {/* 헤더 */}
          <div class="flex items-center justify-between px-4 py-3 border-b border-[var(--color-surface-light)]">
            <h3 class="font-semibold text-[var(--color-text)]">알림</h3>
            <Show when={unreadCount() > 0}>
              <span class="text-xs text-[var(--color-text-muted)]">
                {unreadCount()}개 읽지 않음
              </span>
            </Show>
          </div>

          {/* 알림 목록 */}
          <div class="max-h-96 overflow-y-auto">
            <Show
              when={alerts()?.alerts && alerts()!.alerts.length > 0}
              fallback={<EmptyNotifications />}
            >
              <div class="p-2 space-y-1">
                <For each={alerts()?.alerts}>
                  {(alert) => (
                    <NotificationItem
                      alert={alert}
                      onRead={handleMarkAsRead}
                    />
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* 푸터 */}
          <Show when={alerts()?.alerts && alerts()!.alerts.length > 0}>
            <div class="px-4 py-3 border-t border-[var(--color-surface-light)]">
              <a
                href="/settings?tab=notifications"
                class="text-sm text-[var(--color-primary)] hover:underline"
              >
                알림 설정
              </a>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  )
}

export default NotificationDropdown
