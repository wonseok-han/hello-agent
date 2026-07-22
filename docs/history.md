# 작업 이력 (Hello, Agent)

> 여러 코딩 에이전트(Claude Code, Codex)가 **번갈아 작업**할 때 컨텍스트를 이어받기 위한 워크로그다.
>
> **세션 시작**: 아래 "핵심 결정·발견"과 "작업 규칙"을 읽고, 워크로그 맨 위 엔트리로 현재 상태를 파악한다.
> **세션 종료**: 코드 변경이 없어도 프로젝트에 영향을 주는 분석·결정·검증·외부 작업을 했다면 워크로그 맨 위에 새 엔트리를 추가한다(최신이 위). 헤더는 `### YYYY-MM-DD · by <모델명>` 형식으로 **작업한 모델을 구체적으로** 적는다(예: `by Claude Opus 4.8`, `by GPT-5 Codex`). 한 날 여러 모델이 작업했으면 엔트리를 나눈다. 재사용할 만한 새 결정·발견은 "핵심 결정·발견"에도 반영한다. **엔트리는 덮어쓰지 말고 계속 쌓는다.**

---

## 프로젝트 요약 (고정)

터미널을 한 번도 안 열어본 비개발자를 **코딩 에이전트(Claude Code·Codex)가 굴러가는 상태까지** 데려다주는 데스크톱 GUI 앱. Tauri 2 + React 19 + TypeScript + pnpm, Rust는 최소한(설치·감지·로그인·실행)만.

- 기획 원본: `~/Library/CloudStorage/OneDrive-개인/ObsidianVault/오픈소스/아이디어/후보/에이전트-스타터.md`
- 레포: https://github.com/wonseok-han/hello-agent (로컬 폴더명은 `agent-starter` 유지 — 경로 참조 때문)
- 위저드 6단계: 에이전트 선택 → 진단 → 설치 → 로그인 → 첫 프로젝트 → 졸업식
- 주요 모듈: `agent.rs`(에이전트 레시피) · `detect.rs` · `install.rs` · `login.rs` · `project.rs` · `editor.rs`

## 핵심 결정·발견 (고정 — 재조사 방지)

- **마일스톤**: M0·M1·M2 완료, M3(닥터) MVP 완료. 홈베이스·프로젝트 디스크 스캔·에이전트 상태/업데이트까지 구현. 남음: 전체 실패 경로 닥터 적용, 콘텐츠/원시 오류 i18n 마무리, macOS/Windows 출시 후보 실기기 검증, 정식 배포(코드 서명)
- **PATH**: macOS/Windows 인스톨러 둘 다 PATH를 스스로 등록 안 함 → `ensure_path`가 직접(`~/.zshrc` / Windows 사용자 PATH 레지스트리). 위저드는 절대경로 실행이라 PATH 비의존
- **로그인 자동 감지**: 브라우저 세션이 있으면 확인 코드 없이 자동 승인으로 끝나는 경로가 있어, 대기 중 `auth status` 폴링으로 완료를 잡는다
- **설치 방식**: 클로드 = 공식 install.sh/ps1, 코덱스 = GitHub 릴리즈 tar.gz
- **Codex 데스크톱 앱**: CLI가 앱 번들 `Contents/Resources/codex`에 번들(셸 PATH에 없음). 앱 로그인 자격증명을 CLI가 공유 → 감지되면 로그인도 건너뜀. **벤더가 앱을 옮김**: OpenAI가 Codex.app을 ChatGPT.app으로 통합(2026-07) → `/Applications/{ChatGPT,Codex}.app/...` 여러 후보로 감지(단일 경로 하드코딩 금지). 앞으로도 바뀔 수 있으니 후보 추가로 대응
- **편집기 확장 ID**: Claude Code = `anthropic.claude-code`, Codex = `openai.chatgpt`. 편집기 CLI는 앱 번들 `Contents/Resources/app/bin/{cursor,code}`
- **Windows PowerShell 함정**: 부모가 pwsh 7이면 `PSModulePath` 오염으로 5.1이 기본 cmdlet을 못 찾음 → 자식 실행 시 `PSModulePath` 제거
- **코드 서명 미결정**: 베타는 미서명 + "확인 없이 열기" 안내, 정식은 서명·공증 필요(연 $250~400). 미서명이어도 설치는 가능(사용자가 보안 허용 한 번)
- **스코프**: Codex는 원래 2단계였으나 조기 편입(2026-07-16). Gemini 등 추가는 여전히 2단계
- **UI**: Pretendard + 중성 배경 + 코랄 액센트. 갈색 종이 콘셉트는 사용자가 반려함
- **소개 웹사이트**: 데스크톱 앱과 배포 단위를 분리해 `website/`에서 관리. Vinext + OpenAI Sites 기반의 한국어 정적 랜딩 페이지. 온보딩 6단계뿐 아니라 프로젝트 자동 발견·에이전트 상태·업데이트를 관리하는 재방문 홈베이스도 핵심 가치로 소개
- **브랜드 마크**: `brand/hello-agent-mark.svg`가 정식 원본. 코랄 타일 + 흰 대화선 형태이며 웹 파비콘과 Tauri macOS·Windows 앱 아이콘을 동일 원본에서 생성
- **i18n**: 라이브러리 없이 경량 자체 방식(`src/i18n.tsx` Context/훅 + `src/locales/{ko,en}.ts`). 헤더 언어 토글, 시스템 언어 초기값 + localStorage. 이름 `Hello, Agent`는 언어 무관 고정, UI 문구만 전환. 새 문구는 ko/en 양쪽에 키를 추가해야 함(en은 `Record<MessageKey,string>`으로 누락 시 컴파일 에러)
- **백엔드 에러 구조화 완료, i18n 일부 남음**: `AppError { kind, detail }`와 `ErrorKind` 도입 완료. 다만 `project.rs`·`login.rs`·`editor.rs` 일부 상세 오류와 첫 대화 프롬프트가 한국어 하드코딩이라 영어 UI에서 혼용 가능
- **닥터 적용 범위**: 구조화 오류를 network/checksum/notfound/permission/disk/generic으로 해석해 i18n 문구와 재시도를 제공. 현재 설치·졸업식에 적용됐고 진단·로그인·홈의 설정/업데이트 실패에는 아직 미적용
- **홈베이스**: 프로젝트가 있거나 설치·로그인된 에이전트가 있으면 홈으로 진입. 기준 폴더 바로 아래에서 `.claude`/`CLAUDE.md`/`AGENTS.md` 표식을 스캔하고, 에이전트 상태와 업데이트를 표시

