export type Lang = "en" | "ja" | "zh" | "ko";

export const LANGS: { code: Lang; label: string }[] = [
  { code: "en", label: "EN" },
  { code: "ja", label: "JP" },
  { code: "zh", label: "CN" },
  { code: "ko", label: "KR" },
];

const messages = {
  en: {
    running: "Running",
    waiting: "Waiting",
    done: "Done",
    error: "Error",
    detail: "Detail",
    compact: "Compact",
    noSessions: "No sessions",
    startClaude: "Start a Claude Code session in your terminal",
    total: "Total",
    apiCost: "API cost",
    apiCostTotal: "API cost total",
    context: "Context",
    used: "used",
    remaining: "remaining",
    cumulative: "Cumulative",
    sessions: "sessions",
    sortManual: "Manual",
    sortStatus: "Status",
    sortCost: "Cost",
    sortTime: "Time",
    bg: "bg",
  },
  ja: {
    running: "実行中",
    waiting: "入力待ち",
    done: "完了",
    error: "エラー",
    detail: "詳細",
    compact: "簡易",
    noSessions: "セッションなし",
    startClaude: "Claude Codeを起動してください",
    total: "合計",
    apiCost: "API費用",
    apiCostTotal: "API費用合計",
    context: "コンテキスト",
    used: "使用",
    remaining: "残り",
    cumulative: "累計",
    sessions: "sessions",
    sortManual: "手動",
    sortStatus: "状態順",
    sortCost: "費用順",
    sortTime: "時間順",
    bg: "bg",
  },
  zh: {
    running: "运行中",
    waiting: "等待输入",
    done: "完成",
    error: "错误",
    detail: "详细",
    compact: "简洁",
    noSessions: "无会话",
    startClaude: "请启动 Claude Code",
    total: "合计",
    apiCost: "API费用",
    apiCostTotal: "API费用合计",
    context: "上下文",
    used: "已用",
    remaining: "剩余",
    cumulative: "累计",
    sessions: "sessions",
    sortManual: "手动",
    sortStatus: "状态",
    sortCost: "费用",
    sortTime: "时间",
    bg: "bg",
  },
  ko: {
    running: "실행 중",
    waiting: "입력 대기",
    done: "완료",
    error: "오류",
    detail: "상세",
    compact: "간략",
    noSessions: "세션 없음",
    startClaude: "Claude Code를 시작하세요",
    total: "합계",
    apiCost: "API 비용",
    apiCostTotal: "API 비용 합계",
    context: "컨텍스트",
    used: "사용",
    remaining: "잔여",
    cumulative: "누적",
    sessions: "sessions",
    sortManual: "수동",
    sortStatus: "상태",
    sortCost: "비용",
    sortTime: "시간",
    bg: "bg",
  },
} as const;

export type Messages = {
  running: string;
  waiting: string;
  done: string;
  error: string;
  detail: string;
  compact: string;
  noSessions: string;
  startClaude: string;
  total: string;
  apiCost: string;
  apiCostTotal: string;
  context: string;
  used: string;
  remaining: string;
  cumulative: string;
  sessions: string;
  sortManual: string;
  sortStatus: string;
  sortCost: string;
  sortTime: string;
  bg: string;
};

export function getMessages(lang: Lang): Messages {
  return messages[lang];
}

export function detectLang(): Lang {
  const saved = localStorage.getItem("sessiondock-lang");
  if (saved && saved in messages) return saved as Lang;
  const nav = navigator.language || "";
  if (nav.startsWith("ja")) return "ja";
  if (nav.startsWith("zh")) return "zh";
  if (nav.startsWith("ko")) return "ko";
  return "en";
}
