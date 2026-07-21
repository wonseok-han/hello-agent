import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { ko, type MessageKey } from "./locales/ko";
import { en } from "./locales/en";

export type Lang = "ko" | "en";

const DICTS: Record<Lang, Record<MessageKey, string>> = { ko, en };
const STORAGE_KEY = "hello-agent.lang";

type Params = Record<string, string | number>;

interface I18nContextValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: MessageKey, params?: Params) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function detectInitialLang(): Lang {
  const saved =
    typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  if (saved === "ko" || saved === "en") return saved;
  const nav = typeof navigator !== "undefined" ? navigator.language : "en";
  return nav.toLowerCase().startsWith("ko") ? "ko" : "en";
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detectInitialLang);

  const setLang = useCallback((next: Lang) => {
    setLangState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // 저장 실패는 무시 — 현재 세션 언어는 그대로 유지된다
    }
  }, []);

  const t = useCallback(
    (key: MessageKey, params?: Params) => {
      let text = DICTS[lang][key] ?? ko[key] ?? key;
      if (params) {
        for (const [k, v] of Object.entries(params)) {
          text = text.split(`{${k}}`).join(String(v));
        }
      }
      return text;
    },
    [lang],
  );

  const value = useMemo(() => ({ lang, setLang, t }), [lang, setLang, t]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
