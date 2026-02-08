# ZeroQuant Trading Bot - PRD (Product Requirements Document)

> 버전: 8.0 | 마지막 업데이트: 2026-02-07

---

## 1. 제품 개요

### 1.1 목적
Rust 기반 고성능 다중 시장 자동화 트레이딩 시스템. 국내/해외 주식 및 암호화폐 시장에서 다양한 전략을 자동으로 실행하고 관리한다.

### 1.2 대상 사용자
- 개인 투자자 (개인 프로젝트)
- 퀀트 트레이딩 학습자

### 1.3 핵심 가치
- **자동화**: 전략 기반 자동 매매로 감정 개입 배제
- **다양성**: 25+ 전략, 다중 거래소/시장 지원
- **안전성**: 리스크 관리, 시뮬레이션 검증 후 실전 운용
- **학습**: 백테스트를 통한 전략 성과 분석 및 개선

---

## 2. 기능 요구사항

### 2.1 전략 관리

#### 2.1.1 전략 등록
- 사용자는 제공된 기본 전략(27개) 중 선택하여 새로운 전략 인스턴스를 생성한다
- 전략 유형:
  - **단일 자산 전략**: 하나의 심볼에 대해 매매 신호 생성 (Grid, RSI, Bollinger 등)
  - **자산배분 전략**: 여러 심볼로 구성된 포트폴리오 리밸런싱 (HAA, XAA, All Weather 등)
- 전략 인스턴스는 고유한 이름으로 저장되며, 동일 기본 전략에서 여러 인스턴스 생성 가능

#### 2.1.2 파라미터 설정 (SDUI 자동 생성)

##### 기본 동작
- 각 전략은 SDUI(Server-Driven UI) 스키마를 통해 동적 파라미터 폼을 렌더링한다
- **전략 Config 구조체에서 UI 스키마가 자동 생성**되어 수동 스키마 작성 불필요
- 파라미터 유형:
  - **심볼**: 대상 종목 (자동완성 검색 지원)
  - **기술적 지표**: RSI 기간, 볼린저 밴드 표준편차, 이동평균 기간 등
  - **거래 조건**: 진입/청산 임계값, 포지션 크기 비율
  - **타임프레임**: 1분, 5분, 15분, 30분, 1시간, 4시간, 일봉
  - **다중 타임프레임 (선택)**: Primary 타임프레임 외에 Secondary 타임프레임 추가 (최대 2개)
    - 예: Primary=5분, Secondary=[1시간, 1일]
    - 멀티 타임프레임 분석(MTF Analysis)을 통한 정교한 신호 생성
    - Secondary는 Primary보다 큰 타임프레임만 허용
- 파라미터 유효성 검증:
  - 숫자 범위 제한 (min/max)
  - 필수 값 검증
  - 타입 검증 (정수, 실수, 문자열, 배열)

##### SDUI Fragment 시스템
- **Schema Fragment**: 재사용 가능한 UI 스키마 조각
  - 카테고리: Indicator, Filter, RiskManagement, PositionSizing, Timing, Asset
  - 예: `indicator.rsi`, `filter.route_state`, `risk.trailing_stop`
- **FragmentRegistry**: 빌트인 Fragment 관리 및 조회
- **SchemaComposer**: Fragment + 커스텀 필드 → 완성된 SDUI JSON 조합

##### 자동 생성 흐름
1. 전략 Config에 `#[derive(StrategyConfig)]` 매크로 적용
2. 사용할 Fragment를 `#[fragment("id")]` 속성으로 지정
3. 커스텀 필드는 `#[schema(label, min, max)]` 속성으로 메타데이터 정의
4. 런타임에 `SchemaComposer`가 완성된 SDUI JSON 반환
5. 프론트엔드 `SDUIRenderer`가 JSON 기반으로 폼 렌더링

##### 조건부 필드 표시
- 특정 필드 값에 따라 다른 필드 표시/숨김 가능
- 예: "트레일링 스탑 활성화" 체크 시에만 관련 설정 표시
- `condition` 속성: `"enabled == true"`, `"mode == 'advanced'"` 등

##### API 엔드포인트
- `GET /api/v1/strategies/meta`: 전략 목록 및 기본 정보
- `GET /api/v1/strategies/{id}/schema`: 해당 전략의 완성된 SDUI JSON
- `GET /api/v1/schema/fragments`: 사용 가능한 Fragment 카탈로그

#### 2.1.3 리스크 설정
- 전략별 리스크 파라미터:
  - **손절가 (Stop Loss)**: 진입가 대비 손실 허용 비율 (기본 3%)
  - **익절가 (Take Profit)**: 진입가 대비 목표 수익 비율 (기본 5%)
  - **트레일링 스탑**: 고점 대비 하락 허용 비율 (기본 2%)
  - **포지션 크기**: 총 자본 대비 단일 포지션 비율 (최대 10%)
- 리스크 설정은 전략별 기본값이 SDUI 스키마에 정의되며, 사용자가 수정 가능
- 일일 손실 한도 설정 (기본 3%, UTC 자정 자동 리셋)

#### 2.1.4 전략 CRUD
- **생성**: 기본 전략 선택 → 파라미터 입력 → 저장
- **조회**: 등록된 전략 목록, 상세 정보 조회
- **수정**: 파라미터 변경 (전략 유형 변경 불가)
- **삭제**: 전략 인스턴스 삭제 (관련 백테스트 결과 보존)
- **복사**: 기존 전략 복사하여 새 인스턴스 생성 (이름만 변경)

---

### 2.2 백테스트

#### 2.2.1 백테스트 실행
- 입력 조건:
  - **전략**: 등록된 전략 인스턴스 선택
  - **기간**: 시작일 ~ 종료일 (과거 데이터)
  - **초기 자본**: 시뮬레이션 시작 금액
- 데이터 요구사항:
  - 해당 심볼/기간의 OHLCV 데이터 필요
  - 캐시에 없을 경우 Yahoo Finance에서 자동 다운로드
- 시뮬레이션 옵션:
  - **슬리피지**: 체결가 오차 (기본 0.1%)
  - **수수료**: 거래 수수료 (기본 0.1%)
  - **마진**: 레버리지 설정 (선물/암호화폐)

#### 2.2.2 성과 지표
- 수익률 지표:
  - 총 수익률, 연환산 수익률 (CAGR)
  - 월별/연도별 수익률
- 리스크 지표:
  - **MDD** (Maximum Drawdown): 최대 낙폭
  - **변동성**: 수익률 표준편차
- 위험조정 수익률:
  - **Sharpe Ratio**: (수익률 - 무위험수익률) / 변동성
  - **Sortino Ratio**: 하방 변동성만 고려
  - **Calmar Ratio**: CAGR / MDD
- 거래 통계:
  - 총 거래 수, 승률
  - 평균 수익/손실, 손익비
  - 최장 연승/연패

#### 2.2.3 결과 시각화
- **자산 곡선**: 일별 포트폴리오 가치 추이
- **드로다운 차트**: 고점 대비 하락률 추이
- **월별 수익률 히트맵**: 연-월 매트릭스 색상 표시
- **거래 목록**: 진입/청산 시점, 가격, 손익

#### 2.2.4 결과 저장
- 백테스트 결과 DB 저장
- 저장 항목: 전략 ID, 기간, 파라미터 스냅샷, 성과 지표, 거래 내역
- 히스토리 조회 및 비교 기능

---

### 2.2.5 신호 기록 (SignalMarker)

**목적**: 전략이 생성한 모든 신호를 DB에 저장하여 분석 및 시각화에 활용

**핵심 기능**:
- 진입/청산 신호, 신호 강도, 지표 값 기록
- 백테스트와 실거래에서 동일 형식 사용
- UnifiedTrade trait으로 타입 통합

**저장 정보**:
- 신호 유형 (Entry, Exit, Alert)
- 발생 시점 지표 값 (RSI, MACD, BB 등)
- RouteState, 전략 정보
- 실행 여부 (체결/미체결)

**예상 구현**: v0.6.0 (TODO Phase 1-5)

#### 2.2.6 신호 시각화 (캔들 차트 오버레이)

**SignalMarker 오버레이**:
- 매수 신호: 초록색 위 화살표 ▲
- 매도 신호: 빨간색 아래 화살표 ▼
- 알림 신호: 노란색 점 ●

**IndicatorFilter 패널**:
- RSI 범위 슬라이더
- MACD 크로스 유형 선택
- RouteState 필터
- 전략 선택 드롭다운

**통합 화면**:
- 백테스트 결과 페이지
- 종목 상세 페이지
- 전략 디버깅 페이지

**예상 구현**: v0.6.0 (TODO Phase 2-4)

---

### 2.3 시뮬레이션 (Paper Trading) ⭐ v0.8.0 구현 완료 → v0.9.0 확장

#### 2.3.1 시뮬레이션 실행
- 실시간 시장 데이터 기반 가상 거래
- 실제 자금 사용 없이 전략 검증
- 실행 모드:
  - **실시간 모드**: WebSocket으로 틱/호가 데이터 수신
  - **과거 리플레이 모드**: DB 1분봉 데이터를 실시간 속도로 재생 (v0.9.0)
  - **랜덤 워크 모드**: 장외 시간 현실적 가격 변동 생성 (v0.9.0)

#### 2.3.2 포지션 관리
- 가상 포지션 추적:
  - 보유 종목, 수량, 진입가
  - 미실현 손익 (현재가 기준)
- 가상 주문 실행:
  - 지정가/시장가 주문
  - 주문 체결 시뮬레이션 (호가창 기반 VWAP)
  - 부분 체결 지원 (호가 잔량 기반) (v0.9.0)
  - 스톱 주문: StopLoss, TakeProfit, StopLossLimit, TakeProfitLimit (v0.9.0)

#### 2.3.2.1 주문 관리 (v0.9.0)
- 미체결 주문 큐 관리 (전략별 독립)
- 매 가격 틱마다 자동 매칭 (MockOrderEngine)
- 주문 정정: 수량/가격 변경, 잔고 예약 자동 재계산
- 주문 취소: 큐 제거 + 예약 잔고 해제
- 잔고 예약 시스템: 매수 주문 등록 시 필요 금액 예약 (이중 사용 방지)
- 주문 영속성: DB 저장/복원 (앱 재시작 시 미체결 주문 유지)

#### 2.3.3 성과 모니터링
- 실시간 대시보드:
  - 현재 포트폴리오 가치
  - 일별/누적 수익률
  - 활성 포지션 목록
- 거래 내역 로깅

#### 2.3.4 Paper Trading API ⭐ v0.8.0

