import { createSignal, createContext, useContext, For, Show, type JSX } from 'solid-js'
import { X, CheckCircle, AlertCircle, Info, AlertTriangle } from 'lucide-solid'

// Toast 타입 정의
export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface Toast {
  id: string
  type: ToastType
  title: string
  message?: string
  duration?: number // ms, 0 = 무한
}

interface ToastContextType {
  toasts: () => Toast[]
  addToast: (toast: Omit<Toast, 'id'>) => string
  removeToast: (id: string) => void
  success: (title: string, message?: string) => string
  error: (title: string, message?: string) => string
  warning: (title: string, message?: string) => string
  info: (title: string, message?: string) => string
}

const ToastContext = createContext<ToastContextType>()

// Toast Provider
export function ToastProvider(props: { children: JSX.Element }) {
  const [toasts, setToasts] = createSignal<Toast[]>([])

  const generateId = () => `toast-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`

  const addToast = (toast: Omit<Toast, 'id'>): string => {
    const id = generateId()
    const newToast: Toast = {
      id,
      duration: 5000, // 기본 5초
      ...toast,
    }

    setToasts((prev) => [...prev, newToast])

    // 자동 제거 (duration이 0이 아닌 경우)
    if (newToast.duration && newToast.duration > 0) {
      setTimeout(() => {
        removeToast(id)
      }, newToast.duration)
    }

    return id
  }

  const removeToast = (id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id))
  }

  const success = (title: string, message?: string) => addToast({ type: 'success', title, message })
  const error = (title: string, message?: string) => addToast({ type: 'error', title, message, duration: 8000 })
  const warning = (title: string, message?: string) => addToast({ type: 'warning', title, message, duration: 6000 })
  const info = (title: string, message?: string) => addToast({ type: 'info', title, message })

  const contextValue: ToastContextType = {
    toasts,
    addToast,
    removeToast,
    success,
    error,
    warning,
    info,
  }

  return (
    <ToastContext.Provider value={contextValue}>
      {props.children}
      <ToastContainer />
    </ToastContext.Provider>
  )
}

// Toast 사용을 위한 훅
export function useToast() {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider')
  }
  return context
}

// 개별 Toast 컴포넌트
function ToastItem(props: { toast: Toast; onClose: () => void }) {
  const [isExiting, setIsExiting] = createSignal(false)

  const handleClose = () => {
    setIsExiting(true)
    setTimeout(() => {
      props.onClose()
    }, 200) // 애니메이션 시간
  }

  const iconMap = {
    success: CheckCircle,
    error: AlertCircle,
    warning: AlertTriangle,
    info: Info,
  }

  const colorMap = {
    success: {
      bg: 'bg-green-500/10',
      border: 'border-green-500/30',
      icon: 'text-green-500',
      title: 'text-green-400',
    },
    error: {
      bg: 'bg-red-500/10',
      border: 'border-red-500/30',
      icon: 'text-red-500',
      title: 'text-red-400',
    },
    warning: {
      bg: 'bg-yellow-500/10',
      border: 'border-yellow-500/30',
      icon: 'text-yellow-500',
      title: 'text-yellow-400',
    },
    info: {
      bg: 'bg-blue-500/10',
      border: 'border-blue-500/30',
      icon: 'text-blue-500',
      title: 'text-blue-400',
    },
  }

  const Icon = iconMap[props.toast.type]
  const colors = colorMap[props.toast.type]

  return (
    <div
      class={`
        flex items-start gap-3 p-4 rounded-xl border backdrop-blur-sm
        shadow-lg shadow-black/20
        ${colors.bg} ${colors.border}
        ${isExiting() ? 'animate-slide-out' : 'animate-slide-in'}
        transition-all duration-200
      `}
      role="alert"
    >
      <div class={`flex-shrink-0 ${colors.icon}`}>
        <Icon class="w-5 h-5" />
      </div>
      <div class="flex-1 min-w-0">
        <p class={`font-medium ${colors.title}`}>{props.toast.title}</p>
        <Show when={props.toast.message}>
          <p class="mt-1 text-sm text-[var(--color-text-muted)]">{props.toast.message}</p>
        </Show>
      </div>
      <button
        onClick={handleClose}
        class="flex-shrink-0 p-1 rounded-lg hover:bg-white/10 transition-colors text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
      >
        <X class="w-4 h-4" />
      </button>
    </div>
  )
}

// Toast 컨테이너 (화면 우측 상단에 표시)
function ToastContainer() {
  const context = useContext(ToastContext)
  if (!context) return null

  return (
    <div class="fixed top-4 right-4 z-[100] flex flex-col gap-3 w-full max-w-sm pointer-events-none">
      <For each={context.toasts()}>
        {(toast) => (
          <div class="pointer-events-auto">
            <ToastItem toast={toast} onClose={() => context.removeToast(toast.id)} />
          </div>
        )}
      </For>
    </div>
  )
}
