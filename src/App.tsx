import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Lang, LANGS, Messages, getMessages, detectLang } from "./i18n";

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
  bg_processes: number;
}

type SortKey = "manual" | "status" | "cost" | "time";

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

function statusText(status: string, t: Messages): string {
  switch (status) {
    case "Running": return t.running;
    case "Waiting": return t.waiting;
    case "Done": return t.done;
    case "Error": return t.error;
    default: return status;
  }
}

function App() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [compact, setCompact] = useState(() => {
    const saved = localStorage.getItem("sd-compact");
    return saved !== null ? saved === "true" : true;
  });
  const [sortKey, setSortKey] = useState<SortKey>(() => {
    return (localStorage.getItem("sd-sortKey") as SortKey) || "status";
  });
  const [sortAsc, setSortAsc] = useState(() => {
    const saved = localStorage.getItem("sd-sortAsc");
    return saved !== null ? saved === "true" : true;
  });
  const [manualOrder, setManualOrder] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem("sd-manualOrder") || "[]"); } catch { return []; }
  });
  const [manualEditing, setManualEditing] = useState(false);
  const [dragId, setDragId] = useState<string | null>(null);
  const [hiddenIds, setHiddenIds] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem("sd-hiddenIds") || "[]"); } catch { return []; }
  });
  const [showHidden, setShowHidden] = useState(false);
  const [lang, setLang] = useState<Lang>(detectLang);
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    return (localStorage.getItem("sd-theme") as "dark" | "light") || "dark";
  });
  const t = getMessages(lang);

  function changeLang(l: Lang) {
    setLang(l);
    localStorage.setItem("sessiondock-lang", l);
  }

  // テーマ変更をDOMに反映
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("sd-theme", theme);
  }, [theme]);

  // 設定変更時にlocalStorageに保存
  useEffect(() => { localStorage.setItem("sd-compact", String(compact)); }, [compact]);
  useEffect(() => { localStorage.setItem("sd-sortKey", sortKey); }, [sortKey]);
  useEffect(() => { localStorage.setItem("sd-sortAsc", String(sortAsc)); }, [sortAsc]);
  useEffect(() => { localStorage.setItem("sd-manualOrder", JSON.stringify(manualOrder)); }, [manualOrder]);
  useEffect(() => { localStorage.setItem("sd-hiddenIds", JSON.stringify(hiddenIds)); }, [hiddenIds]);

  useEffect(() => {
    invoke<Session[]>("get_sessions").then(setSessions);
    const unlisten = listen<Session[]>("sessions-updated", (event) => {
      setSessions(event.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const statusOrder: Record<string, number> = { Running: 0, Waiting: 1, Done: 2, Error: 3 };

  // manualモード: 新しいセッションが来たらmanualOrderに追加
  useEffect(() => {
    if (sortKey === "manual") {
      setManualOrder((prev) => {
        const ids = sessions.map((s) => s.session_id);
        const newIds = ids.filter((id) => !prev.includes(id));
        const valid = prev.filter((id) => ids.includes(id));
        return [...valid, ...newIds];
      });
    }
  }, [sessions, sortKey]);

  const visibleSessions = sessions.filter((s) => !hiddenIds.includes(s.session_id));
  const hiddenSessions = sessions.filter((s) => hiddenIds.includes(s.session_id));

  const sorted = [...visibleSessions].sort((a, b) => {
    if (sortKey === "manual") {
      return manualOrder.indexOf(a.session_id) - manualOrder.indexOf(b.session_id);
    }
    let cmp = 0;
    if (sortKey === "status") cmp = (statusOrder[a.status] ?? 9) - (statusOrder[b.status] ?? 9);
    else if (sortKey === "cost") cmp = b.estimated_cost - a.estimated_cost;
    else if (sortKey === "time") cmp = b.status_since_seconds - a.status_since_seconds;
    return sortAsc ? cmp : -cmp;
  });

  function toggleSort(key: SortKey) {
    if (key === "manual") {
      if (sortKey !== "manual") {
        // 他のソートからmanualへ: 編集モードON
        setSortKey("manual");
        setManualEditing(true);
        setManualOrder(sorted.map((s) => s.session_id));
      } else {
        // 既にmanual: 編集モードのトグル
        setManualEditing(!manualEditing);
      }
      return;
    }
    if (sortKey === key) {
      setSortAsc(!sortAsc);
    } else {
      setSortKey(key);
      setSortAsc(true);
    }
  }

  const isEditing = sortKey === "manual" && manualEditing;

  function handleDragStart(e: React.DragEvent, id: string) {
    setDragId(id);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", id);
  }

  function handleDragOver(e: React.DragEvent, targetId: string) {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (!dragId || dragId === targetId) return;
    setManualOrder((prev) => {
      const arr = [...prev];
      const from = arr.indexOf(dragId);
      const to = arr.indexOf(targetId);
      if (from === -1 || to === -1) return prev;
      arr.splice(from, 1);
      arr.splice(to, 0, dragId);
      return arr;
    });
  }

  function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    setDragId(null);
  }

  function handleDragEnd() {
    setDragId(null);
  }

  function handleDoubleClick(pid: number, cwd: string) {
    invoke("focus_terminal", { pid, cwd }).catch((err) => {
      console.error("Failed to focus terminal:", err);
    });
  }

  function moveSession(id: string, direction: "up" | "down") {
    setManualOrder((prev) => {
      const arr = [...prev];
      const idx = arr.indexOf(id);
      if (idx === -1) return prev;
      const target = direction === "up" ? idx - 1 : idx + 1;
      if (target < 0 || target >= arr.length) return prev;
      [arr[idx], arr[target]] = [arr[target], arr[idx]];
      return arr;
    });
  }

  function hideSession(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    setHiddenIds((prev) => prev.includes(id) ? prev : [...prev, id]);
  }

  function restoreSession(e: React.MouseEvent, id: string) {
    e.stopPropagation();
    setHiddenIds((prev) => prev.filter((x) => x !== id));
  }

  const active = visibleSessions.filter((s) => s.status === "Running").length;
  const waiting = visibleSessions.filter((s) => s.status === "Waiting").length;
  const done = visibleSessions.filter((s) => s.status === "Done").length;
  const totalCost = visibleSessions.reduce((sum, s) => sum + s.estimated_cost, 0);

  return (
    <div className="app">
      <div className="lang-bar">
        {LANGS.map((l) => (
          <button
            key={l.code}
            className={`lang-btn ${lang === l.code ? "active" : ""}`}
            onClick={() => changeLang(l.code)}
          >
            {l.label}
          </button>
        ))}
      </div>

      <div className="header">
        <h1>SessionDock</h1>
        <div className="header-right">
          <div className="sort-btns">
            {(["manual", "status", "cost", "time"] as SortKey[]).map((k) => (
              <button
                key={k}
                className={`sort-btn ${sortKey === k ? "active" : ""}`}
                onClick={() => toggleSort(k)}
              >
                {k === "manual"
                  ? (sortKey === "manual" && manualEditing ? "✋✏" : sortKey === "manual" ? "✋🔒" : "✋")
                  : k === "status" ? t.sortStatus : k === "cost" ? t.sortCost : t.sortTime}
                {sortKey === k && k !== "manual" && (sortAsc ? "▲" : "▼")}
              </button>
            ))}
          </div>
          <button className="theme-toggle" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
            {theme === "dark" ? "☀" : "🌙"}
          </button>
          <button className="view-toggle" onClick={() => setCompact(!compact)}>
            {compact ? t.detail : t.compact}
          </button>
          <div className="status-badges">
            {active > 0 && <span className="badge active">▶{active}</span>}
            {waiting > 0 && <span className="badge waiting">⏸{waiting}</span>}
            {done > 0 && <span className="badge done">✓{done}</span>}
          </div>
        </div>
      </div>

      {showHidden ? (
        /* ===== 非表示セッション一覧画面 ===== */
        <>
          <div className="hidden-header">
            <button className="back-btn" onClick={() => setShowHidden(false)}>
              ← {t.backToMain}
            </button>
            <span className="hidden-title">{t.hidden} ({hiddenSessions.length})</span>
          </div>
          {hiddenSessions.length === 0 ? (
            <div className="empty-state">
              <h2>{t.hidden}</h2>
              <p>-</p>
            </div>
          ) : (
            <div className="sessions">
              {hiddenSessions.map((s) => (
                <div key={s.session_id} className={`session-compact ${s.status.toLowerCase()}`}>
                  <div className="compact-row">
                    <span className={`compact-status ${s.status.toLowerCase()}`}>
                      {statusIcon(s.status)}
                    </span>
                    <span className="compact-project">{s.project_name}</span>
                    <span className="compact-since">
                      {statusText(s.status, t)} {formatElapsed(s.status_since_seconds)}
                    </span>
                    <button className="restore-btn" onClick={(e) => restoreSession(e, s.session_id)}>
                      {t.restore}
                    </button>
                  </div>
                  <div className="compact-topic">
                    {s.current_task || s.topic || s.last_message || "-"}
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      ) : (
        /* ===== メイン画面 ===== */
        <>
          {visibleSessions.length === 0 && hiddenSessions.length === 0 ? (
            <div className="empty-state">
              <h2>{t.noSessions}</h2>
              <p>{t.startClaude}</p>
            </div>
          ) : (
            <div className="sessions">
              {sorted.map((s) => {
                const usedPct = contextUsedPercent(s.model, s.last_context_used);
                const remainPct = 100 - usedPct;
                const ctxLimit = getContextLimit(s.model);

                if (compact) {
                  return (
                    <div
                      key={s.session_id}
                      className={`session-compact ${s.status.toLowerCase()} ${dragId === s.session_id ? "dragging" : ""}`}
                      draggable={isEditing}
                      onDragStart={(e) => handleDragStart(e, s.session_id)}
                      onDragOver={(e) => handleDragOver(e, s.session_id)}
                      onDrop={handleDrop}
                      onDragEnd={handleDragEnd}
                      onClick={() => handleDoubleClick(s.pid, s.cwd)}
                    >
                      <div className="compact-row">
                        {isEditing && (
                          <span className="move-btns">
                            <button className="move-btn" onClick={() => moveSession(s.session_id, "up")}>&#9650;</button>
                            <button className="move-btn" onClick={() => moveSession(s.session_id, "down")}>&#9660;</button>
                          </span>
                        )}
                        <span className={`compact-status ${s.status.toLowerCase()}`}>
                          {statusIcon(s.status)}
                        </span>
                        {s.bg_processes > 0 && <span className="bg-badge">{s.bg_processes}{t.bg}</span>}
                        <span className="compact-project">{s.project_name}</span>
                        <span className="compact-since">
                          {statusText(s.status, t)} {formatElapsed(s.status_since_seconds)}
                        </span>
                        <span className="compact-ctx">{remainPct}%</span>
                        <span className="compact-cost">${s.estimated_cost.toFixed(2)}</span>
                        <button className="hide-btn" onClick={(e) => hideSession(e, s.session_id)} title={t.hide}>×</button>
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
                  <div
                    key={s.session_id}
                    className={`session-card ${s.status.toLowerCase()} ${dragId === s.session_id ? "dragging" : ""}`}
                    draggable={isEditing}
                    onDragStart={(e) => handleDragStart(e, s.session_id)}
                    onDragOver={(e) => handleDragOver(e, s.session_id)}
                    onDrop={handleDrop}
                    onDragEnd={handleDragEnd}
                    onClick={() => handleDoubleClick(s.pid, s.cwd)}
                  >
                    <div className="session-top">
                      <span className={`session-status ${s.status.toLowerCase()}`}>
                        {isEditing && (
                          <span className="move-btns">
                            <button className="move-btn" onClick={() => moveSession(s.session_id, "up")}>&#9650;</button>
                            <button className="move-btn" onClick={() => moveSession(s.session_id, "down")}>&#9660;</button>
                          </span>
                        )}
                        {statusIcon(s.status)} {s.bg_processes > 0 && <span className="bg-badge">{s.bg_processes}{t.bg}</span>} {statusText(s.status, t)} {formatElapsed(s.status_since_seconds)}
                      </span>
                      <span className="session-time">
                        <button className="hide-btn" onClick={(e) => hideSession(e, s.session_id)} title={t.hide}>×</button>
                        {t.total} {formatElapsed(s.elapsed_seconds)}
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
                      <span>{t.cumulative} ↓{formatTokens(s.tokens_in)} ↑{formatTokens(s.tokens_out)}</span>
                      <span>{t.apiCost} ${s.estimated_cost.toFixed(2)}</span>
                    </div>
                    <div className="session-context-detail">
                      {t.context}: {formatTokens(s.last_context_used)} / {formatTokens(ctxLimit)} ({usedPct}% {t.used} / {t.remaining} {remainPct}%)
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
              <span>{visibleSessions.length} {t.sessions}</span>
              {hiddenSessions.length > 0 && (
                <button className="hidden-link" onClick={() => setShowHidden(true)}>
                  {t.hidden} ({hiddenSessions.length})
                </button>
              )}
              <span>{t.apiCostTotal} ${totalCost.toFixed(2)}</span>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default App;
