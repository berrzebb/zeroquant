---
name: crawl-api-spec
description: 거래소 API 문서 크롤링 → 구조화된 명세서(markdown) 생성.
disable-model-invocation: true
user-invocable: true
argument-hint: "<서비스명> <문서URL> [출력파일경로]"
allowed-tools: mcp__playwright__*, Write, Read
context: fork
---

# API 문서 크롤링 및 스펙 문서 생성

## 입력

`$ARGUMENTS` 형식: `<서비스명> <문서URL> [출력파일경로]`

예시:
- `upbit https://docs.upbit.com/kr/reference docs/exchange/upbit_openapi_spec.md`
- `bithumb https://apidocs.bithumb.com docs/exchange/bithumb_openapi_spec.md`
- `kis https://apiportal.koreainvestment.com docs/exchange/kis_openapi_spec.md`

---

## 실행 절차

### Phase 1: 사이트 구조 파악

1. Playwright MCP `browser_navigate`로 문서 URL 접속
2. `browser_snapshot`으로 페이지 구조 확인 (사이드바 네비게이션, API 카테고리)
3. 약관 동의 팝업이 있으면 자동 클릭하여 통과
4. API 카테고리 목록과 각 API 페이지 URL을 수집

### Phase 2: 배치 크롤링

5. `browser_run_code`를 사용하여 배치 단위(4~7개 페이지)로 크롤링:
   ```javascript
   async (page) => {
     const pages = ['url1', 'url2', ...];
     const results = {};
     for (const p of pages) {
       await page.goto(p, { waitUntil: 'domcontentloaded', timeout: 15000 });
       await page.waitForTimeout(1500);
       const content = await page.evaluate(() => {
         const article = document.querySelector('article') || document.querySelector('main') || document.body;
         return article.innerText.substring(0, 5000);
       });
       results[p] = content;
     }
     return JSON.stringify(results);
   }
   ```
6. 각 배치에서 추출할 정보:
   - API 이름, Method (GET/POST/DELETE), URL 경로
   - 입력 파라미터 (이름, 타입, 필수여부, 설명)
   - 출력 필드 (이름, 타입, 설명)
   - Rate Limit, 인증 방식
7. 크롤링 대상 우선순위:
   - **필수**: 인증, 시세조회(현재가/호가/체결/캔들), 주문(생성/취소/조회), 잔고, WebSocket
   - **선택**: 입출금, 서비스 상태, 공지 등

### Phase 3: 동적 로딩 대응

8. JavaScript SPA 사이트에서 내용이 안 보이면:
   - `browser_wait_for` 사용하여 특정 텍스트 로딩 대기
   - `browser_evaluate`로 DOM 직접 탐색
   - 그래도 불가능하면 `[크롤링 불가 - 수동 확인 필요]`로 표시
9. 테이블이 HTML table이 아닌 커스텀 컴포넌트인 경우:
   - `innerText` 기반 추출 후 파싱
   - 탭/줄바꿈 패턴으로 필드 분리

### Phase 4: 문서 생성

10. 수집된 데이터를 아래 표준 형식으로 정리:

출력 템플릿은 [output-template.md](output-template.md)를 참조합니다.

---

## 에러 대응

| 상황 | 대응 |
|------|------|
| 사이트 접속 불가 | 3회 재시도 후 실패 보고 |
| 약관/CAPTCHA 차단 | 사용자에게 수동 통과 요청 |
| 내용 누락 | `[미확인 - 수동 확인 필요]`로 표시 |
| Rate Limit | 배치 간 3초 대기 |