**구현 완료** (`trader-api/src/routes/paper_trading.rs`)

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/paper-trading/start` | POST | 페이퍼 트레이딩 세션 시작 (스트리밍 설정 포함, v0.9.0 확장) |
| `/api/v1/paper-trading/accounts` | GET | 가상 계좌 목록 조회 |
| `/api/v1/paper-trading/positions` | GET | 가상 포지션 조회 |
| `/api/v1/paper-trading/executions` | GET | 가상 체결 내역 조회 |
| `/api/v1/paper-trading/reset` | POST | 계좌 초기화 |

**Signal Processor 서비스** (`trader-api/src/services/signal_processor.rs`):
- API 서버에서 Signal 처리 파이프라인 관리
- SimulatedExecutor 연동으로 가상 체결 처리

**프론트엔드** (`frontend/src/components/simulation/PaperTrading.tsx`):
- Simulation 페이지 내 Paper Trading 탭
- 계좌 현황, 포지션, 체결 내역 실시간 표시

**TypeScript 바인딩** (ts-rs 자동 생성):
- PaperTradingAccount, PaperTradingPosition, PaperTradingExecution 등 10개 타입

---

### 2.4 실전 운용 (Live Trading)

#### 2.4.1 거래소 연동 ⭐ v0.8.0 WebSocket 실시간 연동 완료
- 지원 거래소:
  - **Binance**: 암호화폐 현물/선물
  - **KIS (한국투자증권)**: 국내 주식, 해외 주식 (미국)
- 연동 기능:
  - OAuth/API Key 인증
  - 잔고 조회, 주문 실행, 체결 내역 조회
  - 실시간 시세 WebSocket

#### 2.4.1.1 KIS WebSocket 실시간 시세 ⭐ v0.8.0

**6-Phase 구현 완료**:

| Phase | 내용 | 구현 파일 |
|-------|------|----------|
| 1 | 동적 구독 지원 | `websocket_kr.rs`, `websocket_us.rs` |
| 2 | 동시 수신 (Bridge Task) | `stream.rs` (UnifiedMarketStream) |
| 3 | Singleton 스트림 관리 | `services/market_stream.rs` |
| 4 | 전략 실행 시 자동 연결 | `routes/strategies.rs` |
| 5 | 서버 시작 순서 최적화 | `main.rs` |
| 6 | 프론트엔드 WebSocket 브릿지 | `websocket/handler.rs` |

**핵심 아키텍처**:
- **Singleton 패턴**: credential_id별 하나의 WebSocket 스트림 유지
- **참조 카운트**: 여러 전략/클라이언트의 동일 심볼 구독을 하나의 스트림으로 공유
- **동적 구독**: 전략 추가/삭제 시 런타임에 심볼 구독/해제
- **Bridge Task**: KR/US 스트림을 별도 tokio 태스크로 분리, mpsc 채널로 이벤트 통합
- **프론트엔드 브릿지**: `market:{symbol}` WebSocket 채널 구독 → 거래소 스트림 자동 전파

#### 2.4.1.2 Exchange Provider 통합 ⭐ v0.8.0

**구현 완료**:
- `KisExchangeProvider` - KR/US 통합 프로바이더 (`provider/kis.rs`)
- `MockExchangeProvider` - 개발/테스트용 가상 거래소 (`provider/mock.rs`)
- `KIS Client` 공통 HTTP 클라이언트 (`connector/kis/client.rs`)
- `AccountInfo`, `ExchangeConstraints` - 거래소 중립적 표준 타입 (`domain/account.rs`, `exchange_types.rs`)

#### 2.4.2 자동 매매
- 전략 신호 기반 자동 주문:
  - 매수/매도 신호 발생 시 자동 주문 전송
  - 손절/익절 조건 충족 시 자동 청산
- 주문 유형:
  - 시장가 (Market)
  - 지정가 (Limit)
  - 스탑 주문 (Stop-Loss, Take-Profit)
- 주문 검증:
  - 최소 주문 수량 확인
  - 잔고 충분 여부 확인
  - 일일 거래 한도 확인

#### 2.4.3 포트폴리오 관리
- 통합 잔고 조회:
  - 여러 거래소/계좌 잔고 통합
  - 자산 배분 현황 (비중)
- 보유 종목 현황:
  - 종목별 수량, 평균 매입가
  - 평가 금액, 수익률

#### 2.4.4 알림 시스템 ⭐ v0.7.3

**다중 채널 지원**:
| 채널 | 설명 | 설정 |
|------|------|------|
| **Telegram** | 양방향 봇 통신 | Bot Token + Chat ID |
| **Discord** | Webhook 알림 | Webhook URL |
| **Slack** | Webhook 알림 | Webhook URL |
| **Email** | SMTP 발송 | SMTP 서버 + 계정 |
| **SMS** | Twilio 문자 | Account SID + Auth Token |

**알림 유형**:
- 주문 체결 알림
- 손익 임계값 도달 알림
- 전략 신호 발생 알림 (ATTACK 상태)
- 시스템 오류 알림

**저장 전 연결 테스트** ⭐:
- 각 알림 채널 설정 시 저장 전에 실제 테스트 메시지 발송 가능
- API 엔드포인트: `POST /api/v1/credentials/{channel}/test/new`
- 테스트 성공 확인 후 저장 → 설정 오류 방지

**텔레그램 봇 명령어** (양방향 통신):
- `/portfolio`: 현재 포트폴리오 조회
- `/status`: 전략 실행 상태 조회
- `/stop <id>`: 특정 전략 중지
- `/report`: 일일/주간 성과 리포트

**보안**:
- 모든 자격증명 AES-256-GCM 암호화 저장
- API 응답에서 민감 정보 마스킹 (예: `sk-...xxxx`)

---

### 2.5 데이터 관리

#### 2.5.1 시장 데이터 수집
- **OHLCV 데이터**:
  - Open, High, Low, Close, Volume
  - 지원 타임프레임: 1m, 5m, 15m, 30m, 1h, 4h, 1d
- **데이터 소스**:
  - Yahoo Finance (주식, ETF)
  - Binance API (암호화폐)
  - KIS API (국내 주식 실시간)
- **자동 다운로드**:
  - 백테스트 실행 시 필요 데이터 자동 요청
  - 캐시 (TimescaleDB)에 저장하여 재사용

#### 2.5.2 데이터셋 관리
- 데이터셋 목록 조회:
  - 보유 심볼, 기간, 데이터 포인트 수
- 차트 시각화:
  - 캔들스틱 차트
  - 기술적 지표 오버레이 (SMA, EMA, RSI, MACD, Bollinger)
- 데이터 품질 검증:
  - 누락 구간 감지
  - 이상치 표시

#### 2.5.3 심볼 검색
- 자동완성 검색:
  - 티커, 종목명, 영문명으로 검색
  - 시장별 필터링 (KR, US, Crypto)
- 심볼 정보:
  - 정규화된 심볼 (canonical)
  - 거래소별 심볼 매핑 (Yahoo, KIS, Binance)
  - 표시 이름: "티커(종목명)" 형식

#### 2.5.4 심볼 자동 동기화

##### 자동 동기화 (백그라운드)
- **목적**: 스크리닝 수집기 가동 시 자동으로 전체 종목 목록을 수집하여 symbol_info 테이블에 등록
- **데이터 소스**:
  - **KRX (한국거래소)**: KOSPI/KOSDAQ 전 종목 (~2,500개)
  - **Binance**: USDT 거래 페어 활성 종목 (~300개)
  - **Yahoo Finance**: 미국 주식 주요 지수 구성종목 (S&P 500, NASDAQ 등)
- **동기화 트리거**:
  - 서버 시작 시 심볼 수가 최소 기준 이하면 자동 실행
  - Fundamental 배치 수집 전 자동 호출
- **환경변수**:
  | 변수 | 기본값 | 설명 |
  |------|--------|------|
  | `SYMBOL_SYNC_KRX` | true | KRX 동기화 활성화 |
  | `SYMBOL_SYNC_BINANCE` | false | Binance 동기화 활성화 |
  | `SYMBOL_SYNC_YAHOO` | true | Yahoo Finance 동기화 활성화 |
  | `SYMBOL_SYNC_YAHOO_MAX` | 500 | Yahoo 최대 수집 수 |
  | `SYMBOL_SYNC_MIN_COUNT` | 100 | 최소 심볼 수 기준 |

##### CLI 도구 (수동 관리) ✅ v0.5.6
- **목적**: 종목 데이터의 수동 관리 및 유지보수를 위한 CLI 명령어 제공

**1. CSV 변환 (`scripts/convert_krx_new_to_csv.py`)**
- KRX 정보시스템 원본 CSV → 표준 형식 변환
- 상품 분류별 파일 (ETF, 주식, 파생상품 등) 통합 처리
- EUC-KR/CP949 인코딩 자동 감지
- 출력: `data/krx_codes.csv` (종목코드, 종목명)
```bash
python scripts/convert_krx_new_to_csv.py --input-dir data/new --output-dir data
```

**2. CSV 동기화 (`trader sync-csv`)**
- CSV 파일 → symbol_info 테이블 동기화
- KOSPI/KOSDAQ 자동 판별
- Yahoo Finance 심볼 자동 생성
- Upsert 방식으로 안전한 업데이트
- 섹터 정보 선택적 업데이트
```bash
trader sync-csv --codes data/krx_codes.csv [--sectors data/krx_sector_map.csv]
```

**3. 종목 조회 (`trader list-symbols`)**
- DB에서 종목 정보 실시간 조회
- 필터: 시장(KR/US/CRYPTO/ALL), 활성 여부, 검색 키워드
- 출력 형식: table (사람), csv (데이터 분석), json (API 연동)
- 파일 저장 옵션
```bash
trader list-symbols --market KR --limit 100 --format csv --output symbols.csv
```

**4. 온라인 자동 크롤링 (`trader fetch-symbols`) ⭐**
- 온라인 소스에서 실시간 종목 정보 수집 및 DB 저장
- **데이터 소스**:
  - KR: KRX 공식 API (전체 종목, ~2,500개)
  - US: Yahoo Finance (주요 500개, 확장 가능)
  - CRYPTO: Binance API (USDT 페어 ~446개)
- **기능**:
  - 시장별 선택 수집 (KR/US/CRYPTO/ALL)
  - CSV 백업 옵션 (`--save-csv`)
  - 드라이런 모드 (`--dry-run`, 테스트용)
  - 진행 상황 실시간 표시
```bash
# 전체 시장 수집
trader fetch-symbols --market ALL

# 특정 시장만
trader fetch-symbols --market KR --save-csv
```

**워크플로우**:
```
방법 1: 온라인 자동 수집 (권장)
  trader fetch-symbols --market ALL
  ↓
  DB에 직접 저장 완료

방법 2: 수동 CSV 관리
  KRX 사이트에서 CSV 다운로드
  ↓
  python scripts/convert_krx_new_to_csv.py
  ↓
  trader sync-csv --codes data/krx_codes.csv
