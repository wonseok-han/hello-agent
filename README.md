# Agent Starter

> 터미널을 한 번도 안 열어본 사람을 **코딩 에이전트가 굴러가는 상태까지** 데려다주는 데스크톱 앱

[![CI](https://github.com/wonseok-han/agent-starter/actions/workflows/ci.yml/badge.svg)](https://github.com/wonseok-han/agent-starter/actions/workflows/ci.yml)

코딩 에이전트(클로드 코드, 코덱스)를 쓰고 싶지만 **설치·설정 단계에서 막히는** 비개발자를 위한 도우미예요. 검은 터미널 창을 열 필요 없이, 버튼을 따라 누르기만 하면 첫 대화까지 도달합니다.

## 무엇을 해 주나요

더블클릭으로 실행하는 데스크톱 앱이 **6단계**를 대신 처리합니다. 터미널은 한 번도 보이지 않고, 모든 진행 상황은 쉬운 한국어로 안내돼요.

1. **에이전트 선택** — 클로드 코드 / 코덱스 중 쓰던 구독에 맞게 고르기
2. **진단** — 내 컴퓨터에 이미 설치돼 있는지, Node 등 환경 확인
3. **설치** — 공식 배포처에서 자동 설치 + 터미널 PATH까지 정리
4. **로그인** — 브라우저 로그인 연동 (구독/API 방식 안내 포함)
5. **첫 프로젝트** — 안전장치가 들어간 작업 폴더 생성
6. **졸업식** — 에이전트와 첫 대화를 나누고 마무리

## 지원 범위 (MVP)

| 항목 | 지원 |
|---|---|
| 코딩 에이전트 | 클로드 코드 (Anthropic), 코덱스 (OpenAI) |
| 운영체제 | macOS, Windows |
| 언어 | 한국어 |

## 왜 필요한가요

초보자의 코딩 에이전트 첫 사용은 실제로 40분 이상 걸리고, 대부분 **터미널 공포·PATH 문제·개념 혼란·에러 복구 불가**에서 이탈합니다. 시중엔 설치 가이드 글은 많아도 *도구*는 없었어요. Agent Starter는 그 공백을 채우는, "이 앱 하나 받아서 실행해"로 끝나는 물건을 목표로 합니다.

## 개발

Tauri 2 + React 19 + TypeScript, 패키지 매니저는 pnpm입니다. Rust는 설치·감지·로그인 등 시스템 작업에만 최소한으로 씁니다.

```bash
pnpm install                                    # 의존성 설치
pnpm tauri dev                                  # 개발 모드 실행
pnpm build                                      # 프론트엔드 빌드 검증 (tsc + vite)
cargo test --manifest-path src-tauri/Cargo.toml # Rust 테스트
```

- Rust 1.85+ 필요 (edition 2024)
- 네트워크가 필요한 격리 설치 E2E 테스트는 `-- --ignored` 로 실행합니다 (CI에서 macOS·Windows 모두 검증)

## 문서

- [기술 설계](docs/architecture.md) — 시스템 구성, 위저드 상태 머신, 에이전트 레시피, 플랫폼별 전략, 검증 현황
- 설계 다이어그램은 `docs/diagrams/*.excalidraw`

## 상태

개발 중입니다. 기술 검증(M0)과 해피패스 위저드(M1), 개념·안전 안내(M2)가 동작하며, 에러 자동 해석(M3 닥터)과 정식 배포가 남았습니다.