## 작업 규칙 (고정)

- **워크로그 필수 범위**: 구현뿐 아니라 버그 분석, 코드 리뷰, 우선순위 결정, GitHub/배포 같은 외부 변경도 다음 작업자에게 의미가 있으면 이 문서에 기록. 최종 응답 전 `git diff -- docs/history.md`로 기록 여부 확인
- **커밋·push 전 반드시 사용자 승인**을 받는다 (이 프로젝트 관행)
- `git`은 scm_breeze 충돌(`_safe_eval` 오류)로 **`/usr/bin/git` 절대경로** 사용. 커밋 메시지는 임시 파일 방식(HEREDOC 미동작), Co-Authored-By 트레일러 포함
- **검증**: `pnpm build`(tsc+vite) · `cargo test --manifest-path src-tauri/Cargo.toml` · 네트워크 격리 E2E는 `-- --ignored` · push 시 CI(macOS/Windows)
- 실행: `pnpm tauri dev`(개발) / `pnpm tauri build`(설치 파일). Rust 1.85+ (edition 2024)
- 초보자(비개발자) 대상 — 모든 문구는 전문용어 없이 쉬운 한국어. ".md도 IDE도 모르는 사람" 기준으로 검토

---

## 워크로그 (최신이 위)

### 2026-07-22 · by GPT-5 Codex

**한 일 — 프로젝트 기억과 에이전트 인계 규칙 보강**
- 최신 코드와 워크로그를 대조해 다음 우선순위를 정리: ① 진단·로그인·홈 전체에 닥터 적용 ② 재방문 새 프로젝트 흐름 단축 ③ 첫 대화/원시 오류 i18n ④ 프로젝트 스캔 실패 구분 ⑤ 출시 후보 실기기 검증
- `CLAUDE.md`와 `AGENTS.md`를 최신 홈베이스 구조·코드 지도·검증 명령·비자명한 함정에 맞게 갱신
- 코드 변경이 없는 분석·결정·검증·외부 작업도 `docs/history.md` 기록 대상임을 시작/작업 중/종료 체크리스트로 명시
- 두 에이전트 안내 파일을 `.gitignore`에서 제거해 저장소를 통해 공유되도록 변경
- 상단 `핵심 결정·발견`의 오래된 백엔드 오류·닥터 적용 범위를 실제 코드 기준으로 교정

