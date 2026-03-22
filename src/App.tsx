import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Session {
  session_id: string;
  pid: number;
  cwd: string;
  project_name: string;
  started_at: number;
  status: "Running" | "Waiting" | "Done" | "Error";
  topic: string;
  last_message: string;
  current_task: string;
  model: string;
  tokens_in: number;
  tokens_out: number;
  last_context_used: number;
  estimated_cost: number;
  elapsed_seconds: number;
  status_since_seconds: number;
}

function formatElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h${m}m`;
}

function formatTokens(n: number): string {
  if (n < 1000) return `${n}`;
  if (n < 1_000_000) return `${(n / 1000).toFixed(0)}K`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

function shortModel(model: string): string {
  if (model.includes("opus")) return "Op";
  if (model.includes("sonnet")) return "So";
  if (model.includes("haiku")) return "Ha";
  if (!model) return "-";
  return "?";
}

function getContextLimit(model: string): number {
  if (model.includes("opus")) return 1_000_000;
  if (model.includes("sonnet")) return 1_000_000;
  if (model.includes("haiku")) return 200_000;
  return 200_000;
}

function contextUsedPercent(model: string, lastContextUsed: number): number {
  const limit = getContextLimit(model);
  if (limit === 0 || lastContextUsed === 0) return 0;
  return Math.min(100, Math.round((lastContextUsed / limit) * 100));
}

function contextBarClass(percent: number): string {
  if (percent < 50) return "low";
  if (percent < 80) return "medium";
  return "high";
}

function statusIcon(status: string): string {
  switch (status) {
    case "Running": return "▶";
    case "Waiting": return "⏸";
    case "Done": return "✓";
    case "Error": return "✕";
    default: return "?";
  }
}

function statusText(status: string): string {
  switch (status) {
    case "Running": return "実行中";
    case "Waiting": return "入力待ち";
    case "Done": return "完了";
    case "Error": return "エラー";
    default: return status;
  }
}

function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [compact, setCompact] = useState(true);

  useEffect(() => {
    invoke<Session[]>("get_sessions").then(setSessions);
    const unlisten = listen<Session[]>("sessions-updated", (event) => {
      setSessions(event.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const active = sessions.filter((s) => s.status === "Running").length;
  const waiting = sessions.filter((s) => s.status === "Waiting").length;
  const done = sessions.filter((s) => s.status === "Done").length;
  const totalCost = sessions.reduce((sum, s) => sum + s.estimated_cost, 0);

  return (
    <div className="app">
      <div className="header">
        <h1>SessionDock</h1>
        <div className="header-right">
          <button className="view-toggle" onClick={() => setCompact(!compact)}>
            {compact ? "詳細" : "簡易"}
          </button>
          <div className="status-badges">
            {active > 0 && <span className="badge active">▶{active}</span>}
            {waiting > 0 && <span className="badge waiting">⏸{waiting}</span>}
            {done > 0 && <span className="badge done">✓{done}</span>}
          </div>
        </div>
      </div>

      {sessions.length === 0 ? (
        <div className="empty-state">
          <h2>No sessions</h2>
          <p>Claude Codeを起動してください</p>
        </div>
      ) : (
        <div className="sessions">
          {sessions.map((s) => {
            const usedPct = contextUsedPercent(s.model, s.last_context_used);
            const remainPct = 100 - usedPct;
            const ctxLimit = getContextLimit(s.model);

            if (compact) {
              return (
                <div key={s.session_id} className={`session-compact ${s.status.toLowerCase()}`}>
                  <div className="compact-row">
                    <span className={`compact-status ${s.status.toLowerCase()}`}>
                      {statusIcon(s.status)}
                    </span>
                    <span className="compact-project">{s.project_name}</span>
                    <span className="compact-since">{statusText(s.status)} {formatElapsed(s.status_since_seconds)}</span>
                    <span className="compact-ctx">{remainPct}%</span>
                    <span className="compact-cost">${s.estimated_cost.toFixed(2)}</span>
                  </div>
                  <div className="compact-topic">
                    {s.current_task || s.topic || s.last_message || "-"}
                  </div>
                  <div className="context-bar compact-bar">
                    <div
                      className={`context-bar-fill ${contextBarClass(usedPct)}`}
                      style={{ width: `${usedPct}%` }}
                    />
                  </div>
                </div>
              );
            }

            return (
              <div key={s.session_id} className={`session-card ${s.status.toLowerCase()}`}>
                <div className="session-top">
                  <span className={`session-status ${s.status.toLowerCase()}`}>
                    {statusIcon(s.status)} {statusText(s.status)} {formatElapsed(s.status_since_seconds)}
                  </span>
                  <span className="session-time">
                    Total {formatElapsed(s.elapsed_seconds)}
                  </span>
                </div>

                <div className="session-project">{s.project_name}</div>

                {s.topic && <div className="session-topic">{s.topic}</div>}
                {s.current_task && <div className="session-task">{s.current_task}</div>}
                {s.last_message && s.last_message !== s.topic && (
                  <div className="session-message">{s.last_message}</div>
                )}

                <div className="session-meta">
                  <span>{shortModel(s.model)}</span>
                  <span>累計 ↓{formatTokens(s.tokens_in)} ↑{formatTokens(s.tokens_out)}</span>
                  <span>API費用 ${s.estimated_cost.toFixed(2)}</span>
                </div>
                <div className="session-context-detail">
                  コンテキスト: {formatTokens(s.last_context_used)} / {formatTokens(ctxLimit)} ({usedPct}%使用 / 残り{remainPct}%)
                </div>
                <div className="context-bar">
                  <div
                    className={`context-bar-fill ${contextBarClass(usedPct)}`}
                    style={{ width: `${usedPct}%` }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      )}

      {sessions.length > 0 && (
        <div className="footer">
          <span>{sessions.length} sessions</span>
          <span>API費用合計 ${totalCost.toFixed(2)}</span>
        </div>
      )}
    </div>
  );
}

export default App;
