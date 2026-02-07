import { test, expect } from '@playwright/test';

test.describe('Risk Management UI Schema', () => {
  test.beforeEach(async ({ page }) => {
    // API 서버가 실행 중인지 확인
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    expect(response.ok()).toBeTruthy();
  });

  test('API returns strategies with risk management fields', async ({ page }) => {
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    const data = await response.json();

    expect(data.strategies).toBeDefined();
    expect(data.strategies.length).toBeGreaterThan(0);

    // RSI 전략에서 리스크 관리 필드 검증
    const rsiStrategy = data.strategies.find((s: any) => s.id === 'rsi_mean_reversion');
    expect(rsiStrategy).toBeDefined();
    expect(rsiStrategy.ui_schema).toBeDefined();
    expect(rsiStrategy.ui_schema.fields).toBeDefined();

    // 리스크 관리 필드 존재 확인
    const fields = rsiStrategy.ui_schema.fields;
    const riskFieldKeys = [
      'stop_loss_pct',
      'take_profit_pct',
      'use_trailing_stop',
      'trailing_stop_pct',
      'max_position_pct',
      'daily_loss_limit_pct',
    ];

    for (const key of riskFieldKeys) {
      const field = fields.find((f: any) => f.key === key);
      expect(field, `Field ${key} should exist`).toBeDefined();
    }
  });

  test('All strategies have risk management groups', async ({ page }) => {
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    const data = await response.json();

    const riskGroupIds = ['stop_loss', 'trailing_stop', 'position_sizing', 'daily_limit'];

    for (const strategy of data.strategies) {
      const groups = strategy.ui_schema?.groups || [];

      for (const groupId of riskGroupIds) {
        const group = groups.find((g: any) => g.id === groupId);
        expect(group, `Strategy ${strategy.id} should have group ${groupId}`).toBeDefined();
      }
    }
  });

  test('Risk management default values are strategy-specific', async ({ page }) => {
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    const data = await response.json();

    // RSI 전략 기본값 검증 (보수적 설정)
    const rsiStrategy = data.strategies.find((s: any) => s.id === 'rsi_mean_reversion');
    const rsiFields = rsiStrategy.ui_schema.fields;

    const rsiStopLoss = rsiFields.find((f: any) => f.key === 'stop_loss_pct');
    expect(rsiStopLoss.default_value).toBe(3.0);

    const rsiTakeProfit = rsiFields.find((f: any) => f.key === 'take_profit_pct');
    expect(rsiTakeProfit.default_value).toBe(5.0);

    // Volatility Breakout 전략 기본값 검증 (공격적 설정)
    const vbStrategy = data.strategies.find((s: any) => s.id === 'volatility_breakout');
    const vbFields = vbStrategy.ui_schema.fields;

    const vbStopLoss = vbFields.find((f: any) => f.key === 'stop_loss_pct');
    expect(vbStopLoss.default_value).toBe(5.0);

    const vbTakeProfit = vbFields.find((f: any) => f.key === 'take_profit_pct');
    expect(vbTakeProfit.default_value).toBe(10.0);
  });

  test('Navigate to strategies page and verify UI loads', async ({ page }) => {
    await page.goto('/strategies');

    // 페이지 제목 확인
    await expect(page.getByRole('heading', { name: '전략' })).toBeVisible();

    // 전략 추가 버튼 확인
    await expect(page.getByRole('button', { name: /전략 추가/i })).toBeVisible();
  });

  test('Strategy add modal shows risk management section', async ({ page }) => {
    await page.goto('/strategies');

    // 전략 추가 버튼 클릭
    await page.getByRole('button', { name: /전략 추가/i }).click();

    // 모달이 열릴 때까지 대기
    await page.waitForTimeout(500);

    // 루트 전략 선택 드롭다운 확인
    const strategySelect = page.locator('select').first();
    await expect(strategySelect).toBeVisible();

    // RSI 전략 선택
    await strategySelect.selectOption({ label: /RSI/i });

    // 대기 후 리스크 관리 섹션 확인
    await page.waitForTimeout(500);

    // 리스크 관리 관련 텍스트 확인
    const riskSectionExists = await page.getByText(/손절|스탑로스|stop.*loss/i).count();
    expect(riskSectionExists).toBeGreaterThan(0);
  });

  test('Strategy count matches expected number', async ({ page }) => {
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    const data = await response.json();

    // 최소 19개 전략이 있어야 함 (JSON 스키마에 정의된 수)
    expect(data.strategies.length).toBeGreaterThanOrEqual(19);
  });

  test('Conditional field show_when works correctly', async ({ page }) => {
    const response = await page.request.get('http://localhost:3000/api/v1/backtest/strategies');
    const data = await response.json();

    // trailing_stop_pct 필드의 show_when 조건 확인
    const rsiStrategy = data.strategies.find((s: any) => s.id === 'rsi_mean_reversion');
    const trailingStopPct = rsiStrategy.ui_schema.fields.find(
      (f: any) => f.key === 'trailing_stop_pct'
    );

    expect(trailingStopPct.show_when).toBeDefined();
    expect(trailingStopPct.show_when.field).toBe('use_trailing_stop');
    expect(trailingStopPct.show_when.operator).toBe('equals');
    expect(trailingStopPct.show_when.value).toBe(true);
  });
});