```

#### 2.5.5 데이터 프로바이더 이중화 ⭐ v0.6.0

**목적**: KRX OPEN API + Yahoo Finance 이중화 구조로 데이터 소스 안정성 확보

**이중화 구조**:
| 시장 | Primary | Fallback | 비고 |
|------|---------|----------|------|
| 국내 주식 (KR) | KRX OPEN API | Yahoo Finance | API 승인 후 활성화 |
| 해외 주식 (US) | Yahoo Finance | - | 500개 주요 종목 |
| 암호화폐 (CRYPTO) | Yahoo Finance | - | USDT 페어 |

**토글 환경변수**:
| 변수 | 기본값 | 설명 |
|------|--------|------|
| `PROVIDER_KRX_API_ENABLED` | false | KRX API 활성화 (승인 필요) |
| `PROVIDER_YAHOO_ENABLED` | true | Yahoo Finance 활성화 |

**동작 방식**:
- KRX API 비활성화 시 Yahoo Finance로 자동 Fallback
- Yahoo Finance 심볼 자동 변환 (`005930` → `005930.KS`)
- CRYPTO는 Yahoo Finance 전용 (`BTC-USD` 형식)

#### 2.5.6 Standalone Data Collector (trader-collector) ⭐ v0.6.0

**목적**: API 서버와 독립적으로 데이터를 수집하는 Standalone 바이너리

**주요 기능**:
- 심볼 동기화: KRX, Binance, Yahoo Finance에서 종목 목록 동기화
- OHLCV 수집: 일봉 데이터 수집 (KRX API / Yahoo Finance)
- 지표 동기화: RouteState, MarketRegime, TTM Squeeze 등 분석 지표
- GlobalScore 동기화: 7Factor 기반 종합 점수 계산
- KRX Fundamental: PER/PBR/배당수익률/섹터 정보 (KRX API 활성화 시)

**CLI 명령어**:
```bash
# 개별 실행
trader-collector sync-symbols       # 심볼 동기화
trader-collector collect-ohlcv      # OHLCV 수집
trader-collector sync-indicators    # 지표 동기화
trader-collector sync-global-scores # GlobalScore 동기화

# 전체 워크플로우
trader-collector run-all            # 1회 실행
trader-collector daemon             # 데몬 모드
```

**환경변수**:
| 변수 | 기본값 | 설명 |
|------|--------|------|
| `OHLCV_BATCH_SIZE` | 50 | 배치당 심볼 수 |
| `OHLCV_STALE_DAYS` | 1 | 갱신 기준 일수 |
| `OHLCV_REQUEST_DELAY_MS` | 500 | API 요청 간 딜레이 |
| `DAEMON_INTERVAL_MINUTES` | 60 | 데몬 워크플로우 주기 |

**참조 문서**: `docs/data_collection.md`

#### 2.5.7 Fundamental 데이터 백그라운드 수집
- **목적**: 서버 실행 중 백그라운드에서 Fundamental 데이터를 주기적으로 배치 수집
- **수집 지표**:
  - 시가총액, 발행주식수, 52주 고저가
  - PER, PBR, ROE, ROA
  - 배당수익률, 배당성향
  - 영업이익률, 순이익률
  - 부채비율, 유동비율
- **수집 방식**:
  - Yahoo Finance API 연동
  - Rate Limiting 적용 (요청 간 2초 딜레이)
  - 7일 이상 경과한 데이터 자동 갱신
- **OHLCV 증분 업데이트**:
  - Fundamental 수집 시 동일 API 호출로 1년치 일봉 OHLCV도 함께 저장
  - ON CONFLICT DO UPDATE로 중복 없이 병합
- **환경변수**:
  | 변수 | 기본값 | 설명 |
  |------|--------|------|
  | `FUNDAMENTAL_COLLECT_ENABLED` | true | 수집기 활성화 |
  | `FUNDAMENTAL_COLLECT_INTERVAL_SECS` | 3600 | 수집 주기 (초) |
  | `FUNDAMENTAL_STALE_DAYS` | 7 | 갱신 기준 (일) |
  | `FUNDAMENTAL_BATCH_SIZE` | 50 | 배치당 처리 심볼 수 |
  | `FUNDAMENTAL_REQUEST_DELAY_MS` | 2000 | API 요청 간 딜레이 |
  | `FUNDAMENTAL_UPDATE_OHLCV` | true | OHLCV 증분 업데이트 |
  | `FUNDAMENTAL_AUTO_SYNC_SYMBOLS` | true | 심볼 자동 동기화 |

---

### 2.6 다중 타임프레임 (Multiple KLine Period) ⭐

> **참조 문서**: `docs/multiple_kline_period_requirements.md` (상세 요구사항 및 구현 방법론)

#### 2.6.1 개요

**Multiple KLine Period**는 단일 전략에서 여러 타임프레임의 캔들 데이터를 동시에 활용하여 더 정교한 매매 신호를 생성하는 기능입니다.

**핵심 개념**:
- **Primary Timeframe**: 전략의 주 실행 주기 (예: 5분)
- **Secondary Timeframe(s)**: 추가 분석용 타임프레임 (예: 1시간, 1일) - 최대 2개
- **멀티 타임프레임 분석 (MTF Analysis)**: 장기 추세 확인 + 중기 모멘텀 + 단기 진입 타이밍

**사용 예시**:
```
RSI 멀티 타임프레임 전략:
├─ 일봉 RSI > 50 → 상승 추세 확인 (Long 포지션만 허용)
├─ 1시간 RSI < 30 → 과매도 구간 (진입 신호 생성)
└─ 5분 RSI 반등 → 실제 진입 타이밍 결정
```

#### 2.6.2 전략 설정

전략 생성 시 다중 타임프레임을 다음과 같이 설정합니다:

```json
{
  "name": "RSI Multi Timeframe",
  "strategy_type": "RsiMultiTimeframe",
  "multi_timeframe_config": {
    "primary": "5m",
    "secondary": ["1h", "1d"],
    "lookback_periods": {
      "5m": 100,
      "1h": 50,
      "1d": 30
    }
  },
  "parameters": {
    "symbol": "BTCUSDT",
    "rsi_period_5m": 14,
    "rsi_period_1h": 14,
    "rsi_period_1d": 14,
    "oversold_threshold": 30,
    "overbought_threshold": 70
  }
}
```

**설정 제약**:
- Secondary 타임프레임은 Primary보다 **큰 타임프레임만 허용**
- 최대 3개 타임프레임 (Primary 1개 + Secondary 2개)
- 예: Primary=5m일 때, Secondary는 1h, 4h, 1d, 1w 등만 가능 (1m, 3m은 불가)

#### 2.6.3 데이터 조회

시스템은 전략 실행 시 필요한 모든 타임프레임 데이터를 자동으로 로드합니다:

**조회 방식**:
- **Redis 캐시 우선 조회** (멀티키 병렬 GET)
- **캐시 미스 시 PostgreSQL 조회** (단일 UNION ALL 쿼리)
- **타임프레임별 차등 TTL**:
  - 분봉: 60초
  - 시간봉: 300초
  - 일봉: 3600초

**성능 목표**:
- 3개 타임프레임 동시 조회: < 50ms (캐시 히트)
- DB 직접 조회: < 200ms

#### 2.6.4 전략 코드 작성

전략 코드에서 `StrategyContext`를 통해 타임프레임별 데이터에 접근합니다:

```rust
impl Strategy for RsiMultiTimeframeStrategy {
    async fn analyze(&self, ctx: &StrategyContext) -> Result<Signal> {
        // Primary Timeframe (5분)
        let klines_5m = ctx.primary_klines()?;
        let rsi_5m = calculate_rsi(klines_5m, self.config.rsi_period_5m);
        
        // Secondary Timeframes
        let klines_1h = ctx.get_klines(Timeframe::H1)?;
        let rsi_1h = calculate_rsi(klines_1h, self.config.rsi_period_1h);
        
        let klines_1d = ctx.get_klines(Timeframe::D1)?;
        let rsi_1d = calculate_rsi(klines_1d, self.config.rsi_period_1d);
        
        // 계층적 분석
        if rsi_1d > 50.0 && rsi_1h < 30.0 && rsi_5m < 30.0 {
            return Ok(Signal::Buy);
        }
        
        Ok(Signal::Hold)
    }
}
```

#### 2.6.5 Timeframe Alignment (시간 정렬)

시스템은 미래 데이터 누출을 방지하기 위해 타임프레임을 자동으로 정렬합니다:

**정렬 규칙**:
- Primary의 `open_time`을 기준으로 Secondary 데이터 필터링
- Secondary는 Primary의 `open_time` **이전** 데이터만 제공

**예시**:
```
Primary (5분봉): 2026-02-02 10:25:00 캔들
   ↓
Secondary (1시간봉): 2026-02-02 10:00:00 캔들 ✅ 사용 가능
                     2026-02-02 11:00:00 캔들 ❌ 미래 데이터 (제외)
   ↓
