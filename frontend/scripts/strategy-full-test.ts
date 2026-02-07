/**
 * 전략 전체 자동화 테스트 스크립트
 *
 * 모든 19개 전략에 대해:
 * 1. 전략 등록
 * 2. 백테스트 실행
 * 3. 결과 검증 (거래 발생 여부, 지표 유효성)
 * 4. 시뮬레이션 테스트
 *
 * 오류 발생 시 수정 후 처음부터 재시작
 */

import axios from 'axios';

const API_BASE = 'http://localhost:3000/api/v1';

// 인증 토큰 (로그인 후 획득)
let AUTH_TOKEN = '';

// axios 인스턴스
const api = axios.create({
  baseURL: API_BASE,
  headers: { 'Content-Type': 'application/json' },
});

api.interceptors.request.use((config) => {
  if (AUTH_TOKEN) {
    config.headers.Authorization = `Bearer ${AUTH_TOKEN}`;
  }
  return config;
});

// ==================== 전략 파라미터 정의 ====================
// 각 전략이 실제로 거래를 발생시킬 수 있는 파라미터

interface StrategyTestConfig {
  id: string;
  name: string;
  category: 'single' | 'multi';  // 단일 자산 vs 다중 자산
  params: Record<string, unknown>;
  symbols: string[];  // 백테스트할 심볼
  expectedTrades?: number;  // 예상 최소 거래 수 (없으면 1)
}

