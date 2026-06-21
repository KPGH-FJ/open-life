const EMAIL_PATTERN = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi;
const PHONE_PATTERN = /(?<!\d)(?:\+?\d[\d\s().-]{7,}\d)(?!\d)/g;
const TOKEN_PATTERN =
  /\b(?:sk|pk|ghp|gho|ghu|github_pat|xoxb|xoxp|api[_-]?key|token)[A-Za-z0-9_\-:.]{8,}\b/gi;
const LONG_SECRET_PATTERN = /\b[A-Za-z0-9_\-]{32,}\b/g;

export function safePreviewText(value?: string | null, maxLength = 120): string {
  if (!value) return "无内容";
  const normalized = value.replace(/\s+/g, " ").trim();
  const redacted = normalized
    .replace(EMAIL_PATTERN, "[email]")
    .replace(PHONE_PATTERN, "[phone]")
    .replace(TOKEN_PATTERN, "[secret]")
    .replace(LONG_SECRET_PATTERN, "[secret]");
  if (redacted.length <= maxLength) return redacted;
  return `${redacted.slice(0, Math.max(0, maxLength - 1)).trimEnd()}...`;
}