Secondary (일봉): 2026-02-02 00:00:00 캔들 ✅ 사용 가능
```

#### 2.6.6 백테스트 지원

백테스트 엔진은 히스토리 데이터에서 멀티 타임프레임 전략을 정확히 재현합니다:

- 각 타임스탬프마다 올바른 Secondary 데이터 로드
- 히스토리 캐싱으로 반복 쿼리 최소화
- 테스트 결과에 타임프레임별 신호 상세 기록 (디버깅용)

#### 2.6.7 실시간 거래

실시간 거래 시 WebSocket에서 여러 타임프레임을 동시에 구독합니다:

```rust
// 예: BTCUSDT 5분/1시간/일봉 동시 구독
let streams = vec![
    "btcusdt@kline_5m",
    "btcusdt@kline_1h",
    "btcusdt@kline_1d",
];
```

**업데이트 정책**:
- Primary 타임프레임 완료 시에만 전략 재평가
- Secondary 업데이트는 Context에 반영만 하고 즉시 실행하지 않음

#### 2.6.8 API 엔드포인트

**전략 타임프레임 설정 조회**:
```
GET /api/v1/strategies/{id}/timeframes
```

**응답**:
```json
{
  "strategy_id": 123,
  "primary": {
    "timeframe": "5m",
    "description": "5분봉",
    "last_update": "2026-02-02T10:25:00Z"
  },
  "secondary": [
    {
      "timeframe": "1h",
      "description": "1시간봉",
      "last_update": "2026-02-02T10:00:00Z"
    },
    {
      "timeframe": "1d",
      "description": "일봉",
      "last_update": "2026-02-02T00:00:00Z"
    }
  ]
}
```

**멀티 타임프레임 캔들 데이터 조회** (디버깅용):
```
GET /api/v1/klines/multi?symbol=BTCUSDT&timeframes=5m,1h,1d&limit=50
```

#### 2.6.9 UI/UX

**SDUI 스키마**에서 멀티 타임프레임 선택 UI:

```json
{
  "type": "multi-select",
  "id": "secondary_timeframes",
  "label": "보조 타임프레임 (최대 2개)",
  "options": [
    {"value": "1h", "label": "1시간"},
    {"value": "4h", "label": "4시간"},
    {"value": "1d", "label": "1일"},
    {"value": "1w", "label": "1주"}
  ],
  "max_selections": 2,
  "validation": "larger_than_primary"
}
```

**프론트엔드 컴포넌트**: `MultiTimeframeSelector.tsx`

#### 2.6.10 기대 효과

| 효과 | 설명 |
|------|------|
| **신호 신뢰도 향상** | 장기 추세 + 단기 타이밍 조합으로 정확도 증가 |
| **허위 신호 필터링** | 여러 타임프레임 합의 필요 → 노이즈 감소 |
| **전문적 분석** | 기관/전문가가 사용하는 MTF 기법 적용 |
| **전략 다양성** | 새로운 유형의 전략 개발 가능 |
| **리스크 관리** | 상위 타임프레임 추세 역행 시 진입 금지 |

---

### 2.7 매매 일지 (Trading Journal)

#### 2.6.1 체결 내역 동기화
- 거래소에서 체결 내역 자동 동기화:
  - KIS: 국내/해외 체결 내역
  - Binance: 현물/선물 체결 내역
- 동기화 주기: 수동 또는 자동 (설정 가능)

#### 2.6.2 종목별 보유 현황
- 보유 종목 상세 정보:
  - 보유 수량
  - 평균 매입가 (물타기 시 가중평균 자동 계산)
  - 투자 금액 (총 매입가)
  - 평가 금액 (현재가 × 수량)
  - 포트폴리오 내 비중 (%)

#### 2.6.3 매매 이력 타임라인
- 종목별 거래 히스토리:
  - 매수/매도 시점, 가격, 수량
  - 물타기/분할매도 기록
- 시간순 타임라인 뷰

#### 2.6.4 손익 분석
- **실현 손익**: 청산된 거래의 확정 손익
- **미실현 손익**: 보유 중인 포지션의 평가손익
- **기간별 수익률**:
  - 일별, 주별, 월별, 연도별
  - 누적 수익률 곡선

#### 2.6.5 투자 인사이트
- 매매 패턴 분석:
  - 평균 보유 기간
  - 승률, 손익비
- 리밸런싱 추천:
  - 목표 비중 대비 현재 비중 비교
  - 조정 필요 종목 표시

---

### 2.8 ML 예측

#### 2.7.1 모델 훈련
- 지원 알고리즘:
  - XGBoost
  - LightGBM
  - RandomForest
- 훈련 데이터:
  - OHLCV 기반 특징 추출 (22개 기술 지표)
  - **구조적 피처** (6개): 저점 추세, 거래량 질, 박스권 위치, MA 이격도, BB 폭, RSI
  - 레이블: 다음 기간 수익률 방향 (상승/하락)
- 훈련 환경:
  - ONNX 형식으로 저장 후 Rust Runtime에서 추론

#### 2.7.4 구조적 피처 (Structural Features)
- **목적**: "살아있는 횡보"와 "죽은 횡보"를 구분하여 돌파 가능성 예측
- **피처 목록**:
  | 피처 | 설명 | 의미 |
  |------|------|------|
  | `low_trend` | 저점 상승 강도 (Higher Low) | 양수면 저점이 올라가는 중 |
  | `vol_quality` | 양봉/음봉 거래량 비율 | 1 초과면 매수세 우위 |
  | `range_pos` | 박스권 내 위치 (0~1) | 0.8 이상이면 돌파 임박 |
  | `dist_ma20` | MA20 이격도 | 0 근처가 눌림목 구간 |
  | `bb_width` | 볼린저 밴드 폭 | 낮을수록 에너지 응축 |
  | `rsi` | RSI 14일 | 과매수/과매도 필터링 |
- **활용**:
  - ML 모델 입력 피처로 추가
  - 스크리닝 필터 조건으로 활용
  - RouteState 판정 로직에 반영

#### 2.7.5 TTM Squeeze 지표 (John Carter)

**목적**: Bollinger Band가 Keltner Channel 내부로 들어가면 에너지 응축 상태

**계산 방식**:
1. **Bollinger Band** (BB): 20일 SMA ± 2σ
2. **Keltner Channel** (KC): 20일 EMA ± 1.5 × ATR(20)
3. **Squeeze 판정**: BB_upper < KC_upper AND BB_lower > KC_lower
4. **Release 판정**: 이전 봉은 Squeeze, 현재 봉은 Squeeze 해제

**출력 형식**:
```rust
pub struct TtmSqueeze {
    pub is_squeeze: bool,        // 현재 스퀴즈 상태
    pub squeeze_count: u32,      // 연속 스퀴즈 일수
    pub momentum: Decimal,       // 스퀴즈 모멘텀 (방향)
    pub released: bool,          // 이번 봉에서 해제되었는가?
}
```

**활용**:
- RouteState ATTACK 판정 (Release + Momentum > 0)
- TRIGGER 시스템에 +30점 기여
- 변동성 돌파 전략 필터링

**DB 저장**:
- `symbol_fundamental` 테이블에 컬럼 추가:
  - `ttm_squeeze`: BOOLEAN
  - `ttm_squeeze_cnt`: INTEGER (연속 일수)

**예상 구현**: v0.6.0 (TODO Phase 1-2.3)

#### 2.7.6 추가 기술적 지표

**목적**: 분석 정확도 향상을 위한 고급 지표

**4개 신규 지표**:
| 지표 | 설명 | 용도 |
|------|------|------|
| **HMA** | Hull Moving Average | 빠른 반응, 낮은 휩소 |
| **OBV** | On-Balance Volume | 스마트 머니 추적 |
| **SuperTrend** | 추세 추종 지표 | 트렌드 방향 판정 |
| **CandlePattern** | 캔들 패턴 감지 | 망치형, 장악형 등 |

**구현 위치**:
```
trader-analytics/src/indicators/
├── hma.rs         // Hull Moving Average
├── obv.rs         // On-Balance Volume
├── supertrend.rs  // SuperTrend
└── candle_patterns.rs // 캔들 패턴 감지
```

**활용**:
- TRIGGER 시스템에 캔들 패턴 연동
- 전략 신호 생성에 활용
- 구조적 피처 확장

**예상 구현**: v0.6.0 (TODO Phase 1-2.6)

#### 2.7.2 모델 관리
- 모델 등록 API:
  - ONNX 파일 경로, 메타데이터
  - 훈련 심볼, 정확도 지표
- 모델 버전 관리:
  - 심볼별 최신 모델 관리
  - 모델 배포/롤백

#### 2.7.3 예측 활용
- 전략에서 ML 예측 결과 사용:
  - 진입 신호 필터링 (ML이 상승 예측 시만 매수)
  - 예측 확률 기반 포지션 크기 조절
- 패턴 인식 통합:
  - 26개 캔들스틱 패턴
  - 24개 차트 패턴

---

## 3. 비기능 요구사항

### 3.1 성능
| 항목 | 요구사항 |
|------|---------|
| API 응답 시간 | 일반 조회 < 200ms, 백테스트 < 5초 (1년 데이터) |
| 동시 전략 | 10개 이상 동시 실행 |
| 데이터 처리 | 100만 캔들 백테스트 < 30초 |
| WebSocket | 틱 데이터 지연 < 100ms |

### 3.2 보안
| 항목 | 요구사항 |
|------|---------|
| API Key 저장 | AES-256-GCM 암호화 |
| 환경 변수 | 민감 정보 환경 변수로 관리 |
| 접근 제어 | 로컬 실행 (외부 접근 차단) |

### 3.3 가용성
| 항목 | 요구사항 |
|------|---------|
| 실행 환경 | 로컬 PC (Windows/Linux/macOS) |
| 데이터베이스 | TimescaleDB (PostgreSQL 확장) |
| 캐시 | Redis |
| 컨테이너 | Docker/Podman |

### 3.4 확장성
| 항목 | 요구사항 |
|------|---------|
| 전략 추가 | 새로운 전략 플러그인 구조 |
| 거래소 추가 | Exchange trait 구현으로 확장 |
| 지표 추가 | Indicator trait 구현으로 확장 |

---

## 4. 기술 스택

| 계층 | 기술 | 용도 |
|------|------|------|
| Backend | Rust, Tokio, Axum | 고성능 비동기 API 서버 |
| Database | PostgreSQL + TimescaleDB | 시계열 데이터 저장 |
| Cache | Redis | 세션, 실시간 데이터 캐시 |
| Frontend | SolidJS, TypeScript, Vite | 반응형 SPA |
| Exchange | KIS (KR/US 통합), Binance, Mock | 거래소 연동 (v0.8.0 통합) |
| WebSocket | tokio-tungstenite, mpsc | 실시간 시세 연동 (v0.8.0) |
| ML | ONNX Runtime, Python | 모델 추론, 훈련 |
| Infra | Podman/Docker | 컨테이너화된 인프라 |

---

## 5. 지원 거래소

### 5.1 Binance
- **시장**: 암호화폐 현물, 선물
- **기능**: 잔고 조회, 주문 실행, 체결 내역, WebSocket 실시간 시세
- **인증**: API Key + Secret

### 5.2 KIS (한국투자증권) ⭐ v0.8.0 대폭 확장
- **시장**: 국내 주식, 해외 주식 (미국)
- **기능**: 잔고 조회, 주문 실행, 체결 내역 조회, WebSocket 실시간 시세
- **인증**: OAuth 2.0 (App Key, App Secret, 계좌번호)
- **계좌 유형**: 일반, ISA, 연금
- **v0.8.0 변경사항**:
  - KR/US 프로바이더 통합 (`kis_kr.rs` + `kis_us.rs` → `kis.rs`)
  - WebSocket 동적 구독 (연결 중 심볼 추가/해제)
  - 공통 HTTP 클라이언트 (`client.rs`) 추출
  - Bridge Task 기반 KR/US 동시 수신
  - DB 기반 자격증명 관리 (환경변수 의존 제거)

### 5.3 Mock Exchange ⭐ v0.8.0 신규 → v0.9.0 KIS 수준 업그레이드

- **용도**: 개발/테스트용 가상 거래소 (24시간 페이퍼 트레이딩)
- **구현**: `ExchangeProvider` + `OrderExecutionProvider` trait 완전 구현 (`provider/mock.rs`)

#### 5.3.1 현실적 가격 스트리밍 (v0.9.0)

장외 시간에도 현실적인 가격 변동을 WebSocket으로 제공:

| 모드 | 설명 | 데이터 소스 |
|------|------|------------|
| **HistoricalReplay** | 과거 캔들 데이터를 실시간 속도로 재생 | DB 1분봉 → 틱 보간 |
| **RandomWalk** | 정규분포 랜덤 워크 + 평균 회귀 | D1 캔들 ATR 기반 |
| **YahooLegacy** | 기존 Yahoo D1 종가 방식 (하위 호환) | Yahoo Finance |

**스트리밍 데이터 (KIS 수준)**:
- **Ticker**: 체결가, bid/ask, 거래량, 등락률
- **OrderBook**: KR 10단계 호가 (`KrxTickSize` 7단계), US 1단계 호가 (`UsEquityTickSize` $0.01)

**설정**: `MockStreamingConfig` (모드, 발행 간격, OrderBook 발행 여부, 변동성 등)

구현 파일: `provider/mock_streaming.rs` (신규)

#### 5.3.2 주문 매칭 엔진 (v0.9.0)

KIS 거래소 수준의 주문 실행 시뮬레이션:

| 기능 | 설명 |
|------|------|
| **시장가 주문** | bid/ask 즉시 체결, OrderBook 기반 VWAP |
| **지정가 주문** | 조건 충족 시 자동 체결 (Limit Buy: ask ≤ limit) |
| **스톱 주문** | StopLoss/TakeProfit/StopLossLimit/TakeProfitLimit |
| **미체결 관리** | 주문 큐 + 매 틱마다 자동 매칭 |
| **부분 체결** | OrderBook 호가 잔량 기반 부분 체결 + VWAP 계산 |
| **주문 정정/취소** | 수량·가격 변경, 예약 잔고 자동 재계산 |
| **잔고 예약** | 매수 주문 등록 시 필요 금액 예약 (이중 사용 방지) |
| **주문 영속성** | DB 저장/복원 (`mock_pending_orders` 테이블) |

**매칭 규칙**:

| 주문 유형 | 트리거 조건 | 체결 방식 |
|----------|-----------|----------|
| Limit Buy | ask ≤ limit_price | 지정가 이하 체결 |
| Limit Sell | bid ≥ limit_price | 지정가 이상 체결 |
| StopLoss Sell | last ≤ stop_price | 시장가 전환 후 체결 |
| StopLoss Buy | last ≥ stop_price | 시장가 전환 후 체결 |
| TakeProfit Sell | last ≥ stop_price | 시장가 전환 후 체결 |
| StopLossLimit | last crosses stop → Limit 전환 | 지정가 조건 충족 시 체결 |

구현 파일: `provider/mock_order_engine.rs` (신규)

#### 5.3.3 기존 기능 (v0.8.0, 하위 호환)

- 가상 잔고/포지션 관리 (전략별 독립)
- `process_signal()` 즉시 체결 경로 유지 (SimulatedExecutor용)
- Yahoo D1 기반 스트리밍 (YahooLegacy 모드)

### 5.4 추가 거래소 (선택적 확장)
- Coinbase, Kraken (암호화폐)
- Interactive Brokers, 키움증권 (주식)

---

## 6. 전략 목록

### 6.1 단일 자산 전략 (9개)

| 전략 | 설명 | 주요 파라미터 |
|------|------|-------------|
| Grid Trading | 가격 구간별 매수/매도 주문 | 그리드 수, 가격 범위 |
| RSI Mean Reversion | RSI 과매도/과매수 기반 매매 | RSI 기간, 과매도/과매수 임계값 |
| Bollinger Bands | 볼린저 밴드 이탈 시 평균회귀 | 기간, 표준편차 배수 |
| Volatility Breakout | 전일 변동성 돌파 시 진입 | K 계수 |
| Magic Split | 분할 매수/매도 | 분할 횟수, 간격 비율 |
| SMA Crossover | 이동평균 골든/데드 크로스 | 단기/장기 이동평균 기간 |
| Trailing Stop | 트레일링 스탑 기반 청산 | 트레일링 비율 |
| Candle Pattern | 캔들 패턴 인식 매매 | 패턴 유형, 확인 캔들 수 |
| Infinity Bot | 무한 분할 매수 (물타기) | 라운드 수, 매수 간격 |

### 6.2 자산배분 전략 (16개+)

| 전략 | 설명 |
|------|------|
| Momentum Power | 단순 모멘텀 기반 자산배분 |
| HAA | 계층적 자산배분 (Hierarchical Asset Allocation) |
| XAA | 확장 자산배분 (Extended Asset Allocation) |
| All Weather | 전천후 포트폴리오 |
| Compound Momentum | 복리 모멘텀 전략 |
| Stock Rotation | 종목 로테이션 |
| Market Cap Top | 시총 상위 N종목 |
| Market Interest Day | 관심도 기반 단기 매매 |
| Dual Momentum | 절대/상대 모멘텀 조합 |
| BAA | 공격적 자산배분 (Bold Asset Allocation) |
| US 3X Leverage | 레버리지 ETF 전략 |
| Range Trading | 박스권 구간 매매 |
| Momentum Surge | 급등 모멘텀 포착 |
| Sector VB | 섹터 변동성 돌파 |
| Market BothSide | 시장 양방향 전략 |
| Small Cap Quant | 소형주 퀀트 전략 |
| Sector Momentum | 섹터 로테이션 모멘텀 |
| Pension Portfolio | 연금 자산배분 전략 |

### 6.3 추가 전략 (선택적)

| 전략 | 설명 |
|------|------|
| SPAC Arbitrage | 스팩 차익거래 전략 |
| ETF Batch | ETF 일괄 투자 |
| Rotation Savings | 로테이션 적금 전략 |
| Cross Market | 국내주식+해외채권 혼합 |

### 6.4 전략 구현 원칙

> **저작권 고려**: 모든 전략은 공개된 학술 논문, 기술적 분석 이론, 일반적인 투자 원칙에 기반하여 독자적으로 구현되었습니다.
> 특정 상용 제품이나 유료 서비스의 로직을 직접 복제하지 않습니다.

**구현 방식**:
- 기술적 지표(RSI, MACD, BB 등)는 공개된 수식 기반 구현
- 자산배분 전략은 일반적인 포트폴리오 이론 기반 (Modern Portfolio Theory 등)
- 모멘텀 전략은 학술적으로 검증된 팩터 투자 원칙 적용

---

### 2.8 종목 스크리닝 (Symbol Screening)

#### 2.8.1 스크리닝 개요
- **목적**: 전체 시장에서 특정 조건을 만족하는 종목을 필터링하여 전략에 활용
- **데이터 소스**:
  - Fundamental 데이터 (PER, PBR, ROE, 시가총액 등)
  - OHLCV 데이터 (가격 변동률, 거래량 등)
  - 심볼 정보 (시장, 거래소, 섹터)
- **활용**:
  - 전략에서 스크리닝 결과를 유니버스로 사용
  - 사용자 정의 스크리닝 조건으로 종목 탐색
  - 프리셋 스크리닝 (가치주, 고배당주, 성장주 등)

#### 2.8.2 Fundamental 필터
- **밸류에이션 지표**:
  - PER (Price to Earnings Ratio): 주가수익비율
  - PBR (Price to Book Ratio): 주가순자산비율
  - PSR (Price to Sales Ratio): 주가매출비율
  - EV/EBITDA: 기업가치 대비 EBITDA
- **수익성 지표**:
  - ROE (Return on Equity): 자기자본이익률
  - ROA (Return on Assets): 총자산이익률
  - Operating Margin: 영업이익률
  - Net Profit Margin: 순이익률
- **배당 지표**:
  - Dividend Yield: 배당수익률
  - Dividend Payout Ratio: 배당성향
- **안정성 지표**:
  - Debt Ratio: 부채비율
  - Current Ratio: 유동비율
  - Quick Ratio: 당좌비율
- **성장성 지표**:
  - Revenue Growth (YoY): 매출 성장률
  - Earnings Growth (YoY): 이익 성장률
  - Revenue Growth (3Y CAGR): 3년 매출 성장률
- **규모 지표**:
  - Market Cap: 시가총액
  - 52주 최고가/최저가

#### 2.8.3 기술적 필터 (OHLCV 기반)
- **가격 변동률**:
  - 1일 변동률 (당일 대비 전일)
  - 5일 변동률 (5거래일 전 대비)
  - 20일 변동률 (한 달 전 대비)
- **거래량 지표**:
  - Volume Ratio: 평균 거래량 대비 현재 거래량 배율
  - 최소 평균 거래량 필터
- **52주 고저가 대비**:
  - 52주 고가 대비 하락률 (예: 고가 대비 10% 이내)
  - 52주 저가 대비 상승률 (예: 저가 대비 20% 이상)

#### 2.8.4 프리셋 스크리닝
| 프리셋 | 설명 | 주요 조건 |
|--------|------|----------|
| 가치주 (Value) | 저평가 우량주 | PER ≤ 15, PBR ≤ 1.0, ROE ≥ 5% |
| 고배당주 (Dividend) | 안정적 고배당 | 배당수익률 ≥ 3%, ROE ≥ 5%, 부채비율 ≤ 100% |
| 성장주 (Growth) | 고성장 기업 | 매출성장률 ≥ 20%, 이익성장률 ≥ 15%, ROE ≥ 10% |
| 스노우볼 (Snowball) | 저PBR + 고배당 | PBR ≤ 1.0, 배당수익률 ≥ 3%, 부채비율 ≤ 80%, ROE ≥ 8% |
| 대형주 (Large Cap) | 시총 상위 | 시가총액 ≥ 10조원 |
| 52주 신저가 근접 | 바닥 매수 전략 | 52주 고가 대비 ≥ 50% 하락, ROE ≥ 5% |

#### 2.8.5 전략 연계
- **코스닥 급등주 (KOSDAQ Fire Rain)**:
  - 거래량 급증 (평균 대비 3배 이상)
  - 가격 상승 (전일 대비 5% 이상)
  - 시가총액 필터 (소형주 중심)
- **스노우볼 전략**:
  - 저PBR + 고배당 스크리닝 결과를 유니버스로 사용
  - 매월 리밸런싱 시 스크리닝 재실행
- **섹터 모멘텀**:
  - 섹터별 상위 모멘텀 종목 스크리닝
  - OHLCV 기반 가격 변동률 정렬

#### 2.8.6 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/screening` | POST | 커스텀 스크리닝 실행 |
| `/api/v1/screening/presets` | GET | 사용 가능한 프리셋 목록 |
| `/api/v1/screening/presets/{preset}` | GET | 프리셋 스크리닝 실행 |
| `/api/v1/screening/momentum` | GET | 모멘텀 기반 스크리닝 |

