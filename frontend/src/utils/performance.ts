/**
 * Performance monitoring for OpenLife frontend
 * Tracks Web Vitals and custom metrics
 */

interface PerformanceMetrics {
  lcp?: number; // Largest Contentful Paint
  fid?: number; // First Input Delay
  cls?: number; // Cumulative Layout Shift
  fcp?: number; // First Contentful Paint
  ttfb?: number; // Time to First Byte
  tti?: number; // Time to Interactive (estimated)
}

let metrics: PerformanceMetrics = {};

/**
 * Initialize performance monitoring
 * Call this once at app startup
 */
export function initPerformanceMonitoring(): void {
  if (typeof window === "undefined" || !("PerformanceObserver" in window)) {
    return;
  }

  // Largest Contentful Paint
  try {
    const lcpObserver = new PerformanceObserver(entryList => {
      const entries = entryList.getEntries();
      const lastEntry = entries[entries.length - 1] as PerformanceEntry & {
        startTime: number;
      };
      metrics.lcp = lastEntry.startTime;
    });
    lcpObserver.observe({ entryTypes: ["largest-contentful-paint"] });
  } catch {
    // LCP not supported
  }

  // First Input Delay
  try {
    const fidObserver = new PerformanceObserver(entryList => {
      const entries = entryList.getEntries();
      const firstEntry = entries[0] as PerformanceEntry & {
        processingStart: number;
        startTime: number;
      };
      metrics.fid = firstEntry.processingStart - firstEntry.startTime;
    });
    fidObserver.observe({ entryTypes: ["first-input"] });
  } catch {
    // FID not supported
  }

  // Cumulative Layout Shift
  try {
    const clsObserver = new PerformanceObserver(entryList => {
      let clsValue = 0;
      for (const entry of entryList.getEntries()) {
        const layoutShiftEntry = entry as PerformanceEntry & {
          hadRecentInput: boolean;
          value: number;
        };
        if (!layoutShiftEntry.hadRecentInput) {
          clsValue += layoutShiftEntry.value;
        }
      }
      metrics.cls = clsValue;
    });
    clsObserver.observe({ entryTypes: ["layout-shift"] });
  } catch {
    // CLS not supported
  }

  // First Contentful Paint
  try {
    const paintObserver = new PerformanceObserver(entryList => {
      for (const entry of entryList.getEntries()) {
        if (entry.name === "first-contentful-paint") {
          metrics.fcp = entry.startTime;
        }
      }
    });
    paintObserver.observe({ entryTypes: ["paint"] });
  } catch {
    // Paint API not supported
  }

  // Time to First Byte (from navigation timing)
  window.addEventListener("load", () => {
    setTimeout(() => {
      const navEntry = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming;
      if (navEntry) {
        metrics.ttfb = navEntry.responseStart;
      }

      // Estimate TTI from long tasks
      const longTasks = performance.getEntriesByType("longtask");
      if (longTasks.length > 0) {
        const lastLongTask = longTasks[longTasks.length - 1];
        metrics.tti = lastLongTask.startTime + lastLongTask.duration;
      }
    }, 0);
  });
}

/**
 * Get current performance metrics
 */
export function getPerformanceMetrics(): PerformanceMetrics {
  return { ...metrics };
}

/**
 * Track a custom timing
 */
export function trackTiming(name: string, duration: number): void {
  if (typeof window !== "undefined" && "performance" in window) {
    performance.mark(`${name}_end`);
    // Store in a simple registry
    const customTimings = (window as unknown as Record<string, unknown>)["__openlifeTimings"] as
      | Record<string, number[]>
      | undefined;
    if (!customTimings) {
      (window as unknown as Record<string, unknown>)["__openlifeTimings"] = {};
    }
    const timings = (window as unknown as Record<string, unknown>)["__openlifeTimings"] as Record<
      string,
      number[]
    >;
    if (!timings[name]) {
      timings[name] = [];
    }
    timings[name].push(duration);
  }
}

/**
 * Measure a function's execution time
 */
export function measure<T>(name: string, fn: () => T): T {
  const start = performance.now();
  const result = fn();
  const duration = performance.now() - start;
  trackTiming(name, duration);
  return result;
}

/**
 * Report metrics for debugging (development only)
 */
export function reportMetrics(): void {
  if (import.meta.env.DEV) {
    // eslint-disable-next-line no-console
    console.log("[Performance] Metrics:", getPerformanceMetrics());
  }
}
