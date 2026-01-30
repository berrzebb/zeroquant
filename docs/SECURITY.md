# 보안 가이드라인

ZeroQuant 프로젝트의 보안을 유지하기 위한 가이드입니다.

---

## 🔐 보안 키 관리

### 필수 환경 변수

ZeroQuant를 배포하기 전에 반드시 다음 키를 생성하고 설정해야 합니다:

#### 1. JWT Secret Key
JWT 토큰 서명에 사용되는 키입니다.

```bash
# 생성 방법 (최소 32바이트)
openssl rand -hex 32
```

**설정 위치**: `.env` 파일
```env
JWT_SECRET=<생성된_키_입력>
```

#### 2. Encryption Master Key
데이터베이스에 저장되는 API 키를 암호화하는 마스터 키입니다.

```bash
# 생성 방법 (32바이트 Base64 인코딩)
openssl rand -base64 32
```

**설정 위치**: `.env` 파일
```env
ENCRYPTION_KEY=<생성된_키_입력>
```

### ⚠️ 주의사항

1. **절대 기본값 사용 금지**
   - `.env.example`의 예제 값을 그대로 사용하지 마세요
   - 프로덕션 환경에서 기본값 사용 시 Docker Compose가 시작을 거부합니다

2. **키 보관**
   - 생성한 키는 안전한 비밀번호 관리자에 저장하세요
   - 버전 관리 시스템에 절대 커밋하지 마세요 (`.gitignore` 확인)

3. **키 로테이션**
   - 주기적으로 키를 교체하는 것을 권장합니다
   - 키 변경 시 기존 세션/암호화 데이터 영향 고려

---

## 🛡️ 데이터베이스 보안

### API 키 암호화

거래소 API 키와 같은 민감한 자격증명은 AES-256-GCM으로 암호화되어 저장됩니다.

**구현 위치**: `crates/trader-core/src/crypto.rs`

```rust
// 암호화
let encrypted = encrypt_credentials(&credentials, &encryption_key)?;

// 복호화
let decrypted = decrypt_credentials(&encrypted, &encryption_key)?;
```

### 데이터베이스 접근 제어

```env
# 프로덕션 환경에서는 강력한 비밀번호 사용
POSTGRES_USER=trader
POSTGRES_PASSWORD=<강력한_비밀번호>
POSTGRES_DB=trader
```

**권장사항**:
- 최소 16자 이상
- 영문 대소문자 + 숫자 + 특수문자 조합
- 정기적인 비밀번호 변경

---

## 🔒 API 보안

### 인증 체계

ZeroQuant는 JWT(JSON Web Token) 기반 인증을 사용합니다.

#### 인증 흐름
```
1. 클라이언트 → POST /api/v1/auth/login (이메일, 비밀번호)
2. 서버 → JWT 토큰 발급
3. 클라이언트 → 이후 모든 요청에 `Authorization: Bearer <token>` 헤더 포함
4. 서버 → JWT 검증 후 요청 처리
```

#### 비밀번호 해싱

사용자 비밀번호는 Argon2id 알고리즘으로 해싱됩니다.

**구현 위치**: `crates/trader-api/src/auth/password.rs`

```rust
// 비밀번호 해싱
let hash = hash_password(&password)?;

// 비밀번호 검증
let is_valid = verify_password(&password, &hash)?;
```

### CORS 설정

프로덕션 환경에서는 허용된 출처만 API 접근을 허용하세요.

```env
# .env
API_CORS_ORIGINS=https://yourdomain.com,https://app.yourdomain.com
```

**기본값**: `*` (모든 출처 허용) - **프로덕션에서 절대 사용 금지!**

### Rate Limiting

API 남용을 방지하기 위한 속도 제한:

```rust
// crates/trader-api/src/middleware/rate_limit.rs
// 기본: IP당 분당 100 요청
```

**프로덕션 설정**:
```env
RATE_LIMIT_DISABLED=false  # 개발 환경에서만 true
```

---

## 🚨 보안 모범 사례

### 1. 환경 분리

```bash
# 개발 환경
.env.development

# 프로덕션 환경
.env.production
```

각 환경마다 **다른 키**를 사용하세요.

### 2. 컨테이너 보안

#### 최소 권한 원칙
```dockerfile
# Dockerfile
# root가 아닌 사용자로 실행
USER trader
```

