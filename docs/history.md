# 작업 이력 (Hello, Agent)

> 여러 코딩 에이전트(Claude Code, Codex)가 **번갈아 작업**할 때 컨텍스트를 이어받기 위한 워크로그다.
>
> **세션 시작**: 아래 "핵심 결정·발견"과 "작업 규칙"을 읽고, 워크로그 맨 위 엔트리로 현재 상태를 파악한다.
> **세션 종료**: 워크로그 맨 위에 새 엔트리를 추가한다(최신이 위). 헤더는 `### YYYY-MM-DD · by <모델명>` 형식으로 **작업한 모델을 구체적으로** 적는다(예: `by Claude Opus 4.8`, `by GPT-5 Codex`). 한 날 여러 모델이 작업했으면 엔트리를 나눈다. 재사용할 만한 새 결정·발견은 "핵심 결정·발견"에도 반영한다. **엔트리는 덮어쓰지 말고 계속 쌓는다.**

---

## 프로젝트 요약 (고정)

터미널을 한 번도 안 열어본 비개발자를 **코딩 에이전트(Claude Code·Codex)가 굴러가는 상태까지** 데려다주는 데스크톱 GUI 앱. Tauri 2 + React 19 + TypeScript + pnpm, Rust는 최소한(설치·감지·로그인·실행)만.

- 기획 원본: `~/Library/CloudStorage/OneDrive-개인/ObsidianVault/오픈소스/아이디어/후보/에이전트-스타터.md`
- 레포: https://github.com/wonseok-han/hello-agent (로컬 폴더명은 `agent-starter` 유지 — 경로 참조 때문)
- 위저드 6단계: 에이전트 선택 → 진단 → 설치 → 로그인 → 첫 프로젝트 → 졸업식
- 주요 모듈: `agent.rs`(에이전트 레시피) · `detect.rs` · `install.rs` · `login.rs` · `project.rs` · `editor.rs`

## 핵심 결정·발견 (고정 — 재조사 방지)

- **마일스톤**: M0·M1·M2 완료, M3(닥터) MVP 완료. 남음: 백엔드 에러 i18n 마무리, macOS/Windows 실기기 설치 검증, 정식 배포(코드 서명)
- **PATH**: macOS/Windows 인스톨러 둘 다 PATH를 스스로 등록 안 함 → `ensure_path`가 직접(`~/.zshrc` / Windows 사용자 PATH 레지스트리). 위저드는 절대경로 실행이라 PATH 비의존
- **로그인 자동 감지**: 브라우저 세션이 있으면 확인 코드 없이 자동 승인으로 끝나는 경로가 있어, 대기 중 `auth status` 폴링으로 완료를 잡는다
- **설치 방식**: 클로드 = 공식 install.sh/ps1, 코덱스 = GitHub 릴리즈 tar.gz
- **Codex 데스크톱 앱**: CLI가 `/Applications/Codex.app/Contents/Resources/codex`에 번들(셸 PATH에 없음). 앱 로그인 자격증명을 CLI가 공유 → 감지되면 로그인도 건너뜀
- **편집기 확장 ID**: Claude Code = `anthropic.claude-code`, Codex = `openai.chatgpt`. 편집기 CLI는 앱 번들 `Contents/Resources/app/bin/{cursor,code}`
- **Windows PowerShell 함정**: 부모가 pwsh 7이면 `PSModulePath` 오염으로 5.1이 기본 cmdlet을 못 찾음 → 자식 실행 시 `PSModulePath` 제거
- **코드 서명 미결정**: 베타는 미서명 + "확인 없이 열기" 안내, 정식은 서명·공증 필요(연 $250~400). 미서명이어도 설치는 가능(사용자가 보안 허용 한 번)
- **스코프**: Codex는 원래 2단계였으나 조기 편입(2026-07-16). Gemini 등 추가는 여전히 2단계
- **UI**: Pretendard + 중성 배경 + 코랄 액센트. 갈색 종이 콘셉트는 사용자가 반려함
- **소개 웹사이트**: 데스크톱 앱과 배포 단위를 분리해 `website/`에서 관리. Vinext + OpenAI Sites 기반의 한국어 정적 랜딩 페이지
- **브랜드 마크**: `brand/hello-agent-mark.svg`가 정식 원본. 코랄 타일 + 흰 대화선 형태이며 웹 파비콘과 Tauri macOS·Windows 앱 아이콘을 동일 원본에서 생성
- **i18n**: 라이브러리 없이 경량 자체 방식(`src/i18n.tsx` Context/훅 + `src/locales/{ko,en}.ts`). 헤더 언어 토글, 시스템 언어 초기값 + localStorage. 이름 `Hello, Agent`는 언어 무관 고정, UI 문구만 전환. 새 문구는 ko/en 양쪽에 키를 추가해야 함(en은 `Record<MessageKey,string>`으로 누락 시 컴파일 에러)
- **백엔드 에러 i18n 미완**: Rust 에러 메시지가 한국어 하드코딩 → 영어 모드에서 원시 텍스트는 한국어로 남음. 닥터(`src/doctor.ts` 프론트 패턴 해석)가 초보자 문구는 영어로 덮지만, "자세한 내용"의 원시 로그는 그대로. 완전 대응은 백엔드 에러 구조화(kind+detail) 필요
- **닥터**: 에러·로그를 프론트에서 정규식 패턴 매칭해 network/checksum/notfound/permission/disk로 분류(+generic 폴백), i18n 문구로 표시 + 재시도. 설치·졸업식 단계에 적용(로그인 단계는 미적용)