const STRATEGY_CONFIGS: StrategyTestConfig[] = [
  // ==================== 단일 자산 전략 ====================
  {
    id: 'rsi_mean_reversion',
    name: 'RSI 평균회귀',
    category: 'single',
    params: {
      symbols: ['005930'],
      rsi_period: 14,
      oversold_threshold: 30,   // 과매도 기준 완화
      overbought_threshold: 70, // 과매수 기준 완화
      position_size_pct: 50,
      use_stop_loss: true,
      stop_loss_pct: 5,
    },
    symbols: ['005930'],
  },
  {
    id: 'grid_trading',
    name: '그리드 트레이딩',
    category: 'single',
    params: {
      symbols: ['005930'],
      grid_count: 10,
      grid_spacing_pct: 1.5,  // 간격 넓히기
      position_size_per_grid: 10,
      upper_bound_pct: 15,
      lower_bound_pct: 15,
    },
    symbols: ['005930'],
  },
  {
    id: 'bollinger_bands',
    name: '볼린저 밴드',
    category: 'single',
    params: {
      symbols: ['005930'],
      period: 20,
      std_dev: 2.0,
      position_size_pct: 50,
      use_stop_loss: true,
      stop_loss_pct: 3,
    },
    symbols: ['005930'],
  },
  {
    id: 'volatility_breakout',
    name: '변동성 돌파',
    category: 'single',
    params: {
      symbols: ['005930'],
      k_factor: 0.5,  // 돌파 기준 (0.3~0.6)
      position_size_pct: 100,
      use_stop_loss: true,
      stop_loss_pct: 2,
    },
    symbols: ['005930'],
  },
  {
    id: 'magic_split',
    name: '매직 스플릿 (물타기)',
    category: 'single',
    params: {
      symbols: ['005930'],
      split_count: 5,
      split_spacing_pct: 3,
      initial_position_pct: 20,
      take_profit_pct: 5,
    },
    symbols: ['005930'],
  },
  {
    id: 'sma_crossover',
    name: 'SMA 크로스오버',
    category: 'single',
    params: {
      symbols: ['005930'],
      fast_period: 10,
      slow_period: 30,
      position_size_pct: 100,
    },
    symbols: ['005930'],
  },
  {
    id: 'trailing_stop',
    name: '트레일링 스톱',
    category: 'single',
    params: {
      symbols: ['005930'],
      trailing_pct: 5,
      activation_pct: 3,
      position_size_pct: 100,
    },
    symbols: ['005930'],
  },
  {
    id: 'candle_pattern',
    name: '캔들 패턴',
    category: 'single',
    params: {
      symbols: ['005930'],
      patterns: ['hammer', 'engulfing', 'morning_star', 'doji'],
      position_size_pct: 50,
      hold_days: 5,
    },
    symbols: ['005930'],
  },
  {
    id: 'infinity_bot',
    name: '인피니티 봇',
    category: 'single',
    params: {
      symbols: ['005930'],
      rounds: 10,
      spacing_pct: 2,
      position_per_round_pct: 10,
    },
    symbols: ['005930'],
  },
  {
    id: 'market_interest_day',
    name: '거래량 급증 전략',
    category: 'single',
    params: {
      symbols: ['005930'],
      volume_multiplier: 2.0,  // 평균 대비 2배
      lookback_days: 20,
      position_size_pct: 50,
    },
    symbols: ['005930'],
  },

  // ==================== 자산배분 전략 (다중 자산) ====================
  {
    id: 'compound_momentum',
    name: 'Compound Momentum',
    category: 'multi',
    params: {
      offensive_assets: ['QQQ', 'SPY'],
      defensive_assets: ['TLT', 'SHY'],
      momentum_period: 130,
      rebalance_frequency: 'monthly',
      top_n: 1,
    },
    symbols: ['QQQ', 'SPY', 'TLT', 'SHY'],
  },
  {
    id: 'haa',
    name: 'HAA (계층적 자산배분)',
    category: 'multi',
    params: {
      canary_assets: ['VWO', 'BND'],
      offensive_assets: ['QQQ', 'VEA', 'VWO', 'BND'],
      defensive_assets: ['SHY', 'IEF'],
      momentum_period: 60,
    },
    symbols: ['VWO', 'BND', 'QQQ', 'VEA', 'SHY', 'IEF'],
  },
  {
    id: 'xaa',
    name: 'XAA (확장 자산배분)',
    category: 'multi',
    params: {
      canary_assets: ['VWO', 'BND'],
      aggressive_assets: ['QQQ', 'VGK', 'EWJ', 'VWO'],
      safe_assets: ['SHY', 'IEF', 'TLT'],
      momentum_period: 60,
    },
    symbols: ['VWO', 'BND', 'QQQ', 'VGK', 'EWJ', 'SHY', 'IEF', 'TLT'],
  },
  {
    id: 'all_weather',
    name: '올웨더 포트폴리오',
    category: 'multi',
    params: {
      market: 'US',
      stocks_pct: 30,
      long_bonds_pct: 40,
      mid_bonds_pct: 15,
      gold_pct: 7.5,
      commodities_pct: 7.5,
      rebalance_frequency: 'quarterly',
    },
    symbols: ['SPY', 'TLT', 'IEF', 'GLD', 'DBC'],
  },
  {
    id: 'snow',
    name: 'Snow 전략',
    category: 'multi',
    params: {
      base_asset: 'TIP',
      offensive_assets: ['QQQ', 'SPY'],
      defensive_asset: 'SHY',
      ma_period: 200,
    },
    symbols: ['TIP', 'QQQ', 'SPY', 'SHY'],
  },
  {
    id: 'stock_rotation',
    name: '종목 로테이션',
    category: 'multi',
    params: {
      candidates: ['AAPL', 'MSFT', 'GOOGL', 'AMZN', 'NVDA'],
      top_n: 2,
      momentum_period: 60,
      rebalance_frequency: 'monthly',
    },
    symbols: ['AAPL', 'MSFT', 'GOOGL', 'AMZN', 'NVDA'],
  },
  {
    id: 'market_cap_top',
    name: '시총 상위 투자',
    category: 'multi',
    params: {
      market: 'US',
      top_n: 5,
      rebalance_frequency: 'quarterly',
    },
    symbols: ['AAPL', 'MSFT', 'GOOGL', 'AMZN', 'NVDA'],
  },
];

// ==================== 테스트 결과 타입 ====================

interface TestResult {
  strategyId: string;
  strategyName: string;
  registrationSuccess: boolean;
  registrationError?: string;
  backtestSuccess: boolean;
  backtestError?: string;
  trades: number;
  totalReturn: string;
  maxDrawdown: string;
  sharpeRatio: string;
  dataPoints: number;
  hasValidChart: boolean;
  simulationSuccess?: boolean;
  simulationError?: string;
}

// ==================== 테스트 함수 ====================

async function login(): Promise<void> {
  console.log('🔐 로그인 중...');
  try {
    const response = await api.post('/auth/login', {
      username: 'admin',
      password: 'admin123',
    });
    AUTH_TOKEN = response.data.token;
    console.log('✅ 로그인 성공');
  } catch (error) {
    console.error('❌ 로그인 실패:', error);
    throw error;
  }
}

