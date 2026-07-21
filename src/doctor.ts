import type { MessageKey } from "./locales/ko";

export interface Diagnosis {
  titleKey: MessageKey;
  adviceKey: MessageKey;
}

// 에러 메시지·로그에서 흔한 실패 원인을 짚어 초보자용 진단으로 바꾼다.
// 위에서부터 먼저 맞는 패턴을 쓴다 (구체적인 것 먼저).
const PATTERNS: { match: RegExp; title: MessageKey; advice: MessageKey }[] = [
  {
    match: /could not connect|connection refused|network|timed? ?out|resolve host|getaddrinfo|dns|offline/i,
    title: "doctor.network.title",
    advice: "doctor.network.advice",
  },
  {
    match: /checksum|verification failed|corrupt/i,
    title: "doctor.checksum.title",
    advice: "doctor.checksum.advice",
  },
  {
    match: /command not found|not recognized|no such file|enoent/i,
    title: "doctor.notfound.title",
    advice: "doctor.notfound.advice",
  },
  {
    match: /permission denied|eacces|not permitted|operation not permitted/i,
    title: "doctor.permission.title",
    advice: "doctor.permission.advice",
  },
  {
    match: /no space|enospc|disk (is )?full/i,
    title: "doctor.disk.title",
    advice: "doctor.disk.advice",
  },
];

type Source = string | string[] | null | undefined;

export function diagnose(...sources: Source[]): Diagnosis {
  const text = sources
    .flatMap((s) => (Array.isArray(s) ? s : [s]))
    .filter((s): s is string => Boolean(s))
    .join("\n");
  for (const p of PATTERNS) {
    if (p.match.test(text)) {
      return { titleKey: p.title, adviceKey: p.advice };
    }
  }
  return { titleKey: "doctor.generic.title", adviceKey: "doctor.generic.advice" };
}
