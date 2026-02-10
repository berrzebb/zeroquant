# KRX OPEN API 명세서

> 문서 생성일: 2026-02-04
> 데이터 제공 대상기간: 2010년 이후 데이터

## 개요

KRX OPEN API는 한국거래소(KRX)에서 제공하는 공식 데이터 API 서비스입니다.

### API 호출 정보

| 항목 | 값 |
|------|-----|
| Base URL | `https://data-dbg.krx.co.kr/svc/apis/` |
| 인증 방식 | HTTP Header에 `AUTH_KEY` 필드로 전달 |
| 응답 형식 | JSON, XML |
| 요청 방식 | GET |

### HTTP Request 예시

```http
GET /svc/apis/idx/krx_dd_trd?basDd=20200414 HTTP/1.1
Host: openapi.krx.co.kr
AUTH_KEY: [YOUR_AUTH_KEY]
```

### 응답 형식

```json
{
  "OutBlock_1": [
    { /* 데이터 레코드 */ },
    { /* 데이터 레코드 */ }
  ]
}
```

---

## 1. 지수 (5개 API)

### 1.1 KRX 시리즈 일별시세정보

| 항목 | 값 |
|------|-----|
| API ID | `krx_dd_trd` |
| URL | `/svc/apis/idx/krx_dd_trd` |
| 설명 | KRX 시리즈 지수의 시세정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 계열구분 | IDX_CLSS | string |
| 3 | 지수명 | IDX_NM | string |
| 4 | 종가 | CLSPRC_IDX | string |
| 5 | 대비 | CMPPREVDD_IDX | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 시가 | OPNPRC_IDX | string |
| 8 | 고가 | HGPRC_IDX | string |
| 9 | 저가 | LWPRC_IDX | string |
| 10 | 거래량 | ACC_TRDVOL | string |
| 11 | 거래대금 | ACC_TRDVAL | string |
| 12 | 상장시가총액 | MKTCAP | string |

---

### 1.2 KOSPI 시리즈 일별시세정보

| 항목 | 값 |
|------|-----|
| API ID | `kospi_dd_trd` |
| URL | `/svc/apis/idx/kospi_dd_trd` |
| 설명 | KOSPI 시리즈 지수의 시세정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 계열구분 | IDX_CLSS | string |
| 3 | 지수명 | IDX_NM | string |
| 4 | 종가 | CLSPRC_IDX | string |
| 5 | 대비 | CMPPREVDD_IDX | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 시가 | OPNPRC_IDX | string |
| 8 | 고가 | HGPRC_IDX | string |
| 9 | 저가 | LWPRC_IDX | string |
| 10 | 거래량 | ACC_TRDVOL | string |
| 11 | 거래대금 | ACC_TRDVAL | string |
| 12 | 상장시가총액 | MKTCAP | string |

---

### 1.3 KOSDAQ 시리즈 일별시세정보

| 항목 | 값 |
|------|-----|
| API ID | `kosdaq_dd_trd` |
| URL | `/svc/apis/idx/kosdaq_dd_trd` |
| 설명 | KOSDAQ 시리즈 지수의 시세정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 계열구분 | IDX_CLSS | string |
| 3 | 지수명 | IDX_NM | string |
| 4 | 종가 | CLSPRC_IDX | string |
| 5 | 대비 | CMPPREVDD_IDX | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 시가 | OPNPRC_IDX | string |
| 8 | 고가 | HGPRC_IDX | string |
| 9 | 저가 | LWPRC_IDX | string |
| 10 | 거래량 | ACC_TRDVOL | string |
| 11 | 거래대금 | ACC_TRDVAL | string |
| 12 | 상장시가총액 | MKTCAP | string |

---

### 1.4 채권지수 시세정보

