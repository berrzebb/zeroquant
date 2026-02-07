import { type JSX, For } from 'solid-js'
import { A, useLocation } from '@solidjs/router'
import {
  LayoutDashboard,
  Bot,
  Settings,
  LogOut,
  Activity,
  FlaskConical,
  Play,
  Brain,
  Database,
  ListFilter,
  BookOpen,
  Trophy,
} from 'lucide-solid'
import { HeaderIndicators, NotificationDropdown } from './header'

interface LayoutProps {
  children?: JSX.Element
}

const navItems = [
  { path: '/', icon: LayoutDashboard, label: '대시보드' },
  { path: '/strategies', icon: Bot, label: '전략' },
  { path: '/backtest', icon: FlaskConical, label: '백테스트' },
  { path: '/simulation', icon: Play, label: '시뮬레이션' },
  { path: '/dataset', icon: Database, label: '데이터셋' },
  { path: '/screening', icon: ListFilter, label: '스크리닝' },
  { path: '/ranking', icon: Trophy, label: '종목 랭킹' },
  { path: '/ml-training', icon: Brain, label: 'ML 훈련' },
  { path: '/journal', icon: BookOpen, label: '매매일지' },
  { path: '/settings', icon: Settings, label: '설정' },
]

export function Layout(props: LayoutProps) {
  const location = useLocation()

  return (
    <div class="flex h-screen bg-[var(--color-background)]">
      {/* Sidebar */}
      <aside class="w-64 bg-[var(--color-surface)] border-r border-[var(--color-surface-light)] flex flex-col">
        {/* Logo */}
        <div class="p-6 border-b border-[var(--color-surface-light)]">
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-lg bg-[var(--color-primary)] flex items-center justify-center">
              <Activity class="w-6 h-6 text-white" />
            </div>
            <div>
              <h1 class="text-lg font-bold text-[var(--color-text)]">ZeroQuant</h1>
              <p class="text-xs text-[var(--color-text-muted)]">퀀트 트레이딩 플랫폼</p>
            </div>
          </div>
        </div>

        {/* Navigation */}
        <nav class="flex-1 p-4">
          <ul class="space-y-2">
            <For each={navItems}>
              {(item) => {
                const Icon = item.icon
                return (
                  <li>
                    <A
                      href={item.path}
                      class={`flex items-center gap-3 px-4 py-3 rounded-lg transition-colors ${
                        location.pathname === item.path
                          ? 'bg-[var(--color-primary)] text-white'
                          : 'text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)]'
                      }`}
                    >
                      <Icon class="w-5 h-5" />
                      <span class="font-medium">{item.label}</span>
                    </A>
                  </li>
                )
              }}
            </For>
          </ul>
        </nav>

        {/* Bottom section */}
        <div class="p-4 border-t border-[var(--color-surface-light)]">
          <button class="flex items-center gap-3 px-4 py-3 w-full rounded-lg text-[var(--color-text-muted)] hover:bg-[var(--color-surface-light)] hover:text-[var(--color-text)] transition-colors">
            <LogOut class="w-5 h-5" />
            <span class="font-medium">로그아웃</span>
          </button>
        </div>
      </aside>

      {/* Main content */}
      <div class="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <header class="h-16 bg-[var(--color-surface)] border-b border-[var(--color-surface-light)] flex items-center justify-between px-6">
          <div class="flex items-center gap-4">
            <h2 class="text-xl font-semibold text-[var(--color-text)]">
              {navItems.find((item) => item.path === location.pathname)?.label || '대시보드'}
            </h2>
          </div>

          <div class="flex items-center gap-4">
            {/* 시장 상태 + 시장 온도 + 매크로 환경 */}
            <HeaderIndicators showTemperature={true} showMacro={true} />

            {/* 알림 드롭다운 */}
            <NotificationDropdown />
          </div>
        </header>

        {/* Page content */}
        <main class="flex-1 overflow-auto p-6">
          {props.children}
        </main>
      </div>
    </div>
  )
}