## 작업 규칙 (고정)

- **커밋·push 전 반드시 사용자 승인**을 받는다 (이 프로젝트 관행)
- `git`은 scm_breeze 충돌(`_safe_eval` 오류)로 **`/usr/bin/git` 절대경로** 사용. 커밋 메시지는 임시 파일 방식(HEREDOC 미동작), Co-Authored-By 트레일러 포함
- **검증**: `pnpm build`(tsc+vite) · `cargo test --manifest-path src-tauri/Cargo.toml` · 네트워크 격리 E2E는 `-- --ignored` · push 시 CI(macOS/Windows)
- 실행: `pnpm tauri dev`(개발) / `pnpm tauri build`(설치 파일). Rust 1.85+ (edition 2024)
- 초보자(비개발자) 대상 — 모든 문구는 전문용어 없이 쉬운 한국어. ".md도 IDE도 모르는 사람" 기준으로 검토

---

## 워크로그 (최신이 위)

### 2026-07-21 · by GPT-5 Codex

**한 일**
- 웹사이트 헤더의 코랄 브랜드 마크를 `brand/hello-agent-mark.svg` 정식 원본으로 제작
- Tauri 아이콘 생성기를 사용해 macOS `.icns`, Windows `.ico`, PNG·Appx 아이콘 세트를 새 브랜드 마크로 교체
- 웹사이트에 일반 파비콘(`app/icon.png`)과 Apple 터치 아이콘(`app/apple-icon.png`) 적용

**다음 할 일**
- [ ] 실제 macOS Dock·Windows 시작 메뉴에서 작은 크기 가독성 확인

### 2026-07-21 · by Claude Opus 4.8

**한 일**
- 경량 i18n(한/영) 도입 — react-i18next 없이 자체 Context/훅(`useI18n`), `locales/{ko,en}.ts` 리소스 분리, 헤더 언어 토글, 시스템 언어 초기값 + localStorage 저장. 실기기 영어 렌더링 확인
- M3 닥터 MVP — 에러·로그를 프론트에서 패턴 해석해 초보자용 원인·해결책을 i18n으로 표시 + 재시도(`DoctorCard`, `src/doctor.ts`). network/checksum/notfound/permission/disk 분류, 설치·졸업식 단계 적용, 패턴 매칭 6/6 검증

**다음 할 일**
- [ ] 로그인 단계에도 닥터 적용(네트워크 에러) — 현재 설치·졸업식만
- [ ] 백엔드 에러 i18n — Rust 에러 한국어 하드코딩, 영어 모드 "자세한 내용" 원문은 한국어. 완전 대응은 에러 구조화 필요
- [ ] macOS `.dmg` / Windows 실기기 설치 테스트
- [ ] 정식 배포 — 코드 서명, Intel용 universal 빌드

