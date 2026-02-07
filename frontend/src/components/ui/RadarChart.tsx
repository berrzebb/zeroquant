/**
 * 레이더 차트 컴포넌트
 *
 * ComponentScores를 시각적으로 표시하는 SVG 기반 레이더 차트입니다.
 * 7Factor 스코어링 시스템을 지원합니다.
 */
import { type Component, createMemo, For } from 'solid-js'

/** 크기 프리셋 */
export type RadarChartSize = 'sm' | 'md' | 'lg' | number

/** 크기 프리셋 값 (픽셀) */
const SIZE_PRESETS: Record<string, number> = {
  sm: 120,
  md: 200,
  lg: 300,
}

export interface RadarChartProps {
  /** 데이터 포인트 (key: label, value: 0-100) */
  data: Record<string, number | undefined>
  /** 차트 크기 (sm: 120px, md: 200px, lg: 300px 또는 픽셀 값) */
  size?: RadarChartSize
  /** 채우기 색상 */
  fillColor?: string
  /** 테두리 색상 */
  strokeColor?: string
  /** 라벨 표시 여부 */
  showLabels?: boolean
  /** 값 표시 여부 (라벨 옆에 숫자 표시) */
  showValues?: boolean
  /** 평균선(50) 강조 표시 여부 */
  showReferenceLine?: boolean
}

// 라벨 한글 매핑
const LABEL_MAP: Record<string, string> = {
  // 기존 기술적 분석 라벨
  technical: '기술',
  momentum: '모멘텀',
  trend: '추세',
  volume: '거래량',
  volatility: '변동성',
  // 7Factor 정규화 점수 라벨
  norm_momentum: '모멘텀',
  norm_value: '가치',
  norm_quality: '품질',
  norm_volatility: '변동성',
  norm_liquidity: '유동성',
  norm_growth: '성장성',
  norm_sentiment: '심리',
  // GlobalScore 컴포넌트 라벨
  risk_reward: '리스크보상',
  target_room: '목표여력',
  stop_room: '손절여력',
  entry_proximity: '진입근접',
  liquidity: '유동성',
  technical_balance: '기술균형',
}

