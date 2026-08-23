import { invoke } from "@tauri-apps/api/core";

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

export function safeInvoke<T>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (!isTauriEnv()) {
    return Promise.reject(
      new Error("当前不在 OpenLife 桌面应用环境中，无法调用原生功能。请在桌面窗口内操作。")
    );
  }
  if (import.meta.env.DEV && import.meta.env.MODE !== "test") {
    console.log("[safeInvoke]", cmd, redactInvokeArgs(cmd, args));
  }
  return invoke<T>(cmd, args);
}

type RedactedValue =
  | null
  | string
  | number
  | boolean
  | {
      redacted?: true;
      type?: string;
      keys?: string[];
      length?: number;
      itemCount?: number;
      hash?: string;
      items?: RedactedValue[];
      [key: string]: any;
    };

const SECRET_KEY_RE = /(openai_key|api_key|password|token|secret|authorization|credential)/i;
const PAYLOAD_KEY_RE = /(payload|import|export)/i;
const TOOL_ARGUMENT_KEY_RE = /^(arguments|args|toolArguments|tool_arguments)$/i;
const CONTENT_KEY_RE = /^(content|fileContent|file_content|body|emailBody|email_body)$/i;
const NOTES_KEY_RE = /^(notes|note|testerNotes|tester_notes)$/i;
const SESSION_KEY_RE = /^(sessionId|session_id)$/;

export function redactInvokeArgs(
  cmd: string,
  args?: Record<string, any>
): Record<string, RedactedValue> | undefined {
  void cmd;
  if (!args) return args;
  const redacted: Record<string, RedactedValue> = {};
  for (const [key, value] of Object.entries(args)) {
    redacted[key] = redactValue(value, key);
  }
  return redacted;
}

function redactValue(value: any, key: string): RedactedValue {
  if (value == null) return value;
  if (SESSION_KEY_RE.test(key) && typeof value === "string") return value;
  if (SECRET_KEY_RE.test(key)) return summarizeSensitive(value);
  if (PAYLOAD_KEY_RE.test(key)) return summarizeSensitive(value);
  if (TOOL_ARGUMENT_KEY_RE.test(key)) return summarizeSensitive(value);
  if (CONTENT_KEY_RE.test(key)) return summarizeSensitive(value);
  if (NOTES_KEY_RE.test(key)) return summarizeSensitive(value);

  if (Array.isArray(value)) {
    if (key === "messages") {
      return {
        type: "array",
        itemCount: value.length,
        items: value.map(item =>
          item && typeof item === "object"
            ? {
                role: typeof item.role === "string" ? item.role : summarizeSensitive(item.role),
                content: summarizeSensitive(item.content),
              }
            : summarizeSensitive(item)
        ),
      };
    }
    return {
      type: "array",
      itemCount: value.length,
      hash: stableHash(JSON.stringify(value)),
    };
  }

  if (typeof value === "object") {
    const output: Record<string, RedactedValue | string[] | undefined> = {
      type: "object",
      keys: Object.keys(value).sort(),
    };
    for (const [childKey, childValue] of Object.entries(value)) {
      output[childKey] = redactValue(childValue, childKey);
    }
    return output as RedactedValue;
  }

  if (typeof value === "string") {
    return summarizeSensitive(value);
  }
  return value;
}

function summarizeSensitive(value: any): RedactedValue {
  const serialized = typeof value === "string" ? value : JSON.stringify(value);
  return {
    redacted: true,
    type: Array.isArray(value) ? "array" : typeof value,
    keys:
      value && typeof value === "object" && !Array.isArray(value)
        ? Object.keys(value).sort()
        : undefined,
    length: serialized.length,
    hash: stableHash(serialized),
  };
}

function stableHash(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return `fnv1a:${(hash >>> 0).toString(16).padStart(8, "0")}`;
}
