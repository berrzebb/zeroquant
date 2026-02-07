import { Router, Route } from '@solidjs/router'
import { QueryClient, QueryClientProvider } from '@tanstack/solid-query'
import { lazy, Suspense } from 'solid-js'
import { Layout } from './components/Layout'
import { ToastProvider } from './components/Toast'

// ==================== Lazy Loading 페이지 ====================
// 코드 스플리팅으로 초기 번들 크기를 줄이고, 필요한 페이지만 로드합니다.

// 메인 페이지들
const Dashboard = lazy(() => import('./pages/Dashboard'))
const Strategies = lazy(() => import('./pages/Strategies'))
const Backtest = lazy(() => import('./pages/Backtest'))
const Simulation = lazy(() => import('./pages/Simulation'))
const Dataset = lazy(() => import('./pages/Dataset'))
const Screening = lazy(() => import('./pages/Screening'))
const TradingJournal = lazy(() => import('./pages/TradingJournal'))
const Settings = lazy(() => import('./pages/Settings'))
const MLTraining = lazy(() => import('./pages/MLTraining'))

// 상세/부가 페이지
const GlobalRanking = lazy(() => import('./pages/GlobalRanking'))
const SymbolDetail = lazy(() => import('./pages/SymbolDetail'))

// ==================== 로딩 컴포넌트 ====================

/** 페이지 로딩 중 표시되는 스피너 */
function PageLoader() {
  return (
    <div class="flex items-center justify-center h-[calc(100vh-4rem)]">
      <div class="flex flex-col items-center gap-4">
        <div class="w-12 h-12 border-4 border-blue-500 border-t-transparent rounded-full animate-spin" />
        <span class="text-gray-500 dark:text-gray-400">페이지 로딩 중...</span>
      </div>
    </div>
  )
}

// ==================== Query Client 설정 ====================
// Stale-While-Revalidate 패턴:
// - staleTime: 데이터가 신선한 상태로 유지되는 시간 (이 시간 동안은 캐시 사용)
// - gcTime: 가비지 컬렉션 전까지 캐시에 유지되는 시간
// - refetchOnMount: 'always'=항상, false=안함, 기본값='stale'일 때만

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,           // 5분 동안 데이터 신선 (불필요한 재요청 방지)
      gcTime: 1000 * 60 * 30,             // 30분 동안 캐시 유지 (가비지 컬렉션)
      refetchOnWindowFocus: false,         // 탭 포커스 시 재요청 안함
      refetchOnReconnect: 'always',        // 네트워크 재연결 시 항상 재요청
      refetchOnMount: true,                // 컴포넌트 마운트 시 stale이면 재요청
      retry: 2,                            // 실패 시 2회 재시도
      retryDelay: (attemptIndex) =>        // 지수 백오프: 1초, 2초
        Math.min(1000 * 2 ** attemptIndex, 10000),
    },
  },
})

// ==================== 앱 컴포넌트 ====================

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <Router root={Layout}>
          {/* Suspense로 lazy 페이지들을 감싸서 로딩 UI 표시 */}
          <Route path="/" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Dashboard />
            </Suspense>
          )} />
          <Route path="/strategies" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Strategies />
            </Suspense>
          )} />
          <Route path="/backtest" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Backtest />
            </Suspense>
          )} />
          <Route path="/simulation" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Simulation />
            </Suspense>
          )} />
          <Route path="/dataset" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Dataset />
            </Suspense>
          )} />
          <Route path="/screening" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Screening />
            </Suspense>
          )} />
          <Route path="/symbol/:symbol" component={() => (
            <Suspense fallback={<PageLoader />}>
              <SymbolDetail />
            </Suspense>
          )} />
          <Route path="/ranking" component={() => (
            <Suspense fallback={<PageLoader />}>
              <GlobalRanking />
            </Suspense>
          )} />
          <Route path="/ml-training" component={() => (
            <Suspense fallback={<PageLoader />}>
              <MLTraining />
            </Suspense>
          )} />
          <Route path="/journal" component={() => (
            <Suspense fallback={<PageLoader />}>
              <TradingJournal />
            </Suspense>
          )} />
          <Route path="/settings" component={() => (
            <Suspense fallback={<PageLoader />}>
              <Settings />
            </Suspense>
          )} />
        </Router>
      </ToastProvider>
    </QueryClientProvider>
  )
}

export default App
