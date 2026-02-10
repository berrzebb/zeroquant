# 보안 규칙

## 1. API 키 관리

> 환경변수 대신 웹 UI를 통한 암호화 저장

- 거래소 API 키 → Settings 페이지에서 설정
- 텔레그램 봇 토큰 → Settings 페이지에서 설정
- 모든 민감 정보 → AES-256-GCM 암호화 저장

## 2. 민감 정보 로깅 방지

```rust
// ❌ API 키 로깅 금지
tracing::info!("API Key: {}", api_key);

// ✅ 마스킹 처리
tracing::info!("API Key: {}***", &api_key[..4]);

// 또는 secrecy 크레이트 사용
use secrecy::{Secret, ExposeSecret};
let api_key: Secret<String> = Secret::new(key);
```

## 3. 입력 검증

모든 외부 입력에 대해:

- 길이 제한
- 형식 검증
- 범위 검증
- SQL Injection 방지 (prepared statement 사용)
