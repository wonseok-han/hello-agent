import { useEffect, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "./i18n";
import { diagnoseError, toAppError, type AppError } from "./doctor";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getBaseDir, setBaseDir, saveProject, lastOpenedMap } from "./store";
import type { MessageKey } from "./locales/ko";
import "./App.css";

interface ToolInfo {
  path: string;
  version: string;
  inShellPath: boolean;
}

interface EnvironmentReport {
  os: string;
  arch: string;
  osVersion: string | null;
  agent: ToolInfo | null;
  node: ToolInfo | null;
  checkedPaths: string[];
}

interface InstallResult {
  version: string;
  path: string;
  profileUpdated: string | null;
}

type InstallEvent =
  | { type: "phase"; name: string }
  | { type: "log"; line: string }
  | { type: "progress"; downloadedBytes: number };

interface LoginStatus {
  loggedIn: boolean;
  authMethod: string | null;
  email: string | null;
  subscriptionType: string | null;
}

type LoginEvent =
  | { type: "url"; url: string }
  | { type: "log"; line: string }
  | { type: "exit"; success: boolean };

interface ProjectInfo {
  path: string;
  created: boolean;
}

interface EditorInfo {
  id: string;
  name: string;
  url: string;
  installed: boolean;
}

interface AgentStatus {
  installed: boolean;
  version: string | null;
  loggedIn: boolean;
  path: string | null;
}

// 기준 폴더 스캔으로 발견한 프로젝트 (앱 기억이 아니라 디스크 실제 폴더)
interface ScannedProject {
  path: string;
  name: string;
  agent: string;
}
interface HomeProject extends ScannedProject {
  lastOpenedAt: number;
}

// 기준 폴더를 스캔해 프로젝트 목록을 만들고, store의 최근 사용 시각으로 정렬한다.
async function loadHomeProjects(base: string): Promise<HomeProject[]> {
  const [scanned, last] = await Promise.all([
    invoke<ScannedProject[]>("scan_projects", { base }),
    lastOpenedMap(),
  ]);
  return scanned
    .map((s) => ({ ...s, lastOpenedAt: last[s.path] ?? 0 }))
    .sort(
      (a, b) =>
        b.lastOpenedAt - a.lastOpenedAt || a.name.localeCompare(b.name),
    );
}