| 항목 | 값 |
|------|-----|
| API ID | `bon_dd_trd` |
| URL | `/svc/apis/idx/bon_dd_trd` |
| 설명 | 채권지수의 시세정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 지수명 | BND_IDX_GRP_NM | string |
| 3 | 총수익지수_종가 | TOT_EARNG_IDX | string |
| 4 | 총수익지수_대비 | TOT_EARNG_IDX_CMPPREVDD | string |
| 5 | 순가격지수_종가 | NETPRC_IDX | string |
| 6 | 순가격지수_대비 | NETPRC_IDX_CMPPREVDD | string |
| 7 | 제로재투자지수_종가 | ZERO_REINVST_IDX | string |
| 8 | 제로재투자지수_대비 | ZERO_REINVST_IDX_CMPPREVDD | string |
| 9 | 콜재투자지수_종가 | CALL_REINVST_IDX | string |
| 10 | 콜재투자지수_대비 | CALL_REINVST_IDX_CMPPREVDD | string |
| 11 | 시장가격지수_종가 | MKT_PRC_IDX | string |
| 12 | 시장가격지수_대비 | MKT_PRC_IDX_CMPPREVDD | string |
| 13 | 듀레이션 | AVG_DURATION | string |
| 14 | 컨벡시티 | AVG_CONVEXITY_PRC | string |
| 15 | YTM | BND_IDX_AVG_YD | string |

---

### 1.5 파생상품지수 시세정보

| 항목 | 값 |
|------|-----|
| API ID | `drvprod_dd_trd` |
| URL | `/svc/apis/idx/drvprod_dd_trd` |
| 설명 | 파생상품지수의 시세정보를 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 계열구분 | IDX_CLSS | string |
| 3 | 지수명 | IDX_NM | string |
| 4 | 종가 | CLSPRC_IDX | string |
| 5 | 대비 | CMPPREVDD_IDX | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 시가 | OPNPRC_IDX | string |
| 8 | 고가 | HGPRC_IDX | string |
| 9 | 저가 | LWPRC_IDX | string |

---

## 2. 주식 (8개 API)

### 2.1 유가증권 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `stk_bydd_trd` |
| URL | `/svc/apis/sto/stk_bydd_trd` |
| 설명 | 유가증권시장에 상장되어 있는 주권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목코드 | ISU_CD | string |
| 3 | 종목명 | ISU_NM | string |
| 4 | 시장구분 | MKT_NM | string |
| 5 | 소속부 | SECT_TP_NM | string |
| 6 | 종가 | TDD_CLSPRC | string |
| 7 | 대비 | CMPPREVDD_PRC | string |
| 8 | 등락률 | FLUC_RT | string |
| 9 | 시가 | TDD_OPNPRC | string |
| 10 | 고가 | TDD_HGPRC | string |
| 11 | 저가 | TDD_LWPRC | string |
| 12 | 거래량 | ACC_TRDVOL | string |
| 13 | 거래대금 | ACC_TRDVAL | string |
| 14 | 시가총액 | MKTCAP | string |
| 15 | 상장주식수 | LIST_SHRS | string |

---

### 2.2 코스닥 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `ksq_bydd_trd` |
| URL | `/svc/apis/sto/ksq_bydd_trd` |
| 설명 | 코스닥시장에 상장되어 있는 주권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 유가증권 일별매매정보와 동일 (15개 필드)

---