#### 2.8.7 응답 데이터
- 심볼 기본 정보 (티커, 종목명, 시장, 거래소, 섹터)
- Fundamental 지표 (PER, PBR, ROE, 시가총액, 배당수익률 등)
- 기술적 지표 (현재가, 변동률, 거래량 비율, 52주 고저가 대비)
- 정렬 및 페이지네이션 지원

---

#### 2.8.8 거시 환경 필터 (MacroFilter)

**목적**: USD/KRW 환율, 나스닥 지수 모니터링으로 시장 위험도 평가 및 동적 진입 기준 조정

**3단계 리스크 레벨**:
| 레벨 | 조건 | 조치 |
|------|------|------|
| **Critical** | 환율 ≥ 1400원 OR 나스닥 -2% 이상 | EBS +1, 추천 3개로 제한 |
| **High** | 환율 +0.5% 급등 | EBS +1, 추천 5개로 제한 |
| **Normal** | 기본 상태 | EBS 4, 추천 10개 |

**출력 형식**:
```rust
pub struct MacroEnvironment {
    pub risk_level: MacroRisk,
    pub usd_krw: Decimal,
    pub usd_change_pct: f64,
    pub nasdaq_change_pct: f64,
    pub adjusted_ebs: u8,          // 조정된 EBS 기준
    pub recommendation_limit: usize, // 추천 종목 수 제한
}
```

**데이터 소스**:
- USD/KRW: Yahoo Finance `KRW=X`
- 나스닥: Yahoo Finance `^IXIC`
- 갱신 주기: 1시간