export const RadarChart: Component<RadarChartProps> = (props) => {
  // 크기 계산 (프리셋 또는 직접 값)
  const size = () => {
    const sizeValue = props.size || 'md'
    if (typeof sizeValue === 'string') {
      return SIZE_PRESETS[sizeValue] || SIZE_PRESETS.md
    }
    return sizeValue
  }

  const center = () => size() / 2
  const radius = () => (size() / 2) * 0.65  // 라벨 공간 확보를 위해 0.7 → 0.65로 조정
  const fillColor = () => props.fillColor || 'rgba(59, 130, 246, 0.3)'
  const strokeColor = () => props.strokeColor || 'rgb(59, 130, 246)'
  const showLabels = () => props.showLabels !== false
  const showValues = () => props.showValues === true
  const showReferenceLine = () => props.showReferenceLine !== false

  // 데이터 포인트 추출
  const dataPoints = createMemo(() => {
    const result: { key: string; label: string; value: number }[] = []
    for (const key in props.data) {
      if (props.data[key] !== undefined) {
        result.push({
          key,
          label: LABEL_MAP[key] || key,
          value: Math.min(100, Math.max(0, props.data[key] || 0)),
        })
      }
    }
    return result
  })

  // 각도 계산 (시작: 상단, 시계 방향)
  const getAngle = (index: number, total: number) => {
    return (Math.PI * 2 * index) / total - Math.PI / 2
  }

  // 좌표 계산
  const getPoint = (index: number, total: number, value: number) => {
    const angle = getAngle(index, total)
    const r = (value / 100) * radius()
    return {
      x: center() + r * Math.cos(angle),
      y: center() + r * Math.sin(angle),
    }
  }

  // 라벨 위치 계산 (크기에 따라 거리 조정)
  const getLabelPoint = (index: number, total: number) => {
    const angle = getAngle(index, total)
    const labelDistance = size() < 150 ? 15 : size() < 250 ? 20 : 25
    const r = radius() + labelDistance
    return {
      x: center() + r * Math.cos(angle),
      y: center() + r * Math.sin(angle),
    }
  }

  // 폴리곤 포인트 문자열 생성
  const polygonPoints = createMemo(() => {
    const points = dataPoints()
    if (points.length < 3) return ''
    return points
      .map((_, i) => {
        const p = getPoint(i, points.length, points[i].value)
        return `${p.x},${p.y}`
      })
      .join(' ')
  })

  // 그리드 라인 생성 (25, 50, 75, 100)
  const gridLines = createMemo(() => {
    const levels = [25, 50, 75, 100]
    const total = dataPoints().length
    if (total < 3) return []

    return levels.map((level) => {
      const points = Array.from({ length: total }, (_, i) => {
        const p = getPoint(i, total, level)
        return `${p.x},${p.y}`
      }).join(' ')
      return { level, points, isReference: level === 50 }
    })
  })

  // 축 라인 생성
  const axisLines = createMemo(() => {
    const total = dataPoints().length
    if (total < 3) return []

    return Array.from({ length: total }, (_, i) => {
      const p = getPoint(i, total, 100)
      return { x1: center(), y1: center(), x2: p.x, y2: p.y }
    })
  })

  // 폰트 크기 계산 (차트 크기에 따라 조정)
  const fontSize = () => {
    if (size() < 150) return 9
    if (size() < 250) return 11
    return 13
  }

  // 데이터 포인트 원 크기
  const pointRadius = () => {
    if (size() < 150) return 3
    if (size() < 250) return 4
    return 5
  }

  return (
    <svg
      width={size()}
      height={size()}
      viewBox={`0 0 ${size()} ${size()}`}
      class="radar-chart"
    >
      {/* 그리드 라인 */}
      <For each={gridLines()}>
        {(grid) => (
          <polygon
            points={grid.points}
            fill="none"
            stroke={grid.isReference && showReferenceLine() ? 'currentColor' : 'currentColor'}
            stroke-width={grid.isReference && showReferenceLine() ? '1.5' : '1'}
            stroke-dasharray={grid.isReference && showReferenceLine() ? '4,2' : 'none'}
            class={
              grid.isReference && showReferenceLine()
                ? 'text-blue-400 dark:text-blue-500'
                : 'text-gray-200 dark:text-gray-700'
            }
            opacity={grid.isReference && showReferenceLine() ? '0.8' : '0.5'}
          />
        )}
      </For>

      {/* 축 라인 */}
      <For each={axisLines()}>
        {(axis) => (
          <line
            x1={axis.x1}
            y1={axis.y1}
            x2={axis.x2}
            y2={axis.y2}
            stroke="currentColor"
            stroke-width="1"
            class="text-gray-200 dark:text-gray-700"
            opacity="0.5"
          />
        )}
      </For>

      {/* 평균선 라벨 (50) */}
      {showReferenceLine() && size() >= 150 && (
        <text
          x={center() + 5}
          y={center() - radius() * 0.5 - 3}
          class="fill-blue-500 dark:fill-blue-400"
          font-size={fontSize() - 2}
          opacity="0.7"
        >
          50
        </text>
      )}

      {/* 데이터 폴리곤 */}
      <polygon
        points={polygonPoints()}
        fill={fillColor()}
        stroke={strokeColor()}
        stroke-width="2"
      />

      {/* 데이터 포인트 */}
      <For each={dataPoints()}>
        {(point, index) => {
          const p = getPoint(index(), dataPoints().length, point.value)
          return (
            <circle
              cx={p.x}
              cy={p.y}
              r={pointRadius()}
              fill={strokeColor()}
            />
          )
        }}
      </For>

      {/* 라벨 및 값 표시 */}
      {showLabels() && (
        <For each={dataPoints()}>
          {(point, index) => {
            const labelPos = getLabelPoint(index(), dataPoints().length)
            const angle = getAngle(index(), dataPoints().length)

            // 텍스트 정렬 계산
            const isTop = Math.abs(angle + Math.PI / 2) < 0.3
            const isBottom = Math.abs(angle - Math.PI / 2) < 0.3
            const textAnchor = isTop || isBottom
              ? 'middle'
              : angle > -Math.PI / 2 && angle < Math.PI / 2
              ? 'start'
              : 'end'

            // 라벨 텍스트 (값 포함 여부에 따라)
            const labelText = showValues()
              ? `${point.label} (${Math.round(point.value)})`
              : point.label

            return (
              <text
                x={labelPos.x}
                y={labelPos.y}
                text-anchor={textAnchor}
                dominant-baseline="middle"
                font-size={fontSize()}
                class="fill-gray-600 dark:fill-gray-400"
              >
                {labelText}
              </text>
            )
          }}
        </For>
      )}
    </svg>
  )
}

export default RadarChart