### 2.3 코넥스 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `knx_bydd_trd` |
| URL | `/svc/apis/sto/knx_bydd_trd` |
| 설명 | 코넥스시장에 상장되어 있는 주권의 매매정보 제공 ('13년07월01일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 유가증권 일별매매정보와 동일 (15개 필드)

---

### 2.4 신주인수권증권 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `sw_bydd_trd` |
| URL | `/svc/apis/sto/sw_bydd_trd` |
| 설명 | 유가증권/코스닥시장에 상장되어 있는 신주인수권증권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 시장구분 | MKT_NM | string |
| 3 | 종목코드 | ISU_CD | string |
| 4 | 종목명 | ISU_NM | string |
| 5 | 종가 | TDD_CLSPRC | string |
| 6 | 대비 | CMPPREVDD_PRC | string |
| 7 | 등락률 | FLUC_RT | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 거래량 | ACC_TRDVOL | string |
| 12 | 거래대금 | ACC_TRDVAL | string |
| 13 | 시가총액 | MKTCAP | string |
| 14 | 상장증권수 | LIST_SHRS | string |
| 15 | 행사가격 | EXER_PRC | string |
| 16 | 존속기간_시작일 | EXST_STRT_DD | string |
| 17 | 존속기간_종료일 | EXST_END_DD | string |
| 18 | 목적주권_종목코드 | TARSTK_ISU_SRT_CD | string |
| 19 | 목적주권_종목명 | TARSTK_ISU_NM | string |
| 20 | 목적주권_종가 | TARSTK_ISU_PRSNT_PRC | string |

---

### 2.5 신주인수권증서 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `sr_bydd_trd` |
| URL | `/svc/apis/sto/sr_bydd_trd` |
| 설명 | 유가증권/코스닥시장에 상장되어 있는 신주인수권증서의 매매정보 제공 ('10년02월12일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 시장구분 | MKT_NM | string |
| 3 | 종목코드 | ISU_CD | string |
| 4 | 종목명 | ISU_NM | string |
| 5 | 종가 | TDD_CLSPRC | string |
| 6 | 대비 | CMPPREVDD_PRC | string |
| 7 | 등락률 | FLUC_RT | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 거래량 | ACC_TRDVOL | string |
| 12 | 거래대금 | ACC_TRDVAL | string |
| 13 | 시가총액 | MKTCAP | string |
| 14 | 상장증서수 | LIST_SHRS | string |
| 15 | 신주발행가 | ISU_PRC | string |
| 16 | 상장폐지일 | DELIST_DD | string |
| 17 | 목적주권_종목코드 | TARSTK_ISU_SRT_CD | string |
| 18 | 목적주권_종목명 | TARSTK_ISU_NM | string |
| 19 | 목적주권_종가 | TARSTK_ISU_PRSNT_PRC | string |

---

### 2.6 유가증권 종목기본정보

| 항목 | 값 |
|------|-----|
| API ID | `stk_isu_base_info` |
| URL | `/svc/apis/sto/stk_isu_base_info` |
| 설명 | 유가증권 종목기본정보 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 표준코드 | ISU_CD | string |
| 2 | 단축코드 | ISU_SRT_CD | string |
| 3 | 한글 종목명 | ISU_NM | string |
| 4 | 한글 종목약명 | ISU_ABBRV | string |
| 5 | 영문 종목명 | ISU_ENG_NM | string |
| 6 | 상장일 | LIST_DD | string |
| 7 | 시장구분 | MKT_TP_NM | string |
| 8 | 증권구분 | SECUGRP_NM | string |
| 9 | 소속부 | SECT_TP_NM | string |
| 10 | 주식종류 | KIND_STKCERT_TP_NM | string |
| 11 | 액면가 | PARVAL | string |
| 12 | 상장주식수 | LIST_SHRS | string |

---

### 2.7 코스닥 종목기본정보

| 항목 | 값 |
|------|-----|
| API ID | `ksq_isu_base_info` |
| URL | `/svc/apis/sto/ksq_isu_base_info` |
| 설명 | 코스닥 종목기본정보 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 유가증권 종목기본정보와 동일 (12개 필드)

---

### 2.8 코넥스 종목기본정보

| 항목 | 값 |
|------|-----|
| API ID | `knx_isu_base_info` |
| URL | `/svc/apis/sto/knx_isu_base_info` |
| 설명 | 코넥스 종목기본정보 ('13년07월01일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 유가증권 종목기본정보와 동일 (12개 필드)

---

## 3. 증권상품 (3개 API)

### 3.1 ETF 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `etf_bydd_trd` |
| URL | `/svc/apis/etp/etf_bydd_trd` |
| 설명 | ETF(상장지수펀드)의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목코드 | ISU_CD | string |
| 3 | 종목명 | ISU_NM | string |
| 4 | 종가 | TDD_CLSPRC | string |
| 5 | 대비 | CMPPREVDD_PRC | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 순자산가치(NAV) | NAV | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 거래량 | ACC_TRDVOL | string |
| 12 | 거래대금 | ACC_TRDVAL | string |
| 13 | 시가총액 | MKTCAP | string |
| 14 | 순자산총액 | INVSTASST_NETASST_TOTAMT | string |
| 15 | 상장좌수 | LIST_SHRS | string |
| 16 | 기초지수_지수명 | IDX_IND_NM | string |
| 17 | 기초지수_종가 | OBJ_STKPRC_IDX | string |
| 18 | 기초지수_대비 | CMPPREVDD_IDX | string |
| 19 | 기초지수_등락률 | FLUC_RT_IDX | string |

---

### 3.2 ETN 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `etn_bydd_trd` |
| URL | `/svc/apis/etp/etn_bydd_trd` |
| 설명 | ETN(상장지수증권)의 매매정보 제공 ('14년11월17일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목코드 | ISU_CD | string |
| 3 | 종목명 | ISU_NM | string |
| 4 | 종가 | TDD_CLSPRC | string |
| 5 | 대비 | CMPPREVDD_PRC | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 지표가치(IV) | PER1SECU_INDIC_VAL | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 거래량 | ACC_TRDVOL | string |
| 12 | 거래대금 | ACC_TRDVAL | string |
| 13 | 시가총액 | MKTCAP | string |
| 14 | 지표가치총액 | INDIC_VAL_AMT | string |
| 15 | 상장증권수 | LIST_SHRS | string |
| 16 | 기초지수_지수명 | IDX_IND_NM | string |
| 17 | 기초지수_종가 | OBJ_STKPRC_IDX | string |
| 18 | 기초지수_대비 | CMPPREVDD_IDX | string |
| 19 | 기초지수_등락률 | FLUC_RT_IDX | string |

---

### 3.3 ELW 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `elw_bydd_trd` |
| URL | `/svc/apis/etp/elw_bydd_trd` |
| 설명 | ELW(주식위런트증권)의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목코드 | ISU_CD | string |
| 3 | 종목명 | ISU_NM | string |
| 4 | 종가 | TDD_CLSPRC | string |
| 5 | 대비 | CMPPREVDD_PRC | string |
| 6 | 시가 | TDD_OPNPRC | string |
| 7 | 고가 | TDD_HGPRC | string |
| 8 | 저가 | TDD_LWPRC | string |
| 9 | 거래량 | ACC_TRDVOL | string |
| 10 | 거래대금 | ACC_TRDVAL | string |
| 11 | 시가총액 | MKTCAP | string |
| 12 | 상장증권수 | LIST_SHRS | string |
| 13 | 기초자산_자산명 | ULY_NM | string |
| 14 | 기초자산_종가 | ULY_PRC | string |
| 15 | 기초자산_대비 | CMPPREVDD_PRC_ULY | string |
| 16 | 기초자산_등락률 | FLUC_RT_ULY | string |

---

## 4. 채권 (3개 API)

### 4.1 국채전문유통시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `kts_bydd_trd` |
| URL | `/svc/apis/bnd/kts_bydd_trd` |
| 설명 | 국채전문유통시장에 상장되어있는 채권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 시장구분 | MKT_NM | string |
| 3 | 종목코드 | ISU_CD | string |
| 4 | 종목명 | ISU_NM | string |
| 5 | 만기년수 | BND_EXP_TP_NM | string |
| 6 | 종목구분 | GOVBND_ISU_TP_NM | string |
| 7 | 종가_가격 | CLSPRC | string |
| 8 | 종가_대비 | CMPPREVDD_PRC | string |
| 9 | 종가_수익률 | CLSPRC_YD | string |
| 10 | 시가_가격 | OPNPRC | string |
| 11 | 시가_수익률 | OPNPRC_YD | string |
| 12 | 고가_가격 | HGPRC | string |
| 13 | 고가_수익률 | HGPRC_YD | string |
| 14 | 저가_가격 | LWPRC | string |
| 15 | 저가_수익률 | LWPRC_YD | string |
| 16 | 거래량 | ACC_TRDVOL | string |
| 17 | 거래대금 | ACC_TRDVAL | string |

---

### 4.2 일반채권시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `bnd_bydd_trd` |
| URL | `/svc/apis/bnd/bnd_bydd_trd` |
| 설명 | 일반채권시장에 상장되어있는 채권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 시장구분 | MKT_NM | string |
| 3 | 종목코드 | ISU_CD | string |
| 4 | 종목명 | ISU_NM | string |
| 5 | 종가_가격 | CLSPRC | string |
| 6 | 종가_대비 | CMPPREVDD_PRC | string |
| 7 | 종가_수익률 | CLSPRC_YD | string |
| 8 | 시가_가격 | OPNPRC | string |
| 9 | 시가_수익률 | OPNPRC_YD | string |
| 10 | 고가_가격 | HGPRC | string |
| 11 | 고가_수익률 | HGPRC_YD | string |
| 12 | 저가_가격 | LWPRC | string |
| 13 | 저가_수익률 | LWPRC_YD | string |
| 14 | 거래량 | ACC_TRDVOL | string |
| 15 | 거래대금 | ACC_TRDVAL | string |

---

### 4.3 소액채권시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `smb_bydd_trd` |
| URL | `/svc/apis/bnd/smb_bydd_trd` |
| 설명 | 소액채권시장에 상장되어있는 채권의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 일반채권시장 일별매매정보와 동일 (15개 필드)

---

## 5. 파생상품 (6개 API)

### 5.1 선물 일별매매정보 (주식선물外)

| 항목 | 값 |
|------|-----|
| API ID | `fut_bydd_trd` |
| URL | `/svc/apis/drv/fut_bydd_trd` |
| 설명 | 파생상품시장의 선물 중 주식선물을 제외한 선물의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 상품구분 | PROD_NM | string |
| 3 | 시장구분(정규/야간) | MKT_NM | string |
| 4 | 종목코드 | ISU_CD | string |
| 5 | 종목명 | ISU_NM | string |
| 6 | 종가 | TDD_CLSPRC | string |
| 7 | 대비 | CMPPREVDD_PRC | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 현물가 | SPOT_PRC | string |
| 12 | 정산가 | SETL_PRC | string |
| 13 | 거래량 | ACC_TRDVOL | string |
| 14 | 거래대금 | ACC_TRDVAL | string |
| 15 | 미결제약정 | ACC_OPNINT_QTY | string |

---

### 5.2 주식선물(유가) 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `eqsfu_stk_bydd_trd` |
| URL | `/svc/apis/drv/eqsfu_stk_bydd_trd` |
| 설명 | 파생상품시장의 주식선물 중 기초자산이 유가증권시장에 속하는 주식선물의 거래정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 선물 일별매매정보와 동일 (15개 필드)

---

### 5.3 주식선물(코스닥) 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `eqkfu_ksq_bydd_trd` |
| URL | `/svc/apis/drv/eqkfu_ksq_bydd_trd` |
| 설명 | 파생상품시장의 주식선물 중 기초자산이 코스닥시장에 속하는 주식선물의 거래정보 제공 ('15년08월03일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 선물 일별매매정보와 동일 (15개 필드)

---

### 5.4 옵션 일별매매정보 (주식옵션外)

| 항목 | 값 |
|------|-----|
| API ID | `opt_bydd_trd` |
| URL | `/svc/apis/drv/opt_bydd_trd` |
| 설명 | 파생상품시장의 옵션 중 주식옵션을 제외한 옵션의 매매정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 상품구분 | PROD_NM | string |
| 3 | 권리유형(CALL/PUT) | RGHT_TP_NM | string |
| 4 | 종목코드 | ISU_CD | string |
| 5 | 종목명 | ISU_NM | string |
| 6 | 종가 | TDD_CLSPRC | string |
| 7 | 대비 | CMPPREVDD_PRC | string |
| 8 | 시가 | TDD_OPNPRC | string |
| 9 | 고가 | TDD_HGPRC | string |
| 10 | 저가 | TDD_LWPRC | string |
| 11 | 내재변동성 | IMP_VOLT | string |
| 12 | 익일정산가 | NXTDD_BAS_PRC | string |
| 13 | 거래량 | ACC_TRDVOL | string |
| 14 | 거래대금 | ACC_TRDVAL | string |
| 15 | 미결제약정 | ACC_OPNINT_QTY | string |

---

### 5.5 주식옵션(유가) 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `eqsop_bydd_trd` |
| URL | `/svc/apis/drv/eqsop_bydd_trd` |
| 설명 | 파생상품시장의 주식옵션 중 기초자산이 유가증권시장에 속하는 주식옵션의 거래정보 제공 ('10년01월04일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 옵션 일별매매정보와 동일 (15개 필드)

---

### 5.6 주식옵션(코스닥) 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `eqkop_bydd_trd` |
| URL | `/svc/apis/drv/eqkop_bydd_trd` |
| 설명 | 파생상품시장의 주식옵션 중 기초자산이 코스닥시장에 속하는 주식옵션의 거래정보 제공 ('17년06월26일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 옵션 일별매매정보와 동일 (15개 필드)

---

## 6. 일반상품 (3개 API)

### 6.1 석유시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `oil_bydd_trd` |
| URL | `/svc/apis/gen/oil_bydd_trd` |
| 설명 | KRX 석유시장의 매매정보 제공 ('12년03월30일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 유종구분 | OIL_NM | string |
| 3 | 가중평균가격_경쟁 | WT_AVG_PRC | string |
| 4 | 가중평균가격_협의 | WT_DIS_AVG_PRC | string |
| 5 | 거래량 | ACC_TRDVOL | string |
| 6 | 거래대금 | ACC_TRDVAL | string |

---

### 6.2 금시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `gold_bydd_trd` |
| URL | `/svc/apis/gen/gold_bydd_trd` |
| 설명 | KRX 금시장 매매정보 제공 ('14년03월24일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목코드 | ISU_CD | string |
| 3 | 종목명 | ISU_NM | string |
| 4 | 종가 | TDD_CLSPRC | string |
| 5 | 대비 | CMPPREVDD_PRC | string |
| 6 | 등락률 | FLUC_RT | string |
| 7 | 시가 | TDD_OPNPRC | string |
| 8 | 고가 | TDD_HGPRC | string |
| 9 | 저가 | TDD_LWPRC | string |
| 10 | 거래량 | ACC_TRDVOL | string |
| 11 | 거래대금 | ACC_TRDVAL | string |

---

### 6.3 배출권 시장 일별매매정보

| 항목 | 값 |
|------|-----|
| API ID | `ets_bydd_trd` |
| URL | `/svc/apis/gen/ets_bydd_trd` |
| 설명 | KRX 탄소배출권 시장의 매매정보 제공 ('15년01월12일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**: 금시장 일별매매정보와 동일 (11개 필드)

---

## 7. ESG (3개 API)

### 7.1 사회책임투자채권 정보

| 항목 | 값 |
|------|-----|
| API ID | `sri_bond_info` |
| URL | `/svc/apis/esg/sri_bond_info` |
| 설명 | 사회책임투자채권 정보를 제공 ('19년01월01일 데이터부터 제공) |

**입력 파라미터**

| 파라미터 | 설명 | 예시 |
|----------|------|------|
| basDd | 기준일자 | 20200414 |

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 발행기관 | ISUR_NM | string |
| 3 | 표준코드 | ISU_CD | string |
| 4 | 채권종류 | SRI_BND_TP_NM | string |
| 5 | 종목명 | ISU_NM | string |
| 6 | 상장일 | LIST_DD | string |
| 7 | 발행일 | ISU_DD | string |
| 8 | 상환일 | REDMPT_DD | string |
| 9 | 표면이자율 | ISU_RT | string |
| 10 | 발행금액 | ISU_AMT | string |
| 11 | 상장금액 | LIST_AMT | string |
| 12 | 채권유형 | BND_TP_NM | string |

---

### 7.2 ESG 증권상품

| 항목 | 값 |
|------|-----|
| API ID | `esg_etp_info` |
| URL | `/svc/apis/esg/esg_etp_info` |
| 설명 | ESG 증권상품 정보를 제공 ('20년01월02일 데이터부터 제공) |

**입력 파라미터**: 없음

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 종목명 | ISU_ABBRV | string |
| 3 | 현재가 | TDD_CLSPRC | int |
| 4 | 전일비 | CMPPREVDD_PRC | int |
| 5 | 등락률 | FLUC_RT | int |
| 6 | 상장좌수 | LIST_SHRS | int |
| 7 | 거래량(좌) | ACC_TRDVOL | int |
| 8 | 거래대금(원) | ACC_TRDVAL | int |

---

### 7.3 ESG 지수

| 항목 | 값 |
|------|-----|
| API ID | `esg_index_info` |
| URL | `/svc/apis/esg/esg_index_info` |
| 설명 | ESG 지수 정보를 제공 ('20년01월02일 데이터부터 제공) |

**입력 파라미터**: 없음

**출력 필드**

| No | 항목명 | 필드명 | 타입 |
|----|--------|--------|------|
| 1 | 기준일자 | BAS_DD | string |
| 2 | 지수명 | IDX_NM | string |
| 3 | 현재가 | CLSPRC_IDX | int |
| 4 | 전일비 | PRV_DD_CMPR | int |
| 5 | 등락률 | UPDN_RATE | int |
| 6 | 구성종목수 | TRD_ISU_CNT | int |
| 7 | 거래량(천주) | ACC_TRDVOL | int |
| 8 | 거래대금(백만원) | ACC_TRDVAL | int |

---

## API ID 요약표

| 카테고리 | API명 | API ID |
|----------|-------|--------|
| 지수 | KRX 시리즈 일별시세정보 | krx_dd_trd |
| 지수 | KOSPI 시리즈 일별시세정보 | kospi_dd_trd |
| 지수 | KOSDAQ 시리즈 일별시세정보 | kosdaq_dd_trd |
| 지수 | 채권지수 시세정보 | bon_dd_trd |
| 지수 | 파생상품지수 시세정보 | drvprod_dd_trd |
| 주식 | 유가증권 일별매매정보 | stk_bydd_trd |
| 주식 | 코스닥 일별매매정보 | ksq_bydd_trd |
| 주식 | 코넥스 일별매매정보 | knx_bydd_trd |
| 주식 | 신주인수권증권 일별매매정보 | sw_bydd_trd |
| 주식 | 신주인수권증서 일별매매정보 | sr_bydd_trd |
| 주식 | 유가증권 종목기본정보 | stk_isu_base_info |
| 주식 | 코스닥 종목기본정보 | ksq_isu_base_info |
| 주식 | 코넥스 종목기본정보 | knx_isu_base_info |
| 증권상품 | ETF 일별매매정보 | etf_bydd_trd |
| 증권상품 | ETN 일별매매정보 | etn_bydd_trd |
| 증권상품 | ELW 일별매매정보 | elw_bydd_trd |
| 채권 | 국채전문유통시장 일별매매정보 | kts_bydd_trd |
| 채권 | 일반채권시장 일별매매정보 | bnd_bydd_trd |
| 채권 | 소액채권시장 일별매매정보 | smb_bydd_trd |
| 파생상품 | 선물 일별매매정보 (주식선물外) | fut_bydd_trd |
| 파생상품 | 주식선물(유가) 일별매매정보 | eqsfu_stk_bydd_trd |
| 파생상품 | 주식선물(코스닥) 일별매매정보 | eqkfu_ksq_bydd_trd |
| 파생상품 | 옵션 일별매매정보 (주식옵션外) | opt_bydd_trd |
| 파생상품 | 주식옵션(유가) 일별매매정보 | eqsop_bydd_trd |
| 파생상품 | 주식옵션(코스닥) 일별매매정보 | eqkop_bydd_trd |
| 일반상품 | 석유시장 일별매매정보 | oil_bydd_trd |
| 일반상품 | 금시장 일별매매정보 | gold_bydd_trd |
| 일반상품 | 배출권 시장 일별매매정보 | ets_bydd_trd |
| ESG | 사회책임투자채권 정보 | sri_bond_info |
| ESG | ESG 증권상품 | esg_etp_info |
| ESG | ESG 지수 | esg_index_info |

---

## 공통 필드 설명

| 필드명 | 설명 |
|--------|------|
| BAS_DD | 기준일자 (YYYYMMDD 형식) |
| ISU_CD | 종목코드 (표준코드) |
| ISU_SRT_CD | 단축코드 |
| ISU_NM | 종목명 |
| TDD_CLSPRC | 당일 종가 |
| TDD_OPNPRC | 당일 시가 |
| TDD_HGPRC | 당일 고가 |
| TDD_LWPRC | 당일 저가 |
| CMPPREVDD_PRC | 전일 대비 |
| FLUC_RT | 등락률 (%) |
| ACC_TRDVOL | 누적 거래량 |
| ACC_TRDVAL | 누적 거래대금 |
| MKTCAP | 시가총액 |
| LIST_SHRS | 상장주식수/상장좌수 |

---

## 참고 사항

1. **인증키 발급**: KRX OPEN API 사용을 위해서는 회원가입 후 인증키를 발급받아야 합니다.
2. **데이터 지연**: 실시간 데이터가 아닌 일별 데이터이며, T+1 기준으로 제공됩니다.
3. **호출 제한**: API 호출에는 Rate Limit이 있을 수 있으므로 적절한 간격으로 호출해야 합니다.
4. **데이터 형식**: 대부분의 필드는 string 타입으로 반환되며, 숫자 처리 시 형변환이 필요합니다.

---

> 문서 출처: https://openapi.krx.co.kr/