async function deleteAllStrategies(): Promise<void> {
  console.log('🗑️ 기존 전략 삭제 중...');
  try {
    const response = await api.get('/strategies');
    const strategies = response.data.strategies || [];

    for (const strategy of strategies) {
      await api.delete(`/strategies/${strategy.id}`);
      console.log(`  삭제: ${strategy.name}`);
    }
    console.log('✅ 기존 전략 삭제 완료');
  } catch (error) {
    console.warn('⚠️ 전략 삭제 중 오류 (무시됨):', error);
  }
}

async function registerStrategy(config: StrategyTestConfig): Promise<string> {
  console.log(`📝 전략 등록: ${config.name} (${config.id})`);

  const response = await api.post('/strategies', {
    strategy_type: config.id,
    name: config.name,
    parameters: config.params,
  });

  console.log(`  ✅ 등록 완료: ${response.data.strategy_id}`);
  return response.data.strategy_id;
}

async function runBacktest(
  strategyId: string,
  config: StrategyTestConfig,
  startDate: string,
  endDate: string,
  initialCapital: number
): Promise<any> {
  console.log(`📊 백테스트 실행: ${config.name}`);

  const endpoint = config.category === 'multi' ? '/backtest/run-multi' : '/backtest/run';

  const requestBody = config.category === 'multi'
    ? {
        strategy_id: strategyId,
        symbols: config.symbols,
        start_date: startDate,
        end_date: endDate,
        initial_capital: initialCapital,
        slippage_rate: 0.001,
      }
    : {
        strategy_id: strategyId,
        symbol: config.symbols[0],
        start_date: startDate,
        end_date: endDate,
        initial_capital: initialCapital,
        slippage_rate: 0.001,
      };

  const response = await api.post(endpoint, requestBody);
  return response.data;
}

function validateBacktestResult(result: any, config: StrategyTestConfig): {
  isValid: boolean;
  issues: string[];
} {
  const issues: string[] = [];

  // 1. 거래 발생 여부 확인
  const trades = result.metrics?.total_trades || 0;
  const expectedTrades = config.expectedTrades || 1;
  if (trades < expectedTrades) {
    issues.push(`거래 미발생: ${trades}회 (최소 ${expectedTrades}회 필요)`);
  }

  // 2. 데이터 포인트 확인
  const dataPoints = result.config_summary?.data_points || 0;
  if (dataPoints < 10) {
    issues.push(`데이터 부족: ${dataPoints}개 포인트 (최소 10개 필요)`);
  }

  // 3. 자산 곡선 확인
  const equityCurve = result.equity_curve || [];
  if (equityCurve.length < 2) {
    issues.push('자산 곡선 데이터 없음');
  }

  // 4. 지표 유효성 확인
  const metrics = result.metrics;
  if (metrics) {
    const totalReturn = parseFloat(metrics.total_return_pct || '0');
    const maxDrawdown = parseFloat(metrics.max_drawdown_pct || '0');

    // 숫자 유효성
    if (isNaN(totalReturn)) issues.push('총 수익률 지표 무효');
    if (isNaN(maxDrawdown)) issues.push('MDD 지표 무효');
  } else {
    issues.push('백테스트 지표 없음');
  }

  return {
    isValid: issues.length === 0,
    issues,
  };
}

async function checkDataAvailability(symbols: string[]): Promise<{
  available: boolean;
  details: Record<string, { count: number; range: string }>;
}> {
  console.log(`📡 데이터셋 확인: ${symbols.join(', ')}`);

  const details: Record<string, { count: number; range: string }> = {};
  let allAvailable = true;

  for (const symbol of symbols) {
    try {
      const response = await api.get('/market/klines', {
        params: { symbol, timeframe: '1d', limit: 500 },
      });
      const data = response.data.data || [];
      details[symbol] = {
        count: data.length,
        range: data.length > 0
          ? `${data[0].time} ~ ${data[data.length - 1].time}`
          : 'N/A',
      };
      if (data.length < 100) {
        allAvailable = false;
      }
    } catch (error) {
      details[symbol] = { count: 0, range: 'ERROR' };
      allAvailable = false;
    }
  }

  return { available: allAvailable, details };
}

