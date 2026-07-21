import { useEffect, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
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

type AgentId = "claude-code" | "codex";

const AGENTS: {
  id: AgentId;
  name: string;
  vendor: string;
  desc: string;
  badge?: string;
}[] = [
  {
    id: "claude-code",
    name: "클로드 코드",
    vendor: "Anthropic",
    desc: "클로드 구독(Pro 등)으로 사용해요. 이 도우미가 가장 꼼꼼하게 챙겨주는 에이전트예요.",
    badge: "추천",
  },
  {
    id: "codex",
    name: "코덱스",
    vendor: "OpenAI",
    desc: "ChatGPT 계정(Plus 등)으로 사용해요. ChatGPT를 이미 구독 중이라면 이쪽이 편해요.",
  },
];

const agentName = (id: AgentId) =>
  AGENTS.find((a) => a.id === id)?.name ?? id;

const STEPS = ["에이전트", "진단", "설치", "로그인", "첫 프로젝트", "졸업식"] as const;

const INSTALL_PHASES = [
  { id: "download", label: "설치 파일 내려받기" },
  { id: "install", label: "프로그램 설치하기" },
  { id: "path", label: "터미널 설정 정리하기" },
  { id: "verify", label: "잘 됐는지 확인하기" },
] as const;

const PLAN_LABELS: Record<string, string> = {
  pro: "Pro 요금제",
  max: "Max 요금제",
  team: "Team 요금제",
  enterprise: "Enterprise 요금제",
};

function osLabel(report: EnvironmentReport): string {
  if (report.os === "macos") {
    const chip = report.arch === "aarch64" ? "Apple Silicon" : "Intel";
    const ver = report.osVersion ? ` ${report.osVersion}` : "";
    return `macOS${ver} (${chip})`;
  }
  if (report.os === "windows") return "Windows";
  return `${report.os} (${report.arch})`;
}

function App() {
  const [step, setStep] = useState(0);
  const [agent, setAgent] = useState<AgentId | null>(null);
  const [report, setReport] = useState<EnvironmentReport | null>(null);
  const [project, setProject] = useState<ProjectInfo | null>(null);

  function selectAgent(id: AgentId) {
    setAgent(id);
    setReport(null);
    setProject(null);
    setStep(1);
  }

  return (
    <div className="app">
      <header className="header">
        <p className="eyebrow">코딩 에이전트 시작 도우미</p>
        <h1>Agent Starter</h1>
        <p className="tagline">
          코딩 에이전트를 쓸 수 있는 상태까지, 차근차근 같이 갈게요.
        </p>
      </header>

      <ol className="steps">
        {STEPS.map((name, i) => (
          <li
            key={name}
            className={i === step ? "active" : i < step ? "done" : ""}
          >
            <span className="step-dot">{i < step ? "✓" : i + 1}</span>
            {name}
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
          <InstallStep agent={agent} report={report} onNext={() => setStep(3)} />
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
          <GraduationStep agent={agent} project={project} />
        )}
      </main>
    </div>
  );
}

function AgentStep({
  selected,
  onSelect,
}: {
  selected: AgentId | null;
  onSelect: (id: AgentId) => void;
}) {
  return (
    <div>
      <h2>어떤 코딩 에이전트를 시작할까요?</h2>
      <p className="muted">
        코딩 에이전트는 말로 시키면 대신 일해 주는 프로그램이에요.
        <br />
        이미 쓰고 있는 구독에 맞춰 고르면 돼요. 나중에 바꿀 수도 있어요.
      </p>
      <div className="agent-cards">
        {AGENTS.map((a) => (
          <button
            key={a.id}
            className={`agent-card ${selected === a.id ? "selected" : ""}`}
            onClick={() => onSelect(a.id)}
          >
            {a.badge && <span className="badge">{a.badge}</span>}
            <strong>{a.name}</strong>
            <span className="agent-vendor">{a.vendor}</span>
            <span className="agent-desc">{a.desc}</span>
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
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const name = agentName(agent);

  async function run() {
    setRunning(true);
    setError(null);
    try {
      onReport(await invoke<EnvironmentReport>("detect_environment", { agent }));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  if (!report) {
    return (
      <div className="center">
        <h2>먼저 내 컴퓨터 상태를 확인할게요</h2>
        <p className="muted">
          어떤 컴퓨터인지, {name}가 이미 설치되어 있는지 살펴봐요.
          <br />
          컴퓨터의 파일을 바꾸지 않으니 안심하세요.
        </p>
        {error && <p className="error">확인 중 문제가 생겼어요: {error}</p>}
        <button className="primary" onClick={run} disabled={running}>
          {running ? "확인하는 중…" : "내 컴퓨터 확인하기"}
        </button>
      </div>
    );
  }

  const tool = report.agent;
  return (
    <div>
      <h2>확인이 끝났어요</h2>
      <ul className="results">
        <li className="result ok">
          <span className="result-icon">💻</span>
          <div>
            <strong>{osLabel(report)}</strong>
            <div className="muted">이 컴퓨터에서 사용할 수 있어요.</div>
          </div>
        </li>
        <li className={`result ${tool ? "ok" : "todo"}`}>
          <span className="result-icon">{tool ? "✅" : "📦"}</span>
          <div>
            <strong>
              {tool
                ? `${name}가 이미 설치되어 있어요 (버전 ${tool.version.split(" ")[0]})`
                : `${name}가 아직 없어요`}
            </strong>
            <div className="muted">
              {tool
                ? tool.inShellPath
                  ? "터미널에서도 바로 쓸 수 있는 상태예요."
                  : "설치는 되어 있지만 터미널이 아직 위치를 몰라요. 나중에 자동으로 잡아드릴게요."
                : "다음 단계에서 자동으로 설치해 드릴게요."}
            </div>
          </div>
        </li>
        <li className="result ok">
          <span className="result-icon">{report.node ? "🟢" : "⚪️"}</span>
          <div>
            <strong>
              Node.js {report.node ? `${report.node.version} 있음` : "없음"}
            </strong>
            <div className="muted">
              {report.node
                ? "부가 도구를 쓸 때 도움이 돼요."
                : "없어도 에이전트 사용에는 문제 없어요."}
            </div>
          </div>
        </li>
      </ul>
      <div className="actions">
        <button className="ghost" onClick={run} disabled={running}>
          다시 확인
        </button>
        <button className="primary" onClick={onNext}>
          {tool ? "설치는 건너뛰고 다음으로" : "다음: 설치하러 가기"}
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
  const [phase, setPhase] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [downloaded, setDownloaded] = useState(0);
  const [result, setResult] = useState<InstallResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const running = phase !== null && !result && !error;
  const name = agentName(agent);

  if (report?.agent && !result) {
    return (
      <div className="center">
        <h2>이미 설치되어 있어요 ✅</h2>
        <p className="muted">
          {name} {report.agent.version.split(" ")[0]} 버전이 컴퓨터에 있어요.
          <br />이 단계는 할 일이 없으니 바로 넘어갈게요.
        </p>
        <button className="primary" onClick={onNext}>
          다음: 로그인 확인하러 가기
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
      setError(String(err));
    }
  }

  if (result) {
    return (
      <div className="center">
        <h2>설치가 끝났어요 🎉</h2>
        <p className="muted">
          {name} {result.version.split(" ")[0]} 버전이 설치됐어요.
          <br />
          {result.profileUpdated
            ? "터미널에서도 바로 쓸 수 있게 설정까지 마쳤어요."
            : "터미널 설정은 이미 되어 있어서 그대로 뒀어요."}
        </p>
        <button className="primary" onClick={onNext}>
          다음: 로그인 확인하러 가기
        </button>
      </div>
    );
  }

  if (running) {
    const currentIndex = INSTALL_PHASES.findIndex((p) => p.id === phase);
    return (
      <div>
        <h2>설치하고 있어요</h2>
        <p className="muted">
          컴퓨터와 인터넷 속도에 따라 1~5분 정도 걸려요. 창을 닫지 말고 기다려
          주세요.
        </p>
        <ul className="phases">
          {INSTALL_PHASES.map((p, i) => (
            <li
              key={p.id}
              className={
                i < currentIndex ? "done" : i === currentIndex ? "active" : ""
              }
            >
              <span className="phase-mark">
                {i < currentIndex ? "✓" : i === currentIndex ? "…" : "•"}
              </span>
              {p.label}
              {p.id === "download" && i === currentIndex && downloaded > 0 && (
                <span className="phase-detail">
                  {Math.round(downloaded / 1024 / 1024)}MB 받았어요
                </span>
              )}
              {p.id === "install" && i === currentIndex && downloaded > 0 && (
                <span className="phase-detail">
                  {Math.round(downloaded / 1024 / 1024)}MB 받았어요
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
            <summary>자세한 진행 내용 보기</summary>
            <pre>{log.join("\n")}</pre>
          </details>
        )}
      </div>
    );
  }

  return (
    <div className="center">
      <h2>{error ? "설치가 잘 안 됐어요" : `${name}를 설치할게요`}</h2>
      <p className="muted">
        {error ? (
          "괜찮아요, 다시 시도하면 돼요. 보통은 인터넷 연결 문제예요."
        ) : (
          <>
            버튼 하나만 누르면 다운로드부터 터미널 설정까지 알아서 진행돼요.
            <br />
            공식 배포처의 설치 파일만 사용하니 안심하세요.
          </>
        )}
      </p>
      {error && <p className="error">{error}</p>}
      <button className="primary" onClick={start}>
        {error ? "다시 설치하기" : "설치 시작하기"}
      </button>
      {error && log.length > 0 && (
        <details className="log">
          <summary>무슨 일이 있었는지 보기</summary>
          <pre>{log.join("\n")}</pre>
        </details>
      )}
    </div>
  );
}

function LoginStep({ agent, onNext }: { agent: AgentId; onNext: () => void }) {
  const [status, setStatus] = useState<LoginStatus | null>(null);
  const [checking, setChecking] = useState(true);
  const [mode, setMode] = useState<"idle" | "waiting" | "verifying">("idle");
  const [url, setUrl] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const isClaude = agent === "claude-code";
  const name = agentName(agent);

  async function check() {
    setChecking(true);
    try {
      setStatus(await invoke<LoginStatus>("login_status", { agent }));
      setError(null);
    } catch (e) {
      setError(String(e));
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
      setError(String(err));
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
        setError(
          "코드가 맞지 않았어요. 브라우저 창이 다시 열리니 새로 로그인하고, 새 코드를 붙여넣어 주세요.",
        );
        setMode("waiting");
      } catch (err2) {
        setError(String(err2));
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
        <h2>로그인 상태를 확인하고 있어요…</h2>
      </div>
    );
  }

  if (status?.loggedIn) {
    const plan = status.subscriptionType
      ? (PLAN_LABELS[status.subscriptionType] ?? status.subscriptionType)
      : null;
    return (
      <div className="center">
        <h2>로그인되어 있어요 ✅</h2>
        <p className="muted">
          {status.email ?? (isClaude ? "클로드 계정" : "ChatGPT 계정")}
          {plan ? ` · ${plan}` : ""}
          <br />
          {name}를 쓸 준비가 됐어요.
        </p>
        <button className="primary" onClick={onNext}>
          다음: 첫 프로젝트 만들기
        </button>
      </div>
    );
  }

  if (mode === "verifying") {
    return (
      <div className="center">
        <h2>코드를 확인하고 있어요…</h2>
        <p className="muted">잠시만 기다려 주세요.</p>
      </div>
    );
  }

  if (mode === "waiting") {
    return (
      <div>
        <h2>브라우저에서 로그인해 주세요</h2>
        <ol className="guide">
          <li>
            방금 열린 브라우저 창에서{" "}
            {isClaude ? "클로드" : "ChatGPT"} 계정으로 로그인해요.
          </li>
          <li>
            로그인이 끝나면 <strong>이 화면이 알아서 다음으로 넘어가요.</strong>
          </li>
          {isClaude && (
            <li>
              브라우저에 <strong>확인 코드</strong>가 보이는 경우에만, 복사해서
              아래 칸에 붙여넣어 주세요.
            </li>
          )}
        </ol>
        {url && (
          <p className="muted">
            브라우저가 안 열렸다면{" "}
            <button className="link" onClick={() => openUrl(url)}>
              여기를 눌러 주세요
            </button>
          </p>
        )}
        {error && <p className="error">{error}</p>}
        {isClaude && (
          <div className="code-row">
            <input
              className="code-input"
              placeholder="확인 코드 붙여넣기 (코드가 보일 때만)"
              value={code}
              onChange={(e) => setCode(e.currentTarget.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
            />
            <button className="primary" onClick={submit} disabled={!code.trim()}>
              확인
            </button>
          </div>
        )}
        <div className="actions">
          <button className="ghost" onClick={cancel}>
            처음부터 다시
          </button>
        </div>
      </div>
    );
  }

  // 로그인 방식 선택 + 요금 개념 안내 (M2 — 삽질 분류 A·E)
  return (
    <div>
      <h2>{isClaude ? "클로드" : "ChatGPT"} 계정으로 로그인할게요</h2>
      <p className="muted">
        버튼을 누르면 브라우저가 열려요. 비밀번호는 이 앱이 아니라 공식
        사이트에만 입력해요.
      </p>
      {isClaude ? (
        <>
          <div className="method-box">
            <strong>구독으로 쓰기 (추천)</strong>
            <p>
              클로드 Pro/Max 구독이 있다면 이걸 선택하세요. 매달 내는 구독료
              안에서 사용하고, 카드가 따로 청구되지 않아요.
            </p>
          </div>
          <p className="hint">
            참고: 에이전트는 채팅보다 사용량을 많이 써요. 한도에 닿으면 잠시
            쉬었다가 다시 쓸 수 있어요.
          </p>
          {error && <p className="error">{error}</p>}
          <div className="actions">
            <details className="advanced">
              <summary>다른 방식 (API 과금)</summary>
              <p className="muted">
                쓴 만큼 카드로 청구되는 개발자용 방식이에요. 잘 모르겠다면
                구독을 선택하세요.
              </p>
              <button className="ghost" onClick={() => start(true)}>
                API 방식으로 로그인
              </button>
            </details>
            <button className="primary" onClick={() => start(false)}>
              구독으로 로그인 (추천)
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="method-box">
            <strong>ChatGPT 계정으로 쓰기</strong>
            <p>
              ChatGPT Plus 이상 구독이면 코덱스를 바로 쓸 수 있어요. 구독료
              안에서 사용해요.
            </p>
          </div>
          {error && <p className="error">{error}</p>}
          <div className="actions">
            <span />
            <button className="primary" onClick={() => start(false)}>
              브라우저로 로그인 시작
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
  const [name, setName] = useState("내-첫-프로젝트");
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function create() {
    setCreating(true);
    setError(null);
    try {
      onProject(
        await invoke<ProjectInfo>("create_first_project", { agent, name }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  }

  if (project) {
    return (
      <div className="center">
        <h2>폴더가 준비됐어요 📁</h2>
        <p className="muted">
          {project.created
            ? "문서 폴더 안에 새 폴더를 만들었어요."
            : "이미 있던 폴더를 그대로 쓸게요."}
          <br />
          안전장치와 함께, 다음에 혼자 이어갈 수 있게 <strong>시작하기.md</strong>{" "}
          안내 문서도 넣어뒀어요.
        </p>
        <code className="path-box">{project.path}</code>
        <button className="primary" onClick={onNext}>
          다음: 첫 인사 나누기
        </button>
      </div>
    );
  }

  return (
    <div className="center">
      <h2>작업할 폴더를 만들게요</h2>
      <p className="muted">
        에이전트가 일할 전용 폴더예요. 문서 폴더 안에 만들어지고,
        <br />
        다른 파일은 건드리지 않으니 안전해요.
      </p>
      {error && <p className="error">{error}</p>}
      <div className="code-row">
        <input
          className="code-input"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
          placeholder="폴더 이름"
        />
        <button
          className="primary"
          onClick={create}
          disabled={creating || !name.trim()}
        >
          {creating ? "만드는 중…" : "폴더 만들기"}
        </button>
      </div>
    </div>
  );
}

function GraduationStep({
  agent,
  project,
}: {
  agent: AgentId;
  project: ProjectInfo | null;
}) {
  const [running, setRunning] = useState(false);
  const [reply, setReply] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const name = agentName(agent);

  if (!project) {
    return (
      <div className="center">
        <h2>먼저 이전 단계에서 폴더를 만들어 주세요</h2>
      </div>
    );
  }

  async function chat() {
    setRunning(true);
    setError(null);
    try {
      setReply(
        await invoke<string>("run_first_chat", {
          agent,
          projectPath: project!.path,
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  if (reply) {
    return (
      <div className="center">
        <div className="confetti" aria-hidden="true">
          {Array.from({ length: 12 }, (_, i) => (
            <i key={i} />
          ))}
        </div>
        <h2>축하해요, 모든 준비가 끝났어요 🎓</h2>
        <div className="chat-bubble">
          <span className="chat-name">{name}</span>
          {reply}
        </div>
        <p className="muted">
          방금 {name}와 첫 대화를 나눴어요. 이제 진짜예요.
          <br />
          아래 버튼으로 폴더를 열어 두면, 다음에 쓸 때 찾기 쉬워요.
        </p>
        <button className="primary" onClick={() => openPath(project.path)}>
          내 프로젝트 폴더 열기
        </button>
      </div>
    );
  }

  if (running) {
    return (
      <div className="center">
        <h2>{name}가 대답을 쓰고 있어요…</h2>
        <p className="muted">첫 만남이라 몇 초 정도 걸려요.</p>
      </div>
    );
  }

  return (
    <div className="center">
      <h2>마지막 단계예요 — 첫 인사를 나눠 봐요</h2>
      <p className="muted">
        버튼을 누르면 {name}에게 인사를 건네고, 대답이 여기 표시돼요.
      </p>
      {error && <p className="error">{error}</p>}
      <button className="primary" onClick={chat}>
        {name}에게 인사 보내기 👋
      </button>
    </div>
  );
}

export default App;
