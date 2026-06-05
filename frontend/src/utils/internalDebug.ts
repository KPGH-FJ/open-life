export function isInternalDebugSurfaceEnabled(): boolean {
  return (
    (globalThis as any).__OPENLIFE_INTERNAL_DEBUG__ === true ||
    import.meta.env.VITE_OPENLIFE_INTERNAL_DEBUG === "true" ||
    import.meta.env.VITE_OPENLIFE_SHOW_INTERNAL_DEBUG === "true"
  );
}