async function runFullTest(): Promise<TestResult[]> {
  const results: TestResult[] = [];

  console.log('═══════════════════════════════════════════════════════════════');
  console.log('       전략 전체 자동화 테스트 시작');
  console.log('═══════════════════════════════════════════════════════════════\n');

  // 1. 로그인
  await login();

  // 2. 기존 전략 삭제
  await deleteAllStrategies();

  // 3. 테스트 기간 설정 (최근 1년)
  const endDate = new Date().toISOString().split('T')[0];
  const startDate = new Date(Date.now() - 365 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];
  const initialCapital = 10000000;  // 1천만원

  console.log(`\n📅 테스트 기간: ${startDate} ~ ${endDate}`);
  console.log(`💰 초기 자본: ${initialCapital.toLocaleString()}원\n`);

  // 4. 각 전략 테스트
  for (let i = 0; i < STRATEGY_CONFIGS.length; i++) {
    const config = STRATEGY_CONFIGS[i];
    console.log(`\n${'─'.repeat(60)}`);
    console.log(`[${i + 1}/${STRATEGY_CONFIGS.length}] ${config.name} 테스트`);
    console.log(`${'─'.repeat(60)}`);

    const result: TestResult = {
      strategyId: '',
      strategyName: config.name,
      registrationSuccess: false,
      backtestSuccess: false,
      trades: 0,
      totalReturn: '0',
      maxDrawdown: '0',
      sharpeRatio: '0',
      dataPoints: 0,
      hasValidChart: false,
    };

    try {
      // 4.1 데이터셋 확인
      const dataCheck = await checkDataAvailability(config.symbols);
      console.log(`  데이터셋 상태:`, dataCheck.details);

      if (!dataCheck.available) {
        result.backtestError = '데이터셋 부족';
        console.warn(`  ⚠️ 데이터셋 부족 - 대체 심볼로 시도`);
        // TODO: 대체 심볼 시도
      }

      // 4.2 전략 등록
      result.strategyId = await registerStrategy(config);
      result.registrationSuccess = true;

      // 4.3 백테스트 실행
      const backtestResult = await runBacktest(
        result.strategyId,
        config,
        startDate,
        endDate,
        initialCapital
      );

      // 4.4 결과 검증
      const validation = validateBacktestResult(backtestResult, config);

      result.backtestSuccess = backtestResult.success;
      result.trades = backtestResult.metrics?.total_trades || 0;
      result.totalReturn = backtestResult.metrics?.total_return_pct || '0';
      result.maxDrawdown = backtestResult.metrics?.max_drawdown_pct || '0';
      result.sharpeRatio = backtestResult.metrics?.sharpe_ratio || '0';
      result.dataPoints = backtestResult.config_summary?.data_points || 0;
      result.hasValidChart = (backtestResult.equity_curve?.length || 0) > 1;

      if (!validation.isValid) {
        console.log(`  ⚠️ 검증 이슈:`, validation.issues);
        result.backtestError = validation.issues.join('; ');
      } else {
        console.log(`  ✅ 검증 통과`);
      }

      console.log(`  📈 결과: 거래 ${result.trades}회, 수익률 ${result.totalReturn}%, MDD ${result.maxDrawdown}%`);

    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error(`  ❌ 오류:`, errorMsg);

      if (!result.registrationSuccess) {
        result.registrationError = errorMsg;
      } else {
        result.backtestError = errorMsg;
      }
    }

    results.push(result);
  }

  return results;
}

function printSummary(results: TestResult[]): void {
  console.log('\n\n═══════════════════════════════════════════════════════════════');
  console.log('                    테스트 결과 요약');
  console.log('═══════════════════════════════════════════════════════════════\n');

  const passed = results.filter(r => r.backtestSuccess && r.trades > 0);
  const failed = results.filter(r => !r.backtestSuccess || r.trades === 0);

  console.log(`✅ 성공: ${passed.length}개`);
  console.log(`❌ 실패: ${failed.length}개\n`);

  console.log('─── 성공한 전략 ───');
  for (const r of passed) {
    console.log(`  ✓ ${r.strategyName}: ${r.trades}회 거래, ${r.totalReturn}% 수익`);
  }

  if (failed.length > 0) {
    console.log('\n─── 실패한 전략 ───');
    for (const r of failed) {
      const reason = r.registrationError || r.backtestError || '거래 미발생';
      console.log(`  ✗ ${r.strategyName}: ${reason}`);
    }
  }

  console.log('\n═══════════════════════════════════════════════════════════════\n');
}

// 메인 실행
runFullTest()
  .then(results => {
    printSummary(results);

    // 실패한 전략이 있으면 종료 코드 1
    const hasFailed = results.some(r => !r.backtestSuccess || r.trades === 0);
    process.exit(hasFailed ? 1 : 0);
  })
  .catch(error => {
    console.error('테스트 실행 중 치명적 오류:', error);
    process.exit(1);
  });