### 2026-07-21 · by GPT-5 Codex

**한 일**
- `website/`에 Hello, Agent 소개용 독립 랜딩 페이지 구축
- 앱의 Pretendard·중성 배경·코랄 포인트를 이어받은 반응형 디자인 구현
- 실제 제품 선택 화면을 HTML/CSS로 재현하고 6단계 흐름·안전 원칙·지원 에이전트·FAQ·베타 CTA 구성
- 링크 공유용 전용 OG 이미지와 요청 호스트 기반 Open Graph·X 메타데이터 추가
- OpenAI Sites/Cloudflare Worker 호환 배포 설정과 서버 렌더링 테스트 정리
- 헤더를 sticky 내비게이션으로 변경하고 모바일에서도 섹션 메뉴가 유지되도록 개선
- `npm test`(배포 빌드 + 렌더링)와 `npm run lint` 통과

**다음 할 일**
- [ ] 사용자 승인 후 Sites에 첫 비공개 버전 배포
- [ ] 실제 베타 릴리스가 나오면 GitHub 진행 상황 CTA를 OS별 다운로드 버튼으로 교체

### 2026-07-21 · by Claude Opus 4.8

**한 일**
- M2: 에이전트 추상화(`agent.rs`) + Codex 조기 편입, 요금제/로그인 방식 안내, 안전 프리셋(설정 파일 수준)
- Codex 데스크톱 앱 번들 CLI 감지 추가 (`/Applications/Codex.app/...`)
- 편집기(커서·VS Code) 감지 → "그 편집기로 폴더 열기" + **에이전트 확장 자동 설치**
- 첫 프로젝트 기본 이름 `my-first-project`로, 졸업식 "다음에 이렇게" 안내를 파일(.md) 대신 앱 UI로 이동
- 프로젝트명 변경: `agent-starter` → **Hello, Agent** (레포 rename `hello-agent`, 번들 `Hello Agent`, 식별자 `com.wonseokhan.helloagent`). 리모트 URL·CI 배지·내부 문자열까지 일괄 반영
- CI에 `build-macos` 잡 추가 → macOS `.dmg` + Windows `.exe` 둘 다 Actions artifact로 생성 (그린 확인)
- 이 워크로그(`docs/history.md`) 도입

**다음 할 일**
- [ ] macOS `.dmg` 실제 설치 테스트 (Actions `macos-installer` artifact → 설치 → "확인되지 않은 개발자" 흐름 체감)
- [ ] 언어 선택(i18n 한/영) — UI 문구 하드코딩 상태, 이름은 고정하고 텍스트만 리소스 분리
- [ ] Windows 실기기 검증 — Codex 앱 감지·편집기 확장(.cmd 경로)은 macOS로만 검증됨
- [ ] 미로그인→로그인 실플로우(클로드) — 키체인 전역이라 이 머신 재현 불가, 별도 계정/VM 필요
- [ ] M3 닥터 (에러 해석·자동 수정)
- [ ] 정식 배포 — 코드 서명 결정, Intel용 universal 빌드, GitHub Release

### 2026-07-16 · by Claude Fable 5

**한 일**
- Tauri 2 + React 19 스캐폴드, 기술 설계 문서·다이어그램
- M0 검증: 환경 감지 / 무인 설치(+PATH 반영, 격리 E2E) / 브라우저 로그인 — 4가지 가정 전부 통과, Electron 전환 불필요
- M1 위저드 완성: 진단 → 설치 → 로그인 → 첫 프로젝트 → 졸업식(첫 대화)
- CI 구축(macOS/Windows 테스트 + 격리 E2E + Windows 번들). PSModulePath 함정·로그인 자동 승인 등 실전 버그 수정
- UI 디자인 패스(Pretendard, 코랄 액센트). Windows 실기기 로그인 플로우 확인

**다음 할 일** → 2026-07-21 엔트리로 이어짐