async function openDirectoryDialog(): Promise<string | null> {
  try {
    const picked = await openDialog({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  } catch {
    return null;
  }
}

const shortVersion = (v: string | null) => v?.split(" ")[0] ?? "";

// 설치·로그인까지 끝난 에이전트 목록. 홈 진입과 새 프로젝트 단축에 함께 쓴다.
async function getReadyAgents(): Promise<AgentId[]> {
  try {
    const ids: AgentId[] = ["claude-code", "codex"];
    const results = await Promise.all(
      ids.map((id) =>
        invoke<AgentStatus>("agent_status", { agent: id }).catch(() => null),
      ),
    );
    return ids.filter((_, index) => {
      const status = results[index];
      return status?.installed && status.loggedIn;
    });
  } catch {
    return [];
  }
}

// "x.y.z"를 뽑아 숫자 배열로. 최신 > 설치 판정용.
function semverParts(s: string): number[] | null {
  const m = s.match(/(\d+)\.(\d+)\.(\d+)/);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}
function isNewer(latest: string, installed: string): boolean {
  const a = semverParts(latest);
  const b = semverParts(installed);
  if (!a || !b) return false;
  for (let i = 0; i < 3; i++) {
    if (a[i] > b[i]) return true;
    if (a[i] < b[i]) return false;
  }
  return false;
}

type AgentId = "claude-code" | "codex";

const AGENTS: { id: AgentId; vendor: string; recommended?: boolean }[] = [
  { id: "claude-code", vendor: "Anthropic", recommended: true },
  { id: "codex", vendor: "OpenAI" },
];

const STEP_KEYS = [
  "agent",
  "diagnosis",
  "install",
  "login",
  "project",
  "graduation",
] as const;

const INSTALL_PHASE_IDS = ["download", "install", "path", "verify"] as const;
const KNOWN_PLANS = ["pro", "max", "team", "enterprise"];

// 편집기 표시명은 언어 리소스로 (백엔드가 준 name 대신 id 기준)
const editorLabel = (t: (k: MessageKey) => string, id: string) =>
  id === "cursor" || id === "vscode" ? t(`editor.${id}` as MessageKey) : id;

function osLabel(report: EnvironmentReport): string {
  if (report.os === "macos") {
    const chip = report.arch === "aarch64" ? "Apple Silicon" : "Intel";
    const ver = report.osVersion ? ` ${report.osVersion}` : "";
    return `macOS${ver} (${chip})`;
  }
  if (report.os === "windows") return "Windows";
  return `${report.os} (${report.arch})`;
}

// 닥터: 에러·로그를 초보자용 진단으로 풀어주고 재시도를 돕는다 (M3 — 삽질 분류 G)
function DoctorCard({
  error,
  log,
  onRetry,
  retryLabel,
}: {
  error: AppError;
  log?: string[];
  onRetry?: () => void;
  retryLabel?: string;
}) {
  const { t } = useI18n();
  const dx = diagnoseError(error, log);
  const detail = [error.detail, ...(log ?? [])].filter(Boolean).join("\n");
  return (
    <div className="doctor">
      <span className="doctor-badge">{t("doctor.badge")}</span>
      <strong>{t(dx.titleKey)}</strong>
      <p>{t(dx.adviceKey)}</p>
      {onRetry && (
        <button className="primary" onClick={onRetry}>
          {retryLabel ?? t("doctor.retry")}
        </button>
      )}
      {detail && (
        <details className="doctor-details">
          <summary>{t("doctor.details")}</summary>
          <pre>{detail}</pre>
        </details>
      )}
    </div>
  );
}

type View = "loading" | "home" | "wizard";

function App() {
  const { t, lang, setLang } = useI18n();
  const [view, setView] = useState<View>("loading");
  const [projects, setProjects] = useState<HomeProject[]>([]);
  const [baseDir, setBaseDirState] = useState<string>("");
  const [scanError, setScanError] = useState<AppError | null>(null);
  const [readyAgents, setReadyAgents] = useState<AgentId[]>([]);
  const [step, setStep] = useState(0);
  const [agent, setAgent] = useState<AgentId | null>(null);
  const [report, setReport] = useState<EnvironmentReport | null>(null);
  const [project, setProject] = useState<ProjectInfo | null>(null);

  // 홈 진입 조건: 기준 폴더에 실제 프로젝트 폴더가 있거나, 이미 에이전트를
  // 설치·로그인해 둔 경우(이미 준비된 사용자는 온보딩 불필요). 둘 다 아니면 위저드.
  async function refreshHome() {
    let base = await getBaseDir();
    if (!base) base = await invoke<string>("default_projects_dir");
    setBaseDirState(base);
    let list: HomeProject[] = [];
    let nextScanError: AppError | null = null;
    try {
      list = await loadHomeProjects(base);
    } catch (error) {
      nextScanError = toAppError(error);
    }
    const nextReadyAgents = await getReadyAgents();
    setProjects(list);
    setScanError(nextScanError);
    setReadyAgents(nextReadyAgents);
    if (list.length > 0 || nextReadyAgents.length > 0 || nextScanError) {
      setView("home");
      return;
    }
    setView("wizard");
  }
  useEffect(() => {
    refreshHome();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function changeBaseDir() {
    const picked = await openDirectoryDialog();
    if (!picked) return;
    await setBaseDir(picked);
    await refreshHome();
  }

  function selectAgent(id: AgentId) {
    setAgent(id);
    setReport(null);
    setProject(null);
    setStep(readyAgents.includes(id) ? 4 : 1);
  }

  function startWizard() {
    const onlyReadyAgent = readyAgents.length === 1 ? readyAgents[0] : null;
    setAgent(onlyReadyAgent);
    setReport(null);
    setProject(null);
    // 준비된 에이전트가 하나면 진단·설치·로그인을 다시 거치지 않고 프로젝트로.
    setStep(onlyReadyAgent ? 4 : 0);
    setView("wizard");
  }

  // 홈의 "내 에이전트"에서 특정 에이전트 설정으로 진입 — 진단부터(설치·로그인은 자동 스킵)
  function setupAgent(id: AgentId) {
    setAgent(id);
    setReport(null);
    setProject(null);
    setStep(1);
    setView("wizard");
  }

  async function goHome() {
    await refreshHome();
    setView("home");
  }

  // 배포 빌드에서 웹뷰 우클릭 메뉴를 막는다(입력창은 붙여넣기 위해 예외).
  // 개발 중에는 devtools 우클릭이 필요하므로 유지한다.
  useEffect(() => {
    if (!import.meta.env.PROD) return;
    const block = (e: MouseEvent) => {
      const el = e.target as HTMLElement;
      if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", block);
    return () => document.removeEventListener("contextmenu", block);
  }, []);

  return (
    <div className="app">
      <header className="header">
        <div className="lang-toggle">
          <button
            className={lang === "ko" ? "active" : ""}
            onClick={() => setLang("ko")}
          >
            {t("lang.ko")}
          </button>
          <button
            className={lang === "en" ? "active" : ""}
            onClick={() => setLang("en")}
          >
            {t("lang.en")}
          </button>
        </div>
        <p className="eyebrow">{t("app.eyebrow")}</p>
        <h1>Hello, Agent</h1>
        <p className="tagline">{t("app.tagline")}</p>
      </header>

      {view === "loading" ? (
        <main className="panel" />
      ) : view === "home" ? (
        <HomeView
          projects={projects}
          baseDir={baseDir}
          scanError={scanError}
          onNew={startWizard}
          onSetupAgent={setupAgent}
          onChangeBase={changeBaseDir}
          onChanged={refreshHome}
          onRetryScan={refreshHome}
        />
      ) : (
        <>
          <ol className="steps">
            {STEP_KEYS.map((key, i) => (
              <li
                key={key}
                className={i === step ? "active" : i < step ? "done" : ""}
              >
                <span className="step-dot">{i < step ? "✓" : i + 1}</span>
                {t(`steps.${key}` as MessageKey)}
              </li>
            ))}
          </ol>

          <main className="panel" key={step}>
            {step === 0 || !agent ? (
              <AgentStep selected={agent} onSelect={selectAgent} />
            ) : step === 1 ? (
              <DiagnosisStep
                agent={agent}
                report={report}
                onReport={setReport}
                onNext={() => setStep(2)}
              />
            ) : step === 2 ? (
              <InstallStep
                agent={agent}
                report={report}
                onNext={() => setStep(3)}
              />
            ) : step === 3 ? (
              <LoginStep agent={agent} onNext={() => setStep(4)} />
            ) : step === 4 ? (
              <ProjectStep
                agent={agent}
                project={project}
                onProject={setProject}
                onNext={() => setStep(5)}
              />
            ) : (
              <GraduationStep
                agent={agent}
                project={project}
                onHome={goHome}
              />
            )}
          </main>
        </>
      )}
    </div>
  );
}

// 상주 에이전트 상태: 설치·로그인·업데이트를 홈에서 한눈에 (일회성 탈출의 "재방문 이유")
function AgentRow({ id, onSetup }: { id: AgentId; onSetup: () => void }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [latest, setLatest] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const name = t(`agent.${id}.name` as MessageKey);

  async function refresh() {
    setError(null);
    setLatest(null);
    try {
      const s = await invoke<AgentStatus>("agent_status", { agent: id });
      setStatus(s);
      // 설치돼 있으면 백그라운드로 최신 버전 확인(느릴 수 있음, 실패는 조용히 무시)
      if (s.installed && s.version) {
        invoke<string | null>("latest_agent_version", { agent: id })
          .then((v) => setLatest(v))
          .catch(() => {});
      }
    } catch (caught) {
      setStatus(null);
      setError(toAppError(caught));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  async function update() {
    setUpdating(true);
    setError(null);
    const onEvent = new Channel();
    try {
      await invoke("install_agent", { agent: id, testHome: null, onEvent });
      setLatest(null);
      await refresh();
    } catch (caught) {
      setError(toAppError(caught));
    } finally {
      setUpdating(false);
    }
  }

  const updateAvailable =
    !!status?.installed &&
    !!status.version &&
    !!latest &&
    isNewer(latest, status.version);

  let statusText: string;
  let statusClass: string;
  if (!status) {
    statusText = t("home.agent.checking");
    statusClass = "";
  } else if (!status.installed) {
    statusText = t("home.agent.notInstalled");
    statusClass = "warn";
  } else if (!status.loggedIn) {
    statusText = t("home.agent.needLogin");
    statusClass = "warn";
  } else if (updateAvailable) {
    statusText = t("home.agent.updateAvailable", {
      version: semverParts(latest!)!.join("."),
    });
    statusClass = "update";
  } else {
    statusText = t("home.agent.installed", {
      version: shortVersion(status.version),
    });
    statusClass = "ok";
  }

  return (
    <div className="agent-row-block">
      <div className="agent-row">
        <div className="agent-row-info">
          <strong>{name}</strong>
          <span className={`agent-status ${statusClass}`}>{statusText}</span>
        </div>
        {status && !status.installed ? (
          <button className="ghost small" onClick={onSetup}>
            {t("home.agent.setup")}
          </button>
        ) : status && !status.loggedIn ? (
          <button className="ghost small" onClick={onSetup}>
            {t("home.agent.setup")}
          </button>
        ) : updateAvailable ? (
          <button className="primary small" onClick={update} disabled={updating}>
            {updating ? t("home.agent.updating") : t("home.agent.update")}
          </button>
        ) : null}
      </div>
      {error && <DoctorCard error={error} onRetry={status ? update : refresh} />}
    </div>
  );
}

function HomeView({
  projects,
  baseDir,
  scanError,
  onNew,
  onSetupAgent,
  onChangeBase,
  onChanged,
  onRetryScan,
}: {
  projects: HomeProject[];
  baseDir: string;
  scanError: AppError | null;
  onNew: () => void;
  onSetupAgent: (id: AgentId) => void;
  onChangeBase: () => void;
  onChanged: () => void;
  onRetryScan: () => void;
}) {
  const { t, lang } = useI18n();
  const [opening, setOpening] = useState<string | null>(null);

  async function open(p: HomeProject) {
    setOpening(p.path);
    try {
      await openPath(p.path);
      // 최근 사용 시각 기록(정렬용)
      await saveProject({ path: p.path, agent: p.agent, name: p.name });
      onChanged();
    } catch {
      onChanged();
    } finally {
      setOpening(null);
    }
  }

  const fmtWhen = (ts: number) =>
    new Intl.DateTimeFormat(lang === "ko" ? "ko-KR" : "en-US", {
      dateStyle: "medium",
    }).format(ts);

  return (
    <main className="panel">
      <section className="agents-panel">
        <h3>{t("home.agents.title")}</h3>
        {AGENTS.map((a) => (
          <AgentRow key={a.id} id={a.id} onSetup={() => onSetupAgent(a.id)} />
        ))}
      </section>

      <div className="home-head">
        <div>
          <h2>{t("home.title")}</h2>
          <p className="muted">{t("home.subtitle")}</p>
        </div>
        <button className="primary" onClick={onNew}>
          {t("home.new")}
        </button>
      </div>

      <div className="base-row">
        <span className="base-label">{t("home.base.label")}</span>
        <code className="base-path">{baseDir}</code>
        <button className="link" onClick={onChangeBase}>
          {t("home.base.change")}
        </button>
      </div>

      {scanError ? (
        <div className="home-scan-error">
          <DoctorCard error={scanError} onRetry={onRetryScan} />
          <button className="ghost" onClick={onChangeBase}>
            {t("home.base.chooseAnother")}
          </button>
        </div>
      ) : projects.length === 0 ? (
        <p className="muted home-empty">{t("home.empty")}</p>
      ) : (
        <ul className="project-list">
          {projects.map((p) => (
            <li key={p.path} className="project-card">
              <div className="project-info">
                <strong>{p.name}</strong>
                <span className="project-meta">
                  {t(`home.agent.${p.agent}` as MessageKey)}
                  {p.lastOpenedAt > 0
                    ? ` · ${t("home.lastOpened", { when: fmtWhen(p.lastOpenedAt) })}`
                    : ""}
                </span>
                <code className="project-path">{p.path}</code>
              </div>
              <div className="project-actions">
                <button
                  className="primary"
                  onClick={() => open(p)}
                  disabled={opening !== null}
                >
                  {opening === p.path ? t("home.opening") : t("home.open")}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

function AgentStep({
  selected,
  onSelect,
}: {
  selected: AgentId | null;
  onSelect: (id: AgentId) => void;
}) {
  const { t } = useI18n();
  return (
    <div>
      <h2>{t("agent.title")}</h2>
      <p className="muted">{t("agent.intro")}</p>
      <div className="agent-cards">
        {AGENTS.map((a) => (
          <button
            key={a.id}
            className={`agent-card ${selected === a.id ? "selected" : ""}`}
            onClick={() => onSelect(a.id)}
          >
            {a.recommended && <span className="badge">{t("agent.badge")}</span>}
            <strong>{t(`agent.${a.id}.name` as MessageKey)}</strong>
            <span className="agent-vendor">{a.vendor}</span>
            <span className="agent-desc">
              {t(`agent.${a.id}.desc` as MessageKey)}
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function DiagnosisStep({
  agent,
  report,
  onReport,
  onNext,
}: {
  agent: AgentId;
  report: EnvironmentReport | null;
  onReport: (r: EnvironmentReport) => void;
  onNext: () => void;
}) {
  const { t } = useI18n();
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const name = t(`agent.${agent}.name` as MessageKey);

  async function run() {
    setRunning(true);
    setError(null);
    try {
      onReport(await invoke<EnvironmentReport>("detect_environment", { agent }));
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setRunning(false);
    }
  }

  if (!report) {
    return (
      <div className="center">
        <h2>{t("diag.intro.title")}</h2>
        <p className="muted">{t("diag.intro.desc", { name })}</p>
        {error ? (
          <DoctorCard error={error} onRetry={run} retryLabel={t("diag.run")} />
        ) : (
          <button className="primary" onClick={run} disabled={running}>
            {running ? t("diag.checking") : t("diag.run")}
          </button>
        )}
      </div>
    );
  }

  const tool = report.agent;
  return (
    <div>
      <h2>{t("diag.done.title")}</h2>
      <ul className="results">
        <li className="result ok">
          <span className="result-icon">💻</span>
          <div>
            <strong>{osLabel(report)}</strong>
            <div className="muted">{t("diag.os.desc")}</div>
          </div>
        </li>
        <li className={`result ${tool ? "ok" : "todo"}`}>
          <span className="result-icon">{tool ? "✅" : "📦"}</span>
          <div>
            <strong>
              {tool
                ? t("diag.agent.installed", {
                    name,
                    version: tool.version.split(" ")[0],
                  })
                : t("diag.agent.missing", { name })}
            </strong>
            <div className="muted">
              {tool
                ? tool.inShellPath
                  ? t("diag.agent.inpath")
                  : t("diag.agent.notinpath")
                : t("diag.agent.willinstall")}
            </div>
          </div>
        </li>
        <li className="result ok">
          <span className="result-icon">{report.node ? "🟢" : "⚪️"}</span>
          <div>
            <strong>
              {report.node
                ? t("diag.node.has", { version: report.node.version })
                : t("diag.node.none")}
            </strong>
            <div className="muted">
              {report.node ? t("diag.node.hasdesc") : t("diag.node.nonedesc")}
            </div>
          </div>
        </li>
      </ul>
      <div className="actions">
        <button className="ghost" onClick={run} disabled={running}>
          {t("diag.recheck")}
        </button>
        <button className="primary" onClick={onNext}>
          {tool ? t("diag.next.skip") : t("diag.next.install")}
        </button>
      </div>
    </div>
  );
}

function InstallStep({
  agent,
  report,
  onNext,
}: {
  agent: AgentId;
  report: EnvironmentReport | null;
  onNext: () => void;
}) {
  const { t } = useI18n();
  const [phase, setPhase] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [downloaded, setDownloaded] = useState(0);
  const [result, setResult] = useState<InstallResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const running = phase !== null && !result && !error;
  const name = t(`agent.${agent}.name` as MessageKey);

  if (report?.agent && !result) {
    return (
      <div className="center">
        <h2>{t("install.already.title")}</h2>
        <p className="muted">
          {t("install.already.desc", {
            name,
            version: report.agent.version.split(" ")[0],
          })}
        </p>
        <button className="primary" onClick={onNext}>
          {t("install.next")}
        </button>
      </div>
    );
  }

  async function start() {
    setError(null);
    setLog([]);
    setDownloaded(0);
    setResult(null);
    setPhase("download");
    const onEvent = new Channel<InstallEvent>();
    onEvent.onmessage = (e) => {
      if (e.type === "phase") setPhase(e.name);
      else if (e.type === "progress") setDownloaded(e.downloadedBytes);
      else setLog((prev) => [...prev.slice(-199), e.line]);
    };
    try {
      setResult(
        await invoke<InstallResult>("install_agent", {
          agent,
          testHome: null,
          onEvent,
        }),
      );
    } catch (err) {
      setError(toAppError(err));
    }
  }

  if (result) {
    return (
      <div className="center">
        <h2>{t("install.done.title")}</h2>
        <p className="muted">
          {t("install.done.desc", {
            name,
            version: result.version.split(" ")[0],
          })}
          <br />
          {result.profileUpdated
            ? t("install.done.path")
            : t("install.done.nopath")}
        </p>
        <button className="primary" onClick={onNext}>
          {t("install.next")}
        </button>
      </div>
    );
  }

  if (running) {
    const currentIndex = INSTALL_PHASE_IDS.findIndex((p) => p === phase);
    return (
      <div>
        <h2>{t("install.running.title")}</h2>
        <p className="muted">{t("install.running.desc")}</p>
        <ul className="phases">
          {INSTALL_PHASE_IDS.map((id, i) => (
            <li
              key={id}
              className={
                i < currentIndex ? "done" : i === currentIndex ? "active" : ""
              }
            >
              <span className="phase-mark">
                {i < currentIndex ? "✓" : i === currentIndex ? "…" : "•"}
              </span>
              {t(`install.phase.${id}` as MessageKey)}
              {(id === "download" || id === "install") &&
                i === currentIndex &&
                downloaded > 0 && (
                  <span className="phase-detail">
                    {t("install.mb", {
                      mb: Math.round(downloaded / 1024 / 1024),
                    })}
                  </span>
                )}
            </li>
          ))}
        </ul>
        <div className="progress-track">
          <div className="progress-fill" />
        </div>
        {log.length > 0 && (
          <details className="log">
            <summary>{t("install.log.detail")}</summary>
            <pre>{log.join("\n")}</pre>
          </details>
        )}
      </div>
    );
  }

  if (error) {
    return (
      <div className="center">
        <h2>{t("install.fail.title")}</h2>
        <p className="muted">{t("install.fail.desc")}</p>
        <DoctorCard
          error={error}
          log={log}
          onRetry={start}
          retryLabel={t("install.retry")}
        />
      </div>
    );
  }

  return (
    <div className="center">
      <h2>{t("install.start.title", { name })}</h2>
      <p className="muted">{t("install.start.desc")}</p>
      <button className="primary" onClick={start}>
        {t("install.start.btn")}
      </button>
    </div>
  );
}

function LoginStep({ agent, onNext }: { agent: AgentId; onNext: () => void }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<LoginStatus | null>(null);
  const [checking, setChecking] = useState(true);
  const [mode, setMode] = useState<"idle" | "waiting" | "verifying">("idle");
  const [url, setUrl] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const isClaude = agent === "claude-code";
  const name = t(`agent.${agent}.name` as MessageKey);
  const service = t(isClaude ? "login.service.claude" : "login.service.codex");

  async function check() {
    setChecking(true);
    try {
      setStatus(await invoke<LoginStatus>("login_status", { agent }));
      setError(null);
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setChecking(false);
    }
  }

  useEffect(() => {
    check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [agent]);

  // 브라우저에 이미 세션이 있으면 코드 없이 자동 승인으로 끝나는 경로가 있으므로,
  // 기다리는 동안 로그인 완료를 폴링으로 감지한다
  useEffect(() => {
    if (mode !== "waiting") return;
    const timer = setInterval(async () => {
      try {
        const s = await invoke<LoginStatus>("login_status", { agent });
        if (s.loggedIn) {
          await invoke("cancel_login").catch(() => {});
          setStatus(s);
          setError(null);
          setMode("idle");
        }
      } catch {
        // 일시적 확인 실패는 무시하고 다음 폴링을 기다린다
      }
    }, 3000);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  async function openSession(useApiBilling: boolean) {
    setUrl(null);
    const onEvent = new Channel<LoginEvent>();
    onEvent.onmessage = (e) => {
      if (e.type === "url") setUrl(e.url);
    };
    await invoke("start_login", { agent, useApiBilling, onEvent });
  }

  async function start(useApiBilling: boolean) {
    setError(null);
    setCode("");
    setMode("waiting");
    try {
      await openSession(useApiBilling);
    } catch (err) {
      setError(toAppError(err));
      setMode("idle");
    }
  }

  async function submit() {
    if (!code.trim()) return;
    setMode("verifying");
    setError(null);
    const onEvent = new Channel<LoginEvent>();
    try {
      await invoke("submit_login_code", { agent, onEvent, code });
      await check();
      setMode("idle");
    } catch {
      setCode("");
      try {
        await openSession(false);
        setError({ kind: "generic", detail: t("login.codeFail") });
        setMode("waiting");
      } catch (err2) {
        setError(toAppError(err2));
        setMode("idle");
      }
    }
  }

  async function cancel() {
    await invoke("cancel_login").catch(() => {});
    setMode("idle");
    setUrl(null);
  }

  if (checking) {
    return (
      <div className="center">
        <h2>{t("login.checking")}</h2>
      </div>
    );
  }

  if (status?.loggedIn) {
    const plan = status.subscriptionType
      ? KNOWN_PLANS.includes(status.subscriptionType)
        ? t(`plan.${status.subscriptionType}` as MessageKey)
        : status.subscriptionType
      : null;
    const account = t(isClaude ? "login.account.claude" : "login.account.codex");
    return (
      <div className="center">
        <h2>{t("login.done.title")}</h2>
        <p className="muted">
          {status.email ?? account}
          {plan ? ` · ${plan}` : ""}
          <br />
          {t("login.done.ready", { name })}
        </p>
        <button className="primary" onClick={onNext}>
          {t("login.next")}
        </button>
      </div>
    );
  }

  if (mode === "verifying") {
    return (
      <div className="center">
        <h2>{t("login.verifying.title")}</h2>
        <p className="muted">{t("login.verifying.desc")}</p>
      </div>
    );
  }

  if (mode === "waiting") {
    return (
      <div>
        <h2>{t("login.waiting.title")}</h2>
        <ol className="guide">
          <li>{t("login.waiting.step1", { service })}</li>
          <li>{t("login.waiting.step2")}</li>
          {isClaude && <li>{t("login.waiting.step3")}</li>}
        </ol>
        {url && (
          <p className="muted">
            {t("login.browserFail.pre")}
            <button className="link" onClick={() => openUrl(url)}>
              {t("login.browserFail.link")}
            </button>
          </p>
        )}
        {error && <DoctorCard error={error} onRetry={() => start(false)} />}
        {isClaude && (
          <div className="code-row">
            <input
              className="code-input"
              placeholder={t("login.code.placeholder")}
              value={code}
              onChange={(e) => setCode(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
            />
            <button className="primary" onClick={submit} disabled={!code.trim()}>
              {t("login.code.confirm")}
            </button>
          </div>
        )}
        <div className="actions">
          <button className="ghost" onClick={cancel}>
            {t("login.restart")}
          </button>
        </div>
      </div>
    );
  }

  // 로그인 방식 선택 + 요금 개념 안내 (M2 — 삽질 분류 A·E)
  return (
    <div>
      <h2>{t("login.method.title", { service })}</h2>
      <p className="muted">{t("login.method.desc")}</p>
      {isClaude ? (
        <>
          <div className="method-box">
            <strong>{t("login.claude.box.title")}</strong>
            <p>{t("login.claude.box.desc")}</p>
          </div>
          <p className="hint">{t("login.claude.hint")}</p>
          {error && <DoctorCard error={error} onRetry={() => start(false)} />}
          <div className="actions">
            <details className="advanced">
              <summary>{t("login.claude.advanced")}</summary>
              <p className="muted">{t("login.claude.advanced.desc")}</p>
              <button className="ghost" onClick={() => start(true)}>
                {t("login.claude.api.btn")}
              </button>
            </details>
            <button className="primary" onClick={() => start(false)}>
              {t("login.claude.sub.btn")}
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="method-box">
            <strong>{t("login.codex.box.title")}</strong>
            <p>{t("login.codex.box.desc")}</p>
          </div>
          {error && <DoctorCard error={error} onRetry={() => start(false)} />}
          <div className="actions">
            <span />
            <button className="primary" onClick={() => start(false)}>
              {t("login.codex.btn")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}

function ProjectStep({
  agent,
  project,
  onProject,
  onNext,
}: {
  agent: AgentId;
  project: ProjectInfo | null;
  onProject: (p: ProjectInfo) => void;
  onNext: () => void;
}) {
  const { t, lang } = useI18n();
  const [name, setName] = useState("my-first-project");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  async function create() {
    setCreating(true);
    setError(null);
    try {
      const base = (await getBaseDir()) ?? undefined;
      const info = await invoke<ProjectInfo>("create_first_project", {
        agent,
        name,
        base,
        language: lang,
      });
      await saveProject({ path: info.path, agent, name });
      onProject(info);
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setCreating(false);
    }
  }

  if (project) {
    return (
      <div className="center">
        <h2>{t("project.done.title")}</h2>
        <p className="muted">
          {project.created ? t("project.done.created") : t("project.done.reuse")}
          <br />
          {t("project.done.safe")}
        </p>
        <code className="path-box">{project.path}</code>
        <button className="primary" onClick={onNext}>
          {t("project.next")}
        </button>
      </div>
    );
  }

  return (
    <div className="center">
      <h2>{t("project.make.title")}</h2>
      <p className="muted">{t("project.make.desc")}</p>
      {error && <DoctorCard error={error} onRetry={create} />}
      <div className="code-row">
        <input
          className="code-input"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
          placeholder={t("project.folder.placeholder")}
        />
        <button
          className="primary"
          onClick={create}
          disabled={creating || !name.trim()}
        >
          {creating ? t("project.making") : t("project.make.btn")}
        </button>
      </div>
    </div>
  );
}

function GraduationStep({
  agent,
  project,
  onHome,
}: {
  agent: AgentId;
  project: ProjectInfo | null;
  onHome: () => void;
}) {
  const { t, lang } = useI18n();
  const [running, setRunning] = useState(false);
  const [reply, setReply] = useState<string | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [editors, setEditors] = useState<EditorInfo[]>([]);
  const [opening, setOpening] = useState<string | null>(null);
  const name = t(`agent.${agent}.name` as MessageKey);

  async function refreshEditors() {
    try {
      setEditors(await invoke<EditorInfo[]>("detect_editors"));
    } catch {
      // 감지 실패는 무시 — 다음 확인 때 다시 시도한다
    }
  }

  useEffect(() => {
    refreshEditors();
    // 사용자가 편집기를 설치하고 앱으로 돌아오면(창 포커스) 자동으로 다시 감지한다
    window.addEventListener("focus", refreshEditors);
    return () => window.removeEventListener("focus", refreshEditors);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function openInEditor(id: string) {
    setOpening(id);
    try {
      await invoke("open_in_editor", { editor: id, agent, path: project!.path });
    } catch {
      // 실패해도 조용히 — 사용자는 폴더 열기 버튼으로 대신할 수 있다
    } finally {
      setOpening(null);
    }
  }

  if (!project) {
    return (
      <div className="center">
        <h2>{t("grad.noproject")}</h2>
      </div>
    );
  }

  const installed = editors.filter((e) => e.installed);

  async function chat() {
    setRunning(true);
    setError(null);
    try {
      setReply(
        await invoke<string>("run_first_chat", {
          agent,
          projectPath: project!.path,
          language: lang,
        }),
      );
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setRunning(false);
    }
  }

  if (reply) {
    const fallback = [
      { id: "cursor", url: "https://cursor.com" },
      { id: "vscode", url: "https://code.visualstudio.com" },
    ];
    return (
      <div className="center">
        <div className="confetti" aria-hidden="true">
          {Array.from({ length: 12 }, (_, i) => (
            <i key={i} />
          ))}
        </div>
        <h2>{t("grad.done.title")}</h2>
        <div className="chat-bubble">
          <span className="chat-name">{name}</span>
          {reply}
        </div>
        <p className="muted">{t("grad.chatted", { name })}</p>
        <button className="primary" onClick={() => openPath(project.path)}>
          {t("grad.openFolder")}
        </button>

        <div className="next-guide">
          <strong>{t("grad.next.title")}</strong>
          {installed.length > 0 ? (
            <>
              <p>{t("grad.editor.installed.desc", { name })}</p>
              <div className="editor-btns">
                {installed.map((e) => (
                  <button
                    key={e.id}
                    className="ghost"
                    disabled={opening !== null}
                    onClick={() => openInEditor(e.id)}
                  >
                    {opening === e.id
                      ? t("grad.editor.preparing")
                      : t("grad.editor.open", { editor: editorLabel(t, e.id) })}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <>
              <p>{t("grad.editor.none.desc")}</p>
              <div className="editor-btns">
                {(editors.length > 0 ? editors : fallback).map((e) => (
                  <button
                    key={e.id}
                    className="ghost"
                    onClick={() => openUrl(e.url)}
                  >
                    {t("grad.editor.get", { editor: editorLabel(t, e.id) })}
                  </button>
                ))}
              </div>
              <button className="link recheck" onClick={refreshEditors}>
                {t("grad.editor.recheck")}
              </button>
            </>
          )}
          <p className="hint">{t("grad.reopen")}</p>
        </div>

        <button className="link home-link" onClick={onHome}>
          {t("home.title")} →
        </button>
      </div>
    );
  }

  if (running) {
    return (
      <div className="center">
        <h2>{t("grad.writing.title", { name })}</h2>
        <p className="muted">{t("grad.writing.desc")}</p>
      </div>
    );
  }

  return (
    <div className="center">
      <h2>{t("grad.last.title")}</h2>
      <p className="muted">{t("grad.last.desc", { name })}</p>
      {error ? (
        <DoctorCard
          error={error}
          onRetry={chat}
          retryLabel={t("grad.sayHi", { name })}
        />
      ) : (
        <button className="primary" onClick={chat}>
          {t("grad.sayHi", { name })}
        </button>
      )}
    </div>
  );
}

export default App;
