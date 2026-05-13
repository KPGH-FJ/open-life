/**
 * DEV-only logger — suppresses console output in production builds.
 */
const DEV = import.meta.env.DEV;

export function logError(...args: unknown[]): void {
  if (DEV) console.error(...args);
}

export function logWarn(...args: unknown[]): void {
  if (DEV) console.warn(...args);
}

export function logInfo(...args: unknown[]): void {
  if (DEV) console.log(...args);
}