**활용**:
- 전략 진입 차단 (Critical 시 신규 진입 중지)
- Global Score EBS 기준 동적 조정
- 텔레그램 알림 (리스크 상승 시)

**API 엔드포인트**:
- `GET /api/v1/market/macro`: 현재 거시 환경 조회
- 스크리닝 응답에 `macro_risk` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.4)

#### 2.8.9 시장 온도 지표 (MarketBreadth)

**목적**: 20일선 상회 종목 비율로 시장 전체 건강 상태 측정

**3단계 온도**:
| 온도 | Above_MA20 비율 | 의미 |
|------|----------------|------|
| Overheat 🔥 | ≥ 65% | 과열 (조정 임박) |
| Neutral 🌤 | 35~65% | 중립 (정상) |
| Cold 🧊 | ≤ 35% | 냉각 (반등 대기) |

**출력 형식**:
```rust
pub struct MarketBreadth {
    pub all: f64,
    pub kospi: f64,
    pub kosdaq: f64,
    pub temperature: MarketTemperature,
}
```

**계산 방식**:
- 전체 종목 중 종가 > SMA(20) 비율
- 시장별 개별 계산 (KOSPI, KOSDAQ)

**활용**:
- 시장 타이밍 (Overheat 시 신규 진입 신중)
- 대시보드 위젯 (시장 온도 게이지)
- 전략 필터링

**API 엔드포인트**:
- `GET /api/v1/market/breadth`: 현재 시장 온도 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.5)

#### 2.8.10 섹터 분석 (SectorRS)

**목적**: 시장 대비 초과수익(Relative Strength)으로 진짜 주도 섹터 발굴

**계산 방식**:
- `rel_20d_%`: 20일 전 대비 수익률
- `sector_rs`: 섹터 평균 `rel_20d_%`
- `market_rs`: 전체 시장 평균 `rel_20d_%`
- `excess_return`: `sector_rs - market_rs`

**종합 섹터 점수**:
```
score = RS × 0.6 + 단순수익 × 0.4
```

**출력 형식**:
- 스크리닝 응답에 `sector_rs`, `sector_rank` 필드 추가
- 섹터별 순위 (1~11)

**11개 섹터 분류 (GICS)**:
- 에너지, 소재, 산업재, 경기소비재
- 필수소비재, 헬스케어, 금융, IT
- 커뮤니케이션, 유틸리티, 부동산

**활용**:
- 섹터 모멘텀 전략 (상위 3개 섹터 집중)
- 섹터 로테이션 전략
- 대시보드 섹터 히트맵

**API 엔드포인트**:
- `GET /api/v1/market/sectors`: 섹터별 RS 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.7)

---

### 2.9 종목 랭킹 시스템 (Global Score)

#### 2.9.1 개요
- **목적**: 모든 기술적 지표를 단일 점수(GLOBAL_SCORE 0~100)로 종합하여 종목 순위 산출
- **활용**: 스크리닝 결과 정렬, TOP N 종목 추천, 전략 유니버스 선정

#### 2.9.2 스코어링 팩터 (가중치 합계 = 1.0)
| 팩터 | 가중치 | 설명 |
|------|--------|------|
| Risk/Reward | 0.25 | 목표가 대비 손절가 비율 |
| Target Room | 0.18 | 현재가 대비 목표가 여유율 |
| Stop Room | 0.12 | 현재가 대비 손절가 여유율 |
| Entry Proximity | 0.12 | 추천 진입가 근접도 |
| Momentum | 0.10 | ERS + MACD 기울기 + RSI 중심 보너스 |
| Liquidity | 0.13 | 거래대금 퍼센타일 |
| Technical Balance | 0.10 | 변동성(VolZ) 스윗스팟 + 이격도 안정성 |

#### 2.9.3 페널티 시스템 (점수 차감)
| 조건 | 페널티 | 설명 |
|------|--------|------|
| 5일 과열 | -6점 | 5일 수익률 +10% 초과 시 |
| 10일 과열 | -6점 | 10일 수익률 +20% 초과 시 |
| RSI 이탈 | -4점 | RSI 45~65 밴드 이탈 |
| MACD 음수 | -4점 | MACD 기울기 음수 |
| 진입 괴리 | -4점 | 추천가 대비 현재가 괴리 과다 |
| 저유동성 | -4점 | 거래대금 하위 20% |
| 변동성 스파이크 | -2점 | VolZ > 3 |

#### 2.9.4 유동성 게이트 (시장별)
| 시장 | 최소 거래대금 | 완화 기준 |
|------|--------------|----------|
| KR-KOSPI | 200억원 | 150억원 |
| KR-KOSDAQ | 100억원 | 80억원 |
| US-NYSE/NASDAQ | $100M | $50M |
| JP-TSE | ¥10B | ¥5B |

#### 2.9.5 품질 게이트
- **EBS (Entry Balance Score)**: 진입 조건 균형 점수
- 기본 통과 기준: EBS ≥ 4
- 후보 부족 시 자동 완화: EBS ≥ 3

#### 2.9.6 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/ranking/global` | POST | 글로벌 랭킹 조회 |
| `/api/v1/ranking/top` | GET | TOP N 종목 조회 |

---

#### 2.9.7 추천 검증 (RealityCheck)

**목적**: 전일 추천 종목의 익일 실제 성과 자동 검증

**2개 신규 테이블 (TimescaleDB Hypertable)**:

**price_snapshot 테이블**:
```sql
CREATE TABLE price_snapshot (
    snapshot_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    close_price DECIMAL(18,4),
    volume BIGINT,
    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (snapshot_date, symbol)
);
SELECT create_hypertable('price_snapshot', 'snapshot_date');
```

**reality_check 테이블**:
```sql
CREATE TABLE reality_check (
    check_date DATE NOT NULL,
    recommend_date DATE NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    recommend_rank INT,
    recommend_score DECIMAL(5,2),
    entry_price DECIMAL(18,4),
    next_close DECIMAL(18,4),
    return_pct DECIMAL(8,4),
    is_win BOOLEAN,
    holding_days INT DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (check_date, symbol)
);
SELECT create_hypertable('reality_check', 'check_date');
```

**워크플로우**:
1. 매일 종가 시점에 TOP 10 스냅샷 저장 (`price_snapshot`)
2. 익일 종가에 전일 스냅샷과 비교 (`reality_check`)
3. 승률, 평균 수익률 계산

**출력 지표**:
- 추천 종목 승률 (전체, 7일, 30일)
- 평균 수익률
- 최고/최저 수익률
- 레짐별 성과 (MarketRegime 연동)

**활용**:
- 전략 신뢰도 측정
- 백테스트 vs 실거래 괴리 분석
- 파라미터 튜닝 피드백
- 대시보드 성과 위젯

**API 엔드포인트**:
- `GET /api/v1/reality-check/stats`: 통계 조회
- `GET /api/v1/reality-check/history?days=30`: 이력 조회

**예상 구현**: v0.6.0 (TODO Phase 1-2.8)

#### 2.9.8 대시보드 위젯

**시장 심리 지표**:
- `FearGreedGauge`: RSI + Disparity 기반 0~100 게이지
- `MarketBreadthWidget`: 20일선 상회 비율 게이지
- `MacroRiskPanel`: 환율, 나스닥 상태 표시

**팩터 분석 차트**:
- `RadarChart7Factor`: 7개 팩터 레이더 차트
- `ScoreWaterfall`: 점수 기여도 워터폴
- `KellyVisualization`: 켈리 자금관리 바

**포트폴리오 분석**:
- `CorrelationHeatmap`: TOP 10 상관관계 히트맵
- `VolumeProfile`: 매물대 가로 막대 오버레이
- `OpportunityMap`: TOTAL vs TRIGGER 산점도

**상태 관리 UI**:
- `KanbanBoard`: ATTACK/ARMED/WATCH 3열 칸반
- `SurvivalBadge`: 생존일 뱃지 (연속 상위권 일수)
- `RegimeSummaryTable`: 레짐별 평균 성과

**섹터 시각화**:
- `SectorTreemap`: 거래대금 기반 트리맵
- `SectorMomentumBar`: 5일 수익률 Top 10

**예상 구현**: v0.6.0 (TODO Phase 2-5)

---

### 2.10 종목 상태 관리 (RouteState)

#### 2.10.1 상태 정의
| 상태 | 설명 | 액션 |
|------|------|------|
| `ATTACK` | 공략 - 진입 시그널 발생 | 매수 검토 |
| `ARMED` | 임박 - 발사 준비 완료 | 모니터링 강화 |
| `WAIT` | 대기 - 추세 양호, 타점 대기 | 관찰 유지 |
| `OVERHEAT` | 과열 - 단기 급등 | 익절/주의 |
| `NEUTRAL` | 중립 - 특별 신호 없음 | 기본 관찰 |

#### 2.10.2 상태 판정 기준
- **ATTACK**: TTM Squeeze 해제 + 모멘텀 상승 + RSI 적정대
- **ARMED**: 박스권 상단 + 거래량 증가 + 저점 상승
- **WAIT**: 정배열 + MA 지지 + 눌림목
- **OVERHEAT**: 5일 수익률 > 15% 또는 RSI > 70
- **NEUTRAL**: 위 조건 미충족

#### 2.10.3 활용
- 스크리닝 결과에 상태 표시
- 전략에서 상태 기반 필터링
- 알림 시스템 연동 (ATTACK 상태 시 푸시 알림)

---

#### 2.10.4 시장 추세 분류 (MarketRegime)

**목적**: 종목의 추세 단계를 5단계로 분류하여 매매 타이밍 판단

**5단계 레짐**:
| 레짐 | 조건 | 의미 |
|------|------|------|
| StrongUptrend | rel_60d > 10% + slope > 0 + RSI 50~70 | ① 강한 상승 추세 |
| Correction | rel_60d > 5% + slope ≤ 0 | ② 상승 후 조정 |
| Sideways | -5% ≤ rel_60d ≤ 5% | ③ 박스 / 중립 |
| BottomBounce | rel_60d ≤ -5% + slope > 0 | ④ 바닥 반등 시도 |
| Downtrend | rel_60d < -5% + slope < 0 | ⑤ 하락 / 약세 |

**계산 지표**:
- `rel_60d_%`: 60일 전 종가 대비 현재 수익률
- `slope`: 60일 선형 회귀 기울기
- `RSI`: 14일 RSI

**활용**:
- RouteState 판정에 활용 (Downtrend → NEUTRAL 고정)
- 전략 필터링 (Downtrend 종목 진입 차단)
- 스크리닝 API에 `regime` 필드 추가

**API 엔드포인트**:
- `GET /api/v1/market/regime/{symbol}`: 종목별 레짐 조회
- 스크리닝 응답에 `market_regime` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.1)

#### 2.10.5 진입 신호 강도 (TRIGGER)

**목적**: 여러 기술적 조건을 종합하여 진입 신호 강도(0~100점) 산출