**다음 할 일**
- [ ] 진단·로그인·홈의 설정/업데이트 실패에 `DoctorCard` 적용 및 실제 실패 경로 검증
- [ ] 홈의 `새 프로젝트`에서 준비된 에이전트를 미리 선택하고 완료 단계 단축
- [ ] 첫 대화 프롬프트와 남은 Rust 한국어 상세 오류 i18n 정리
- [ ] macOS/Windows 출시 후보 설치 시나리오 검증

### 2026-07-22 · by GPT-5 Codex

**한 일 — 오늘 앱 고도화 내용을 소개 웹사이트에 반영**
- 메인 앱 미리보기를 에이전트 선택 화면에서 `내 에이전트`·`내 프로젝트` 홈 화면으로 교체
- 프로젝트 자동 발견, 설치·로그인 상태 확인, 에이전트 업데이트를 소개하는 `설치 후에도` 섹션 추가
- 이미 설치한 사용자와 재방문 사용자를 설명하도록 히어로·FAQ·검색/Open Graph 설명 갱신
- 웹사이트 `npm test`(빌드 + 렌더링 테스트)와 `npm run lint` 통과

### 2026-07-22 · by Claude Opus 4.8 (5)

**한 일 — 프로젝트 목록을 앱 기억→디스크 스캔 기반으로 (사용자 지적)**
- 문제: 홈 목록이 store(앱 기억)에서만 왔음 → 디스크에 폴더가 있어도 앱이 안 만든 것이면 "프로젝트 없음"
- `project.rs`: `scan_projects(base)` — 기준 폴더 하위에서 에이전트 표식(`.claude`/`CLAUDE.md`→클로드, `AGENTS.md`→코덱스)이 있는 폴더를 발견해 목록화. `default_projects_dir`(기본 Documents). `create_first_project`에 `base` 인자 추가(새 프로젝트가 기준 폴더에 생성)
- 기준 폴더를 사용자가 지정: `tauri-plugin-dialog` 추가, 홈에 "이 폴더에서 찾는 중 + 폴더 바꾸기"(디렉터리 선택). store에 `baseDir` 저장
- store.ts: `getBaseDir/setBaseDir/lastOpenedMap`. 목록은 스캔 결과 + store의 최근 사용 시각으로 정렬. "목록에서 지우기"는 제거(목록=디스크 실제 폴더라 의미 안 맞음)
- **실기기 검증**: store 비운 상태에서 `~/Documents/my-first-project`(AGENTS.md)가 코덱스 프로젝트로 자동 인식·표시. cargo test 12개 + pnpm build 통과

### 2026-07-22 · by Claude Opus 4.8 (4)

**한 일 — 홈 진입 조건 개선 + 레이아웃 수정 (사용자 요청)**
- 홈 진입 조건: 저장된 프로젝트가 있거나 **에이전트가 이미 설치·로그인돼 있으면** 홈으로(온보딩 위저드 건너뜀). 둘 다 아니면 위저드. `isAnyAgentReady()`가 agent_status로 판정. 실기기: 프로젝트 0인데 claude 준비돼 있어 홈 직행 확인
- 레이아웃: 헤더 하단 여백이 위저드의 `.steps` margin에만 의존해 홈 뷰에서 패널이 타이틀에 붙던 문제 → `.header{margin-bottom}` + `.steps` top margin 제거로 두 뷰 일관화

### 2026-07-22 · by Claude Opus 4.8 (3)

**한 일 — 고도화: 상주 에이전트 상태 + 업데이트 알림 (홈베이스의 "재방문 이유")**
- `src-tauri/src/status.rs` 신설 — `agent_status`(설치·버전·로그인, 로컬 빠름) + `latest_agent_version`(네트워크). 최신 버전: Claude=`downloads.claude.ai/.../stable`, Codex=`github.com/.../releases/latest` 리다이렉트 최종 URL에서 semver 추출(api.github.com은 60회/시 제한이라 회피)
- 홈에 "내 에이전트" 패널(`AgentRow`): 각 에이전트 설치/로그인/업데이트 상태를 한눈에. 미설치·미로그인→"설정하기"(위저드 진단부터, 자동 스킵), 업데이트 가능→"업데이트"(인라인 install_agent). 최신 확인은 백그라운드(느림/오프라인이면 조용히 무시)
- 프론트 semver 비교(`isNewer`), `detect::agent_version`/`login::is_logged_in` pub(crate) 노출
- **실기기 검증**: 홈에서 클로드="설치됨·2.1.216"(최신) 정상 렌더. cargo test 11개 + pnpm build 통과
- **감지 갭 수정**(사용자 지적): OpenAI가 Codex.app→ChatGPT.app 통합으로 CLI 경로 이동 → 코덱스가 "미설치"로 잘못 표시됨. `agent.rs` Codex 후보를 `/Applications/{ChatGPT,Codex}.app/...` 여러 개로 확장. 실기기에서 `ChatGPT.app/.../codex` 0.145.0 감지 확인. 교훈: 앱 번들 경로 단일 하드코딩은 벤더 변경에 취약

