import { createSignal, onMount, onCleanup } from 'solid-js'
import type {
  Ticker,
  WsMessage,
  WsTicker,
  WsOrderUpdate,
  WsPositionUpdate,
  WsStrategyUpdate,
  WsActiveAccountChanged,
} from '../types'
import { createLogger } from '../utils/logger'

const { log, error: logError } = createLogger('WebSocket')

interface CreateWebSocketOptions {
  onTicker?: (ticker: Ticker) => void
  onOrderUpdate?: (order: WsOrderUpdate) => void
  onPositionUpdate?: (position: WsPositionUpdate) => void
  onStrategyUpdate?: (strategy: WsStrategyUpdate) => void
  onActiveAccountChanged?: (data: WsActiveAccountChanged) => void
  onMessage?: (message: WsMessage) => void
  onConnect?: () => void
  onDisconnect?: () => void
  autoReconnect?: boolean
  reconnectInterval?: number
}

// 백엔드 티커 형식을 프론트엔드 형식으로 변환
function convertTicker(wsTicker: WsTicker): Ticker {
  return {
    symbol: wsTicker.symbol,
    price: parseFloat(wsTicker.price),
    change: parseFloat(wsTicker.change_24h),
    changePercent: parseFloat(wsTicker.change_24h),
    high: parseFloat(wsTicker.high_24h),
    low: parseFloat(wsTicker.low_24h),
    volume: parseFloat(wsTicker.volume_24h),
    timestamp: wsTicker.timestamp,
  }
}

// 심볼을 채널 이름으로 변환 (예: "BTC/USDT" -> "market:BTC-USDT")
function symbolToChannel(symbol: string): string {
  return `market:${symbol.replace('/', '-')}`
}

/**
 * SolidJS WebSocket 훅
 *
 * @example
 * ```tsx
 * const { isConnected, subscribe, unsubscribe } = createWebSocket({
 *   onTicker: (ticker) => setTickers(prev => new Map(prev).set(ticker.symbol, ticker)),
 * })
 *
 * onMount(() => {
 *   subscribe('BTC/USDT')
 * })
 * ```
 */
export function createWebSocket(options: CreateWebSocketOptions = {}) {
  const {
    onTicker,
    onOrderUpdate,
    onPositionUpdate,
    onStrategyUpdate,
    onActiveAccountChanged,
    onMessage,
    onConnect,
    onDisconnect,
    autoReconnect = true,
    reconnectInterval = 3000,
  } = options

  // SolidJS signals (React의 useState 대체)
  const [isConnected, setIsConnected] = createSignal(false)
  const [subscribedChannels, setSubscribedChannels] = createSignal<Set<string>>(new Set())

  // 일반 변수로 ref 대체 (SolidJS는 클로저 캡처)
  let ws: WebSocket | null = null
  let reconnectTimeout: number | null = null

  const connect = () => {
    if (ws?.readyState === WebSocket.OPEN) {
      return
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws`

    log('Connecting to:', wsUrl)
    ws = new WebSocket(wsUrl)

    ws.onopen = () => {
      log('Connected')
      setIsConnected(true)
      onConnect?.()

      // 재연결 시 기존 채널 재구독
      const channels = subscribedChannels()
      if (channels.size > 0) {
        const channelArray = Array.from(channels)
        ws?.send(JSON.stringify({ type: 'subscribe', channels: channelArray }))
      }
    }

    ws.onclose = () => {
      log('Disconnected')
      setIsConnected(false)
      onDisconnect?.()

      if (autoReconnect) {
        reconnectTimeout = window.setTimeout(() => {
          connect()
        }, reconnectInterval)
      }
    }

    ws.onerror = (error) => {
      logError('Connection error:', error)
    }

    ws.onmessage = (event) => {
      try {
        const message: WsMessage = JSON.parse(event.data)
        onMessage?.(message)

        // 메시지 타입별 처리
        switch (message.type) {
          case 'ticker':
            log('Ticker received:', (message as WsTicker).symbol)
            if (onTicker) {
              onTicker(convertTicker(message as WsTicker))
            }
            break
          case 'order_update':
            onOrderUpdate?.(message as WsOrderUpdate)
            break
          case 'position_update':
            onPositionUpdate?.(message as WsPositionUpdate)
            break
          case 'strategy_update':
            onStrategyUpdate?.(message as WsStrategyUpdate)
            break
          case 'active_account_changed':
            log('Active account changed:', (message as WsActiveAccountChanged).exchange_name)
            onActiveAccountChanged?.(message as WsActiveAccountChanged)
            break
          case 'welcome':
            log('Server version:', (message as { version: string }).version)
            break
          case 'subscribed':
            log('Subscribed to:', (message as { channels: string[] }).channels)
            break
          case 'error':
            logError('Server error:', (message as { code: string; message: string }).message)
            break
        }
      } catch (error) {
        logError('Failed to parse message:', error)
      }
    }
  }

  const disconnect = () => {
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout)
      reconnectTimeout = null
    }

    if (ws) {
      ws.close()
      ws = null
    }
  }

  // 심볼 구독 (market:SYMBOL 형식으로 변환)
  const subscribe = (symbol: string) => {
    const channel = symbolToChannel(symbol)
    log('Subscribing:', symbol, '->', channel)
    setSubscribedChannels((prev) => new Set(prev).add(channel))

    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'subscribe', channels: [channel] }))
    }
  }

  // 심볼 구독 해제
  const unsubscribe = (symbol: string) => {
    const channel = symbolToChannel(symbol)
    setSubscribedChannels((prev) => {
      const next = new Set(prev)
      next.delete(channel)
      return next
    })

    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'unsubscribe', channels: [channel] }))
    }
  }

  // 채널 직접 구독 (orders, positions, strategies 등)
  const subscribeChannels = (channels: string[]) => {
    channels.forEach((ch) => {
      setSubscribedChannels((prev) => new Set(prev).add(ch))
    })

    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'subscribe', channels }))
    }
  }

  // 채널 직접 구독 해제
  const unsubscribeChannels = (channels: string[]) => {
    channels.forEach((ch) => {
      setSubscribedChannels((prev) => {
        const next = new Set(prev)
        next.delete(ch)
        return next
      })
    })

    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'unsubscribe', channels }))
    }
  }

  // JWT 토큰으로 인증
  const authenticate = (token: string) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'auth', token }))
    }
  }

  // Ping 전송 (연결 유지)
  const ping = () => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'ping' }))
    }
  }

  // 일반 메시지 전송
  const send = (data: unknown) => {
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(data))
    }
  }

  // 컴포넌트 마운트 시 자동 연결
  onMount(() => {
    connect()
  })

  // 컴포넌트 언마운트 시 자동 해제
  onCleanup(() => {
    disconnect()
  })

  return {
    isConnected,
    subscribe,
    unsubscribe,
    subscribeChannels,
    unsubscribeChannels,
    authenticate,
    ping,
    send,
    connect,
    disconnect,
  }
}
