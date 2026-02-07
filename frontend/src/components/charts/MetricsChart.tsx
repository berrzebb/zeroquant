import { onMount, onCleanup, createEffect, on, For } from 'solid-js'
import { createChart, ColorType, LineSeries } from 'lightweight-charts'
import type { IChartApi, ISeriesApi, LineData } from 'lightweight-charts'

export interface MetricDataPoint {
  time: string | number
  value: number
}

export interface MetricSeries {
  name: string
  data: MetricDataPoint[]
  color?: string
  lineWidth?: number
  visible?: boolean
}

interface MetricsChartProps {
  series: MetricSeries[]
  height?: number
  showLegend?: boolean
  colors?: {
    background?: string
    text?: string
    grid?: string
  }
}

const defaultSeriesColors = [
  '#3b82f6', // blue
  '#22c55e', // green
  '#f59e0b', // amber
  '#ef4444', // red
  '#8b5cf6', // violet
  '#06b6d4', // cyan
]

export function MetricsChart(props: MetricsChartProps) {
  let containerRef: HTMLDivElement | undefined
  let chart: IChartApi | undefined
  let seriesMap: Map<string, ISeriesApi<'Line'>> = new Map()

  const defaultColors = {
    background: 'transparent',
    text: '#d1d5db',
    grid: '#374151',
  }

  const getColors = () => ({
    ...defaultColors,
    ...props.colors,
  })

  onMount(() => {
    if (!containerRef) return

    const colors = getColors()

    chart = createChart(containerRef, {
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
      },
      grid: {
        vertLines: { color: colors.grid },
        horzLines: { color: colors.grid },
      },
      width: containerRef.clientWidth,
      height: props.height || 250,
      crosshair: {
        mode: 1,
      },
      timeScale: {
        borderColor: colors.grid,
        timeVisible: true,
      },
      rightPriceScale: {
        borderColor: colors.grid,
        scaleMargins: {
          top: 0.1,
          bottom: 0.1,
        },
      },
    })

    // Create series for each metric - lightweight-charts v5 API
    props.series.forEach((metric, index) => {
      if (!chart) return

      const seriesColor = metric.color || defaultSeriesColors[index % defaultSeriesColors.length]

      const lineSeries = chart.addSeries(LineSeries, {
        color: seriesColor,
        lineWidth: metric.lineWidth || 2,
        title: metric.name,
      })

      if (metric.data && metric.data.length > 0) {
        lineSeries.setData(metric.data as LineData[])
      }

      seriesMap.set(metric.name, lineSeries)
    })

    if (chart) {
      chart.timeScale().fitContent()
    }

    // Handle resize
    const handleResize = () => {
      if (chart && containerRef) {
        chart.applyOptions({ width: containerRef.clientWidth })
      }
    }

    window.addEventListener('resize', handleResize)

    onCleanup(() => {
      window.removeEventListener('resize', handleResize)
      seriesMap.clear()
      if (chart) {
        chart.remove()
      }
    })
  })

  // Update series data when props change
  createEffect(
    on(
      () => props.series,
      (series) => {
        if (!chart) return

        series.forEach((metric, index) => {
          let lineSeries = seriesMap.get(metric.name)

          // Create new series if it doesn't exist - lightweight-charts v5 API
          if (!lineSeries) {
            const seriesColor = metric.color || defaultSeriesColors[index % defaultSeriesColors.length]
            lineSeries = chart!.addSeries(LineSeries, {
              color: seriesColor,
              lineWidth: metric.lineWidth || 2,
              title: metric.name,
            })
            seriesMap.set(metric.name, lineSeries)
          }

          // Update data
          if (metric.data && metric.data.length > 0) {
            lineSeries.setData(metric.data as LineData[])
          }
        })

        chart?.timeScale().fitContent()
      },
      { defer: true }
    )
  )

  return (
    <div class="relative">
      <div
        ref={containerRef}
        class="w-full rounded-lg overflow-hidden"
        style={{ height: `${props.height || 250}px` }}
      />

      {/* Legend */}
      {props.showLegend !== false && (
        <div class="absolute top-2 right-2 flex flex-wrap gap-3 bg-[var(--color-surface)]/80 backdrop-blur-sm rounded-lg px-3 py-2">
          <For each={props.series}>
            {(metric, index) => (
              <div class="flex items-center gap-2 text-xs">
                <div
                  class="w-3 h-0.5 rounded"
                  style={{
                    'background-color': metric.color || defaultSeriesColors[index() % defaultSeriesColors.length],
                  }}
                />
                <span class="text-[var(--color-text-muted)]">{metric.name}</span>
              </div>
            )}
          </For>
        </div>
      )}
    </div>
  )
}