**다음 할 일**
- [ ] 업데이트/설정 실패 시 홈에서 DoctorCard 노출(현재는 설정 화면으로 유도만)
- [ ] 재방문 "새 프로젝트"에서 에이전트 미리 선택
- [ ] 첫 대화 프롬프트 한국어 고정(콘텐츠 i18n)
- [ ] 레시피 외부화(agent.rs → 원격 JSON)

### 2026-07-22 · by Claude Opus 4.8 (2)

**한 일 — 고도화 ②③: 영속성 + 홈베이스("일회성 도구" 탈출)**
- `docs/architecture.md §3` 재설계 — 단일 선형 위저드 → 뷰 라우팅(홈/위저드) 홈베이스. 앱 실행 시 저장된 프로젝트 있으면 홈, 없으면 온보딩 위저드
- 영속성: `tauri-plugin-store` 추가(Rust 등록 + `store:default` capability + JS `@tauri-apps/plugin-store`). `src/store.ts`가 프로젝트 목록을 앱 데이터 디렉터리 JSON에 저장(경로·에이전트·이름·생성/최근사용). 에이전트 설치·로그인 상태는 저장 안 하고 매번 실시간 감지
- `App.tsx`에 `view` 라우팅 + `HomeView`(프로젝트 카드: 열기/목록에서 지우기, 새 프로젝트, 최근순 정렬). `ProjectStep`이 생성 시 `saveProject`, 졸업식에 "내 프로젝트 →" 링크
- **실기기 검증 완료**: store에 샘플 심고 재실행 → 홈 화면·프로젝트 카드 정상 렌더 확인. cargo test 10개 + pnpm build 통과. 테스트 데이터 정리함
- 주의: 앱 바이너리명이 rename 후 **`hello-agent`** (pgrep/스크린샷 시 옛 이름 agent-starter 아님)

**다음 할 일**
- [ ] 상주 닥터(홈에서 재진단) + 에이전트 업데이트 알림 — 홈베이스의 "재방문 이유" 완성
- [ ] 재방문 "새 프로젝트"에서 에이전트 미리 선택(현재는 위저드가 install/login만 자동 스킵)
- [ ] 첫 대화 프롬프트 한국어 고정(콘텐츠 i18n)
- [ ] 레시피 외부화(agent.rs → 원격 JSON)

### 2026-07-22 · by Claude Opus 4.8

**한 일 — 고도화 착수(기술부채 정리 ①): 백엔드 에러 구조화**
- `src-tauri/src/error.rs` 신설 — `AppError { kind, detail }` + `ErrorKind`(network/checksum/not-found/permission/disk/generic). 모든 커맨드가 `Result<T, String>` → `Result<T, AppError>`로 전환
- 원인을 아는 지점은 명시적 kind(예: curl 실패→network, tar 실패→checksum), io 에러는 `AppError::classify`로 원문(영어) 기반 분류. **에러 detail이 더는 한국어 프로즈가 아니라 영어 기술 텍스트** → 영어 모드에서 "자세한 내용"도 영어
- 프론트 `doctor.ts`: `diagnoseError`(kind 우선, generic이면 로그 정규식 보완) + `toAppError`(객체/문자열 수용). `DoctorCard`가 문자열 대신 `AppError`를 받음
- 검증: cargo test 10개 통과(에러 분류 단위테스트 포함), pnpm build 통과. 해피패스(Ok) 구조 불변이라 런타임 리스크 낮음, 실제 에러 경로는 실기기 검증 대기

**다음 할 일**
- [ ] 영속성 계층(tauri-plugin-store) → 홈베이스의 전제
- [ ] 홈 화면 + 재방문 흐름(일회성 탈출) — App.tsx를 뷰 라우팅으로
- [ ] 상주 닥터 + 에이전트 업데이트 알림
- [ ] 첫 대화 프롬프트가 한국어 고정 — 영어 사용자에게 한국어 인사(콘텐츠 i18n, 별건)
- [ ] 레시피 외부화(agent.rs → 원격 JSON)

### 2026-07-21 · by GPT-5 Codex

**한 일**
- GitHub 저장소 About 설명을 비개발자용 Claude Code·Codex 설치 도우미라는 제품 성격이 드러나도록 작성
- 제품·기술 스택·지원 플랫폼 중심의 GitHub 토픽 13개 등록

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