#### 비밀 정보 관리
```yaml
# docker-compose.yml (프로덕션)
# 환경변수 대신 Docker Secrets 사용
secrets:
  jwt_secret:
    external: true
  encryption_key:
    external: true
```

### 3. 네트워크 보안

#### HTTPS 사용
프로덕션 환경에서는 반드시 HTTPS를 사용하세요.

```nginx
# Nginx 리버스 프록시 예시
server {
    listen 443 ssl;
    ssl_certificate /etc/ssl/certs/cert.pem;
    ssl_certificate_key /etc/ssl/private/key.pem;
    
    location / {
        proxy_pass http://trader-api:3000;
    }
}
```

#### 방화벽 설정
```bash
# 필요한 포트만 열기
- 443 (HTTPS)
- 3000 (API - 내부 네트워크만)
- 5432 (PostgreSQL - 내부 네트워크만)
- 6379 (Redis - 내부 네트워크만)
```

### 4. 로깅 및 모니터링

#### 민감 정보 로깅 금지
```rust
// ❌ 나쁜 예
tracing::info!("API Key: {}", api_key);

// ✅ 좋은 예
tracing::info!("API Key configured for user {}", user_id);
```

#### 보안 이벤트 알림
```rust
// 의심스러운 활동 감지 시 알림
if failed_login_attempts > 5 {
    notify_security_team("Brute force attempt detected", user_ip).await;
}
```

---

## 🔍 취약점 스캔

### Rust 의존성 감사

```bash
# cargo-audit 설치
cargo install cargo-audit

# 취약점 스캔
cargo audit

# 자동 업데이트 (주의 필요)
cargo audit fix
```

**권장**: CI/CD 파이프라인에 `cargo audit` 통합

### Docker 이미지 스캔

```bash
# Trivy 사용
trivy image trader-api:latest

# 높은 심각도 취약점만
trivy image --severity HIGH,CRITICAL trader-api:latest
```

---

## 📋 보안 체크리스트

배포 전 확인 사항:

- [ ] JWT_SECRET과 ENCRYPTION_KEY를 고유하게 생성했는가?
- [ ] 데이터베이스 비밀번호를 기본값에서 변경했는가?
- [ ] CORS 설정이 특정 도메인으로 제한되어 있는가?
- [ ] Rate Limiting이 활성화되어 있는가?
- [ ] HTTPS가 설정되어 있는가?
- [ ] 로그에 민감 정보가 포함되지 않는가?
- [ ] 방화벽이 불필요한 포트를 차단하는가?
- [ ] 의존성 취약점 스캔을 수행했는가?
- [ ] Docker 이미지 스캔을 수행했는가?
- [ ] 백업 및 복구 절차가 수립되어 있는가?

---

## 🚨 보안 사고 대응

### 1. API 키 유출 시

```bash
# 1. 즉시 거래소에서 API 키 비활성화
# 2. 데이터베이스에서 해당 자격증명 삭제
DELETE FROM exchange_credentials WHERE id = '<compromised_id>';

# 3. 영향받은 사용자에게 알림
# 4. 새로운 API 키로 교체
```

### 2. JWT Secret 유출 시

```bash
# 1. 즉시 새로운 JWT_SECRET 생성
openssl rand -hex 32

# 2. .env 파일 업데이트 및 서비스 재시작
docker-compose restart trader-api

# 3. 모든 사용자 세션 무효화 (자동)
# 4. 사용자에게 재로그인 안내
```

### 3. Encryption Key 유출 시

⚠️ **매우 심각** - 모든 암호화된 자격증명이 노출됨

```bash
# 1. 새로운 마스터 키 생성
openssl rand -base64 32

# 2. 기존 자격증명 복호화 후 새 키로 재암호화
# (스크립트 필요 - 문의 필요 시 관리자 연락)

# 3. 모든 거래소 API 키 교체 권장
```

---

## 📞 보안 문의

보안 취약점을 발견하셨나요?

**연락처**: security@yourdomain.com (또는 GitHub Issues - private)

**책임 있는 공개 정책**:
1. 취약점 발견 시 즉시 보고
2. 공개적으로 노출하지 않기
3. 패치 적용 후 공개 논의

---

## 📚 참고 자료

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Best Practices](https://anssi-fr.github.io/rust-guide/)
- [Docker Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Docker_Security_Cheat_Sheet.html)
- [JWT Best Practices](https://datatracker.ietf.org/doc/html/rfc8725)