**6가지 트리거 유형**:
| 트리거 | 점수 | 조건 |
|--------|------|------|
| SqueezeBreak | +30점 | TTM Squeeze 해제 |
| BoxBreakout | +25점 | 박스권 상단 돌파 (Range_Pos ≥ 0.85) |
| VolumeSpike | +20점 | 거래량 평균 대비 3배 이상 |
| MomentumUp | +15점 | MACD 기울기 > 0 |
| HammerCandle | +10점 | 망치형 캔들 패턴 |
| Engulfing | +10점 | 장악형 캔들 패턴 |

**출력 형식**:
```rust
pub struct TriggerResult {
    pub score: f64,              // 0~100 (중복 가능)
    pub triggers: Vec<TriggerType>,
    pub label: String,           // "🚀급등시동, 📦박스돌파"
}
```

**활용**:
- RouteState ATTACK 판정 (TRIGGER ≥ 50점)
- Global Score 모멘텀 팩터에 반영
- 스크리닝 정렬 기준
- 텔레그램 알림 (고강도 신호 발생 시)

**API 엔드포인트**:
- 스크리닝 응답에 `trigger_score`, `trigger_label` 필드 포함

**예상 구현**: v0.6.0 (TODO Phase 1-2.2)

---

### 2.11 관심종목 관리 (Watchlist) ⭐ v0.6.0

#### 2.11.1 개요
**목적**: 사용자별 관심종목 그룹을 생성하고 관리

**핵심 기능**:
- 관심종목 그룹 생성 (예: "반도체 관련주", "배당주")
- 그룹별 종목 추가/삭제
- 순서 관리 (드래그 앤 드롭)
- 그룹 공유 (선택적)

#### 2.11.2 데이터 모델
```sql
CREATE TABLE watchlist (
    id SERIAL PRIMARY KEY,
    user_id INTEGER,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE watchlist_item (
    id SERIAL PRIMARY KEY,
    watchlist_id INTEGER REFERENCES watchlist(id),
    symbol VARCHAR(20) NOT NULL,
    sort_order INTEGER DEFAULT 0,
    added_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 2.11.3 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/watchlist` | GET | 관심종목 그룹 목록 |
| `/api/v1/watchlist` | POST | 그룹 생성 |
| `/api/v1/watchlist/{id}` | PUT | 그룹 수정 |
| `/api/v1/watchlist/{id}` | DELETE | 그룹 삭제 |
| `/api/v1/watchlist/{id}/items` | POST | 종목 추가 |
| `/api/v1/watchlist/{id}/items/{symbol}` | DELETE | 종목 삭제 |

---

### 2.12 7Factor 종합 점수 시스템 ⭐ v0.6.0

#### 2.12.1 개요
**목적**: 7가지 팩터를 통합한 종합 스코어링 시스템

**7개 팩터**:
| 팩터 | 설명 | 지표 |
|------|------|------|
| **Momentum** | 가격 상승 추세 | ERS, MACD 기울기, RSI |
| **Value** | 저평가 정도 | PER, PBR |
| **Quality** | 재무 건전성 | ROE, 부채비율 |
| **Volatility** | 변동성 안정성 | ATR, VolZ |
| **Liquidity** | 유동성 | 거래대금 퍼센타일 |
| **Growth** | 성장성 | 매출 성장률, 이익 성장률 |
| **Sentiment** | 시장 심리 | 이격도, RSI 중립도 |

#### 2.12.2 점수 계산
- 각 팩터: 0~100점 정규화
- 가중치 기반 종합 점수 (GLOBAL_SCORE)
- 페널티 시스템 적용 (과열, RSI 이탈 등)

#### 2.12.3 API 엔드포인트
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/ranking/7factor/{ticker}` | GET | 개별 종목 7Factor |
| `/api/v1/ranking/7factor/batch` | POST | 배치 조회 |

---

### 2.13 TypeScript 바인딩 자동 생성 (ts-rs) ⭐ v0.6.0

#### 2.13.1 개요
**목적**: Rust 타입 → TypeScript 타입 자동 변환으로 API 타입 안전성 확보

**적용 대상**:
- API 요청/응답 DTO
- Domain 모델 (Signal, Order, Position 등)
- 전략 스키마 타입

**장점**:
- 프론트엔드-백엔드 타입 불일치 방지
- 자동 생성으로 수동 동기화 불필요
- IDE 자동완성 지원

#### 2.13.2 사용 방법
```rust
// Rust에서 TS 어노테이션
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StrategyResponse {
    pub id: i32,
    pub name: String,
    pub running: bool,
}
```

**생성 파일**: `frontend/src/types/generated/`

**빌드 명령**:
```bash
cargo test --features ts-binding
# 또는
cargo build --features generate-ts
```

---

### 2.14 호가 단위 관리 (Tick Size)

#### 2.11.1 거래소별 틱 사이즈
| 거래소 | 규칙 | 예시 |
|--------|------|------|
| **KRX** | 가격대별 7단계 | 50,000원 → 100원 틱 |
| **NYSE/NASDAQ** | 고정 $0.01 | 페니 틱 |
| **TSE (일본)** | 가격대별 변동 | ¥3,000 이하 1円 |
| **HKEX** | 가격대별 변동 | HK$0.25~5,000 |

#### 2.11.2 KRX 호가 단위 (7단계)
| 가격대 | 호가 단위 |
|--------|----------|
| 2,000원 미만 | 1원 |
| 2,000원 ~ 5,000원 미만 | 5원 |
| 5,000원 ~ 20,000원 미만 | 10원 |
| 20,000원 ~ 50,000원 미만 | 50원 |
| 50,000원 ~ 200,000원 미만 | 100원 |
| 200,000원 ~ 500,000원 미만 | 500원 |
| 500,000원 이상 | 1,000원 |

#### 2.11.3 활용
- 주문 가격 유효성 검증
- 목표가/손절가 자동 반올림
- 슬리피지 계산

---

### 2.15 분석 데이터 API ⭐ v0.6.4

> **목적**: 프론트엔드 시각화 컴포넌트에 필요한 백엔드 데이터 API 제공

#### 2.15.1 Volume Profile (매물대 분석)

**목적**: 가격대별 거래량 분포를 계산하여 지지/저항 구간 파악

**계산 방식**:
- 기간 내 가격 범위를 N개 레벨로 분할 (기본 20레벨)
- 각 레벨에 해당하는 거래량 집계
- POC (Point of Control): 최대 거래량 가격대
- Value Area (70% 거래량 구간): VAH, VAL 계산

**데이터 구조**:
```rust
pub struct VolumeProfile {
    pub price_levels: Vec<PriceLevel>,
    pub poc: Decimal,              // Point of Control
    pub value_area_high: Decimal,  // 상단 70% 경계
    pub value_area_low: Decimal,   // 하단 70% 경계
}

pub struct PriceLevel {
    pub price: Decimal,
    pub volume: u64,
    pub buy_volume: u64,
    pub sell_volume: u64,
}
```

**API 엔드포인트**:
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/symbols/{ticker}/volume-profile` | GET | 매물대 분석 |

**쿼리 파라미터**:
- `period`: 분석 기간 일수 (기본 60)
- `levels`: 가격 레벨 수 (기본 20)

#### 2.15.2 Correlation Matrix (상관관계 행렬)

**목적**: 종목 간 가격 움직임 상관관계를 계산하여 포트폴리오 분산 분석

**계산 방식**:
- N일 종가 데이터 기준 Pearson 상관계수 계산
- N×N 대칭 행렬 생성
- 범위: -1.0 (역상관) ~ +1.0 (정상관)

**데이터 구조**:
```rust
pub struct CorrelationMatrix {
    pub symbols: Vec<String>,
    pub matrix: Vec<Vec<f64>>,  // N×N 상관계수 행렬
    pub period_days: u32,
}
```

**API 엔드포인트**:
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/analytics/correlation` | GET | 상관관계 행렬 |

**쿼리 파라미터**:
- `symbols`: 종목 목록 (쉼표 구분)
- `period`: 분석 기간 일수 (기본 60)

#### 2.15.3 Score History (점수 히스토리)

**목적**: 종목별 Global Score 및 RouteState 변화 추적

**저장 항목**:
- 일자별 Global Score (0~100)
- RouteState 상태
- 순위 (전체 중 등수)
- 개별 팩터 점수 (7Factor)

**데이터 모델**:
```sql
CREATE TABLE score_history (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(20) NOT NULL,
    score_date DATE NOT NULL,
    global_score DECIMAL(5,2),
    route_state VARCHAR(20),
    rank INTEGER,
    component_scores JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(symbol, score_date)
);
```

**API 엔드포인트**:
| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/api/v1/symbols/{ticker}/score-history` | GET | 점수 히스토리 |

**쿼리 파라미터**:
- `days`: 조회 기간 (기본 90)

---

### 2.16 주봉 기반 지표 ⭐ v0.6.4

#### 2.16.1 Weekly MA20 (주봉 20선)

**목적**: 중장기 추세 판단을 위한 주봉 이동평균

**계산 방식**:
1. 일봉 데이터 → 주봉 리샘플링
   - Open: 주 첫 거래일 시가
   - High: 주간 최고가
   - Low: 주간 최저가
   - Close: 주 마지막 거래일 종가
   - Volume: 주간 거래량 합계
2. 주봉 MA20 계산 (20주 단순이동평균)
3. 일봉에 해당 주의 MA20 값 매핑

**활용**:
- 중장기 추세 판단 (주봉 MA20 위/아래)
- 눌림목 매수 시점 판단
- 자산배분 전략 필터

**데이터 구조**:
```rust
pub struct WeeklyIndicator {
    pub date: NaiveDate,
    pub weekly_ma20: Option<Decimal>,
    pub weekly_close: Decimal,
    pub is_above_ma20: bool,
}
```

**API 통합**:
- ScreeningResult에 `weekly_ma20`, `is_above_weekly_ma20` 필드 추가
- 스크리닝 필터 조건으로 활용 가능

---

### 2.17 생존일 추적 (Survival Days) ⭐ v0.6.4

**목적**: 연속 상위권 유지 일수를 추적하여 지속 강세 종목 발굴

**계산 방식**:
- 매일 Global Score 기준 상위 N% 또는 상위 N위 종목 확인
- 연속으로 상위권에 포함된 일수 카운트
- 한 번이라도 탈락하면 카운트 리셋

**데이터 구조**:
```rust
pub struct SurvivalStats {
    pub ticker: String,
    pub consecutive_days: u32,      // 연속 상위권 일수
    pub longest_streak: u32,        // 최장 연속 기록
    pub first_entry_date: NaiveDate,
    pub streak_level: StreakLevel,  // Cold/Warm/Hot/Fire
}

pub enum StreakLevel {
    Cold,   // 0-2일
    Warm,   // 3-5일
    Hot,    // 6-9일
    Fire,   // 10일+
}
```

**활용**:
- 스크리닝 결과에 Survival Badge 표시
- 지속 강세 종목 우선 노출
- 텔레그램 알림 (10일+ 연속 시)

**API 통합**:
- ScreeningResult에 `survival_days`, `streak_level` 필드 추가

