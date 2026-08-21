type ErrorRecord = Record<string, unknown>;

function asRecord(value: unknown): ErrorRecord | null {
  return value !== null && typeof value === "object" ? (value as ErrorRecord) : null;
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function journeyErrorCode(error: unknown): string {
  if (error instanceof Error) return error.message || error.name || "unknown_error";
  if (typeof error === "string") return error.trim() || "unknown_error";

  const record = asRecord(error);
  if (!record) return "unknown_error";

  const detail = asRecord(record.detail);
  const kind = nonEmptyString(record.kind);
  const code = nonEmptyString(detail?.code) ?? nonEmptyString(record.code);
  const hint = nonEmptyString(detail?.hint) ?? nonEmptyString(record.hint);

  if (kind && hint) return `${kind}:${hint}`;
  if (code) return code;
  if (hint) return hint;
  if (kind) return `${kind}:command_failed`;
  return "structured_command_error";
}
