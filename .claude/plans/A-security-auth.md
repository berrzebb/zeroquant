# Plan: [A] 보안 & 인증 기반

> 🔴 라이브 트레이딩 전 필수. 모든 그룹과 독립, 즉시 착수 가능.
> 병렬: B, G와 동시 진행 가능

## 선행 조건
- 없음 (독립 착수 가능)

## 예상 규모
Small

---

## A-1: API 인증 체계 구축

- [ ] 전체 API 라우트에 JWT `AuthUser` extractor 적용 (`trader-api/src/routes/`)
- [ ] WebSocket 핸드셰이크 시 토큰 검증 미들웨어 추가 (`trader-api/src/websocket/`)
- [ ] Axum `RequestBodyLimit` 미들웨어 적용 (DoS 방지)
- [ ] `config/default.toml` 기본 시크릿 제거 → 환경변수 필수화

## 관련 파일
- `crates/trader-api/src/routes/`
- `crates/trader-api/src/websocket/`
- `config/default.toml`