---

### 2.18 동적 라우트 태깅 (Dynamic Route Tagging) ⭐ v0.6.4

> **보완**: 2.10.2 RouteState 판정 기준에 동적 임계값 적용

**목적**: 고정 임계값 대신 시장 분포 기반 퍼센타일 임계값으로 RouteState 판정

**기존 문제**:
- 고정 임계값 (예: RSI > 70)은 시장 상황에 따라 적합하지 않음
- 강세장에서는 대부분 OVERHEAT, 약세장에서는 대부분 NEUTRAL

**동적 임계값 계산**:
```rust
pub struct DynamicThresholds {
    pub r5_q75: f64,      // 5일 수익률 상위 25% 경계
    pub slope_q60: f64,   // MACD 기울기 상위 40% 경계
    pub ebs_q60: f64,     // EBS 점수 상위 40% 경계
    pub now_gap_q25: f64, // 진입 괴리 하위 25% 경계
}

/// 매일 전체 종목 데이터로 임계값 재계산
pub fn compute_dynamic_thresholds(data: &[SymbolData]) -> DynamicThresholds;
```

**RouteState 판정 (동적)**:
- **ATTACK**: r5 ≥ q75 AND slope ≥ q60 AND ebs ≥ q60 AND now_gap ≤ q25
- **ARMED**: TTM Squeeze 활성 OR (r5 ≥ q60 AND slope > 0)
- **OVERHEAT**: r5 > q90 (상위 10%)
- **WAIT/NEUTRAL**: 기존 로직 유지

**장점**:
- 시장 상황에 적응하는 상대적 평가
- 일정 비율의 종목만 ATTACK/ARMED로 분류
- 백테스트 결과 일관성 향상

---

## 7. 핵심 워크플로우

### 7.1 전략 개발 워크플로우

```
[1] 전략 등록 (Strategies.tsx)
    - 기본 전략 선택
    - 파라미터 커스터마이징
    - 리스크 설정
         ↓
[2] 백테스트 (Backtest.tsx)
    - 과거 데이터로 전략 검증
    - 성과 지표 분석
    - (필요시 파라미터 조정 → 1번 반복)
         ↓
[3] 시뮬레이션 (Simulation.tsx)
    - 실시간 데이터로 모의 거래
    - 실제 시장 환경 검증
         ↓
[4] 실전 운용 (Dashboard)
    - 검증된 전략 활성화
    - 실제 거래 실행
    - 포트폴리오 모니터링
```

### 7.2 데이터 흐름

```
Yahoo Finance / Binance / KIS
         ↓
    [데이터 수집]
         ↓
    TimescaleDB (OHLCV 저장)
         ↓
    [전략 엔진] ← 실시간 시세 (WebSocket)
         ↓
    [주문 실행] → 거래소 API / SimulatedExecutor
         ↓
    [알림] → Telegram / Discord / Slack / Email / SMS
```

#### 7.2.1 WebSocket 실시간 시세 흐름 ⭐ v0.8.0

```
[KIS WebSocket KR/US]
        ↓ (Bridge Task - mpsc channel)
[UnifiedMarketStream] ← 동적 구독 (command channel)
        ↓
[MarketDataAggregator] → broadcast
        ↓
[SubscriptionManager] → Frontend WebSocket 세션
```

- **Singleton 패턴**: credential_id별 단일 스트림 (여러 전략이 공유)
- **동적 구독**: 전략 추가/삭제 시 런타임에 심볼 구독/해제
- **참조 카운트**: 구독 심볼의 사용 추적, 마지막 사용자 해제 시 구독 종료
- **Lazy 초기화**: 전략 시작 시 스트림 자동 생성 (DB 자격증명 기반)

---

### 2.19 Dashboard 실시간 지표 ⭐ v0.7.3

#### 2.19.1 헤더 시장 지표

**목적**: Dashboard 상단에 실시간 시장 상황을 한눈에 파악할 수 있는 지표 표시

**표시 항목**:
| 지표 | 데이터 소스 | 설명 |
|------|------------|------|
| **KOSPI** | Yahoo Finance (^KS11) | 한국 대표 지수 |
| **KOSDAQ** | Yahoo Finance (^KQ11) | 코스닥 지수 |
| **USD/KRW** | Yahoo Finance (KRW=X) | 원/달러 환율 |
| **VIX** | Yahoo Finance (^VIX) | 변동성 지수 (공포 지수) |

**시각화**:
- 현재가 + 전일 대비 변동률 (%) 표시
- 상승: 초록색 ▲, 하락: 빨간색 ▼
- 자동 갱신 (1분 주기)

**API 엔드포인트**:
```
GET /api/v1/market/macro-indicators
```

**응답**:
```json
{
  "kospi": { "price": 2650.32, "change_pct": 0.45 },
  "kosdaq": { "price": 850.21, "change_pct": -0.12 },
  "usd_krw": { "price": 1345.50, "change_pct": 0.08 },
  "vix": { "price": 15.32, "change_pct": -2.31 }
}
```

#### 2.19.2 알림 벨

**목적**: 읽지 않은 알림 표시 및 알림 히스토리 접근

**기능**:
- 읽지 않은 알림 개수 뱃지 표시
- 클릭 시 최근 알림 목록 드롭다운
- 알림 유형별 아이콘 (체결, 신호, 시스템)
- 읽음/안읽음 상태 관리

**API 엔드포인트**:
```
GET /api/v1/alerts/unread-count
GET /api/v1/alerts/recent?limit=10
PUT /api/v1/alerts/{id}/read
```

---

### 2.20 Migration 관리 도구 ⭐ v0.7.3

#### 2.20.1 개요

**목적**: 18개의 마이그레이션 파일을 7개로 통합하고, 안전한 마이그레이션 검증/적용 도구 제공

**현재 문제점**:
| 문제 | 심각도 | 설명 |
|------|--------|------|
| 09→10 순환 | 🔴 | symbols 삭제 후 복원 패턴 |
| 뷰 중복 정의 | 🟡 | v_symbol_with_fundamental 3곳 정의 |
| DROP CASCADE | 🟡 | 6개 파일에서 사용 |
| IF NOT EXISTS 누락 | 🟡 | 멱등성 미보장 |

#### 2.20.2 CLI 명령어

```bash
# 현재 마이그레이션 검증
trader migrate verify --verbose

# 통합 계획 생성 (dry-run)
trader migrate consolidate --dry-run

# 의존성 시각화 (mermaid)
trader migrate graph --output deps.md

# 마이그레이션 적용
trader migrate apply --db-url "postgres://..." --dir migrations_v2
```

#### 2.20.3 통합 마이그레이션 구조 (migrations_v2/)

| # | 파일 | 내용 |
|---|------|------|
| 01 | core_foundation.sql | Extensions, ENUM, symbols, credentials |
| 02 | data_management.sql | symbol_info, ohlcv, v_symbol_with_fundamental |
| 03 | trading_analytics.sql | trade_executions, position_snapshots, 분석 뷰 |
| 04 | strategy_signals.sql | signal_marker, alert_rule, alert_history |
| 05 | evaluation_ranking.sql | global_score, reality_check, score_history |
| 06 | user_settings.sql | watchlist, preset, notification, checkpoint |
| 07 | performance_optimization.sql | 인덱스, MV, Hypertable 정책 |

**예상 효과**:
- 18개 파일 → 7개 파일 (61% 감소)
- 5,204줄 → ~3,300줄 (36% 감소)
- 중복 정의 제거
- 멱등성 보장

---

## 8. 참고 문서

| 문서 | 위치 | 용도 |
|------|------|------|
| CLAUDE.md | 프로젝트 루트 | 프로젝트 구조, API 검증 가이드, 에이전트 지침 |
| todo.md | docs/ | 작업 관리, 진행 상황 추적 |

---

*버전 이력: v1.0 → v2.0 → v2.1 → v3.0 → v4.0 → v4.1 → v5.0 → v5.1 → v5.2 → v6.0 → v6.1 → v7.0*

**v7.0 변경사항 (2026-02-06):**
- 다중 채널 알림 시스템 추가 - Discord, Slack, Email, SMS (2.4.4)
- 저장 전 연결 테스트 기능 추가 - /test/new 엔드포인트 (2.4.4)
- Dashboard 실시간 지표 추가 - KOSPI, KOSDAQ, USD/KRW, VIX 헤더 표시 (2.19)
- 알림 벨 기능 추가 - 읽지 않은 알림 표시 (2.19.2)
- Migration 관리 도구 추가 - trader-cli migrate (2.20)
- migrations_v2 통합 계획 - 18개 → 7개 마이그레이션 통합 (2.20.3)

**v6.1 변경사항:**
- 분석 데이터 API 요구사항 추가 (2.15)
  - Volume Profile API (2.15.1)
  - Correlation Matrix API (2.15.2)
  - Score History API (2.15.3)
- 주봉 기반 지표 요구사항 추가 - Weekly MA20 (2.16)
- 생존일 추적 요구사항 추가 - Survival Days (2.17)
- 동적 라우트 태깅 요구사항 추가 - Dynamic Route Tagging (2.18)

**v6.0 변경사항:**
- 데이터 프로바이더 이중화 (KRX API + Yahoo Finance) 요구사항 추가 (2.5.5)
- Standalone Data Collector (trader-collector) 요구사항 추가 (2.5.6)
- SignalMarker (신호 기록) 요구사항 추가 (2.2.5)
- 신호 시각화 (캔들 차트 오버레이) 요구사항 추가 (2.2.6)
- TTM Squeeze 지표 요구사항 추가 (2.7.5)
- 추가 기술적 지표 (HMA, OBV, SuperTrend, CandlePattern) 요구사항 추가 (2.7.6)
- MacroFilter (거시 환경 필터) 요구사항 추가 (2.8.8)
- MarketBreadth (시장 온도 지표) 요구사항 추가 (2.8.9)
- SectorRS (섹터 분석) 요구사항 추가 (2.8.10)
- RealityCheck (추천 검증) 요구사항 추가 (2.9.7)
- 대시보드 위젯 요구사항 추가 (2.9.8)
- MarketRegime (시장 추세 분류) 요구사항 추가 (2.10.4)
- TRIGGER (진입 신호 강도) 요구사항 추가 (2.10.5)
- 관심종목 관리 (Watchlist) 요구사항 추가 (2.11)
- 7Factor 종합 점수 시스템 요구사항 추가 (2.12)
- TypeScript 바인딩 자동 생성 (ts-rs) 요구사항 추가 (2.13)

**v5.2 변경사항:**
- 종목 랭킹 시스템 (Global Score) 요구사항 추가
- 종목 상태 관리 (RouteState) 요구사항 추가
- 호가 단위 관리 (Tick Size) 요구사항 추가
- ML 구조적 피처 (Structural Features) 요구사항 추가

**v5.1 변경사항:**
- 심볼 자동 동기화 기능 추가 (KRX, Binance, Yahoo Finance)
- Fundamental 데이터 백그라운드 수집 기능 추가
- OHLCV 증분 업데이트 기능 추가
