/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        "ol-canvas": "var(--ol-canvas)",
        "ol-sidebar": "var(--ol-sidebar)",
        "ol-surface": "var(--ol-surface)",
        "ol-surface-subtle": "var(--ol-surface-subtle)",
        "ol-surface-sunken": "var(--ol-surface-sunken)",
        "ol-line": "var(--ol-line)",
        "ol-line-strong": "var(--ol-line-strong)",
        "ol-ink": "var(--ol-ink)",
        "ol-ink-secondary": "var(--ol-ink-secondary)",
        "ol-ink-muted": "var(--ol-ink-muted)",
        "ol-amber": "var(--ol-amber)",
        "ol-amber-soft": "var(--ol-amber-soft)",
        "ol-red": "var(--ol-red)",
        "ol-red-soft": "var(--ol-red-soft)",
        "ol-green": "var(--ol-green)",
        "ol-green-soft": "var(--ol-green-soft)",
        "ol-focus": "var(--ol-focus)",
      },
      spacing: {
        "ol-1": "var(--ol-space-1)",
        "ol-2": "var(--ol-space-2)",
        "ol-3": "var(--ol-space-3)",
        "ol-4": "var(--ol-space-4)",
        "ol-5": "var(--ol-space-5)",
        "ol-6": "var(--ol-space-6)",
        "ol-8": "var(--ol-space-8)",
        "ol-10": "var(--ol-space-10)",
        "ol-12": "var(--ol-space-12)",
      },
      borderRadius: {
        "ol-1": "var(--ol-radius-1)",
        "ol-2": "var(--ol-radius-2)",
        "ol-3": "var(--ol-radius-3)",
      },
      fontFamily: {
        "ol-ui": "var(--ol-font-ui)",
        "ol-mono": "var(--ol-font-mono)",
      },
      fontSize: {
        "ol-caption": ["var(--ol-type-caption)", "var(--ol-line-caption)"],
        "ol-body": ["var(--ol-type-body)", "var(--ol-line-body)"],
        "ol-reading": ["var(--ol-type-reading)", "var(--ol-line-reading)"],
        "ol-section": ["var(--ol-type-section)", "var(--ol-line-section)"],
        "ol-surface": ["var(--ol-type-surface)", "var(--ol-line-surface)"],
        "ol-display": ["var(--ol-type-display)", "var(--ol-line-display)"],
      },
      boxShadow: {
        "ol-overlay": "var(--ol-shadow-overlay)",
      },
    },
  },
  plugins: [],
};
