# Hello, Agent

터미널을 한 번도 열어보지 않은 비개발자가 Claude Code·Codex를 설치하고, 안전한 첫 프로젝트를 시작한 뒤 계속 관리하도록 돕는 Tauri 데스크톱 앱.

## 가장 먼저 지킬 인계 규칙

`docs/history.md`가 Claude와 Codex가 공유하는 **프로젝트의 단일 작업 기억**이다.

### 작업 시작 전 — 반드시

1. `docs/history.md`를 읽는다. 최소한 `핵심 결정·발견`, `작업 규칙`, 최신 워크로그와 미완료 `다음 할 일`을 확인한다.
2. 문서의 현재 상태를 실제 코드·Git 상태와 대조한다. 오래된 항목을 사실처럼 반복하지 않는다.
3. 사용자가 지정한 범위와 현재 워킹 트리의 기존 변경을 확인한다.

### 작업 중 — 발견 즉시

- 다음 작업자가 재조사하지 않아도 될 결정·함정·외부 변경은 `docs/history.md`의 `핵심 결정·발견`에도 반영한다.
- 과거 워크로그는 덮어쓰거나 삭제하지 않는다. 완료된 예전 TODO는 최신 엔트리에서 완료 사실을 남긴다.
- 코드와 문서가 충돌하면 코드를 확인해 현재 사실로 바로잡고, 확인하지 못한 내용은 추정이라고 표시한다.

### 응답을 마치기 전 — 반드시

코드 변경 여부와 관계없이, **프로젝트에 영향을 주는 작업·분석·결정·검증·외부 변경**을 했다면 `docs/history.md` 맨 위에 새 엔트리를 추가한다.

- 형식: `### YYYY-MM-DD · by <정확한 모델명>`
- 포함: 한 일, 중요한 결정·발견, 검증 결과, 남은 다음 할 일
- 예: 구현, 버그 분석, 코드 리뷰, 우선순위 제안, GitHub 메타데이터 변경, 배포·릴리스 판단
- 단순 사용법 답변처럼 프로젝트 상태나 판단이 전혀 바뀌지 않은 대화만 생략할 수 있다.
- 최종 응답 전에 `git diff -- docs/history.md`로 이번 기록이 실제로 들어갔는지 확인한다.

## 현재 제품 범위

- 에이전트: Claude Code + Codex
- 플랫폼: macOS + Windows
- UI: 한국어·영어 전환, 이름 `Hello, Agent`는 고정
- 구조: 재방문용 홈베이스 + 6단계 위저드(에이전트 → 진단 → 설치 → 로그인 → 프로젝트 → 첫 대화)
- 홈베이스: 프로젝트 폴더 스캔, 에이전트 설치·로그인 상태, 업데이트 확인
- 상태: M0~M3와 주요 오류 복구 흐름 구현. 실기기 출시 후보 검증과 서명·배포 결정이 남음

## 코드 지도

- `src/App.tsx`: 최상위 홈/위저드 라우팅과 화면 컴포넌트
- `src/i18n.tsx`, `src/locales/{ko,en}.ts`: 경량 i18n. 새 키는 ko/en 양쪽에 추가
- `src/doctor.ts`: 구조화 오류를 초보자용 원인·해결책으로 변환
- `src/store.ts`: 기준 폴더와 프로젝트 최근 사용 시각 저장
- `src-tauri/src/agent.rs`: 에이전트별 설치·감지·로그인 레시피
- `src-tauri/src/{detect,install,login,project,status,editor,error}.rs`: 시스템 기능
- `website/`: 별도 Vinext/OpenAI Sites 소개 웹사이트
- `docs/architecture.md`: 기술 설계
- `docs/history.md`: 현재 상태와 작업 인계의 기준

## 구현 원칙과 함정

- 사용자는 비개발자다. 터미널을 노출하지 않고 문구는 전문용어 없이 쓴다.
- 안전 기본값을 우선하며 기존 사용자 설정 파일을 덮어쓰지 않는다.
- Rust 오류는 `AppError { kind, detail }`로 전달한다. 사용자 설명은 프론트 i18n, `detail`은 도움 요청용 기술 정보로 분리한다.
- OpenAI 앱 번들 경로는 바뀔 수 있다. Codex CLI 감지는 `/Applications/{ChatGPT,Codex}.app/...` 후보 방식을 유지한다.
- 프로젝트 스캔은 현재 기준 폴더의 바로 아래 하위 폴더만 검사한다.
- `scm_breeze`의 `_safe_eval` 오류가 나면 `/usr/bin/git`, `/bin/ls`, `/usr/bin/find`처럼 절대경로를 사용한다.

## 검증 명령

```bash
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build

cd website
npm test
npm run lint
```

- 네트워크 격리 설치 E2E: `cargo test --manifest-path src-tauri/Cargo.toml isolated_install -- --ignored --nocapture`
- Rust 1.85+ 필요(edition 2024)

## Git 작업 규칙

- 커밋과 push는 반드시 사용자 승인을 받은 뒤 실행한다.
- `git`은 `/usr/bin/git` 절대경로를 사용한다.
- 커밋 메시지는 임시 파일 방식으로 작성하고 `Co-Authored-By` 트레일러를 포함한다.
- 사용자 변경과 무관한 파일을 임의로 스테이징하거나 되돌리지 않는다.
