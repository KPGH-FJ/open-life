import { ExternalLink } from "lucide-react";
import { openExternalHttpsSource } from "@/tauri";

const WEB_SOURCE_HEADING = "来源（OpenLife 引用已绑定，内容未背书）";
const RESOURCE_SOURCE_HEADING = "来源（OpenLife 已核验）";
const TOOL_EVIDENCE_HEADING = "工具证据（OpenLife 已核验）";

export type WorkspaceMessageSource = {
  id: string;
  label: string;
  detail: string;
  kind: "web" | "resource" | "tool";
  url?: string;
};

export type WorkspaceMessagePresentation = {
  body: string;
  sources: WorkspaceMessageSource[];
};

function safeHttpsUrl(value: string): string | undefined {
  try {
    const parsed = new URL(value);
    if (parsed.protocol !== "https:" || parsed.username || parsed.password) return undefined;
    return parsed.toString();
  } catch {
    return undefined;
  }
}

function unescapeBackendLabel(value: string): string {
  return value.replace(/\\([\\[\]()])/g, "$1").trim();
}

function parseWebSource(line: string): WorkspaceMessageSource | null {
  const match = line.match(
    /^\s*-\s+`([^`]+)`\s+—\s+\[((?:\\.|[^\]])+)\]\(([^)]+)\)\s+—\s+(.+)\s*$/
  );
  if (!match) return null;
  const url = safeHttpsUrl(match[3]);
  if (!url) return null;
  return {
    id: match[1],
    label: unescapeBackendLabel(match[2]),
    detail: unescapeBackendLabel(match[4]),
    kind: "web",
    url,
  };
}

function parseResourceSource(line: string): WorkspaceMessageSource | null {
  const match = line.match(/^\s*-\s+`([^`]+)`\s+—\s+(.+?)\s+—\s+(.+)\s*$/);
  if (!match) return null;
  return {
    id: match[1],
    label: unescapeBackendLabel(match[2]),
    detail: unescapeBackendLabel(match[3]),
    kind: "resource",
  };
}

function parseToolEvidence(line: string): WorkspaceMessageSource | null {
  const source = parseResourceSource(line);
  return source ? { ...source, kind: "tool" } : null;
}

export function parseWorkspaceMessage(content: string): WorkspaceMessagePresentation {
  const headings = [WEB_SOURCE_HEADING, RESOURCE_SOURCE_HEADING, TOOL_EVIDENCE_HEADING]
    .map(heading => ({ heading, index: content.indexOf(`\n\n${heading}`) }))
    .filter(item => item.index >= 0)
    .sort((left, right) => left.index - right.index);
  if (headings.length === 0) return { body: content, sources: [] };

  const body = content.slice(0, headings[0].index).trimEnd();
  const sources: WorkspaceMessageSource[] = [];
  headings.forEach((item, index) => {
    const start = item.index + 2 + item.heading.length;
    const end = headings[index + 1]?.index ?? content.length;
    const parser =
      item.heading === WEB_SOURCE_HEADING
        ? parseWebSource
        : item.heading === TOOL_EVIDENCE_HEADING
          ? parseToolEvidence
          : parseResourceSource;
    content
      .slice(start, end)
      .split("\n")
      .map(parser)
      .filter((source): source is WorkspaceMessageSource => source !== null)
      .forEach(source => sources.push(source));
  });

  // A malformed footer stays visible as ordinary assistant text. This avoids
  // presenting model-authored markdown as a backend-verified source list.
  return sources.length > 0 ? { body, sources } : { body: content, sources: [] };
}

export function WorkspaceMessageContent({
  content,
  allowBackendSources = false,
}: {
  content: string;
  allowBackendSources?: boolean;
}) {
  const presentation = allowBackendSources
    ? parseWorkspaceMessage(content)
    : { body: content, sources: [] };
  return (
    <div className="ol-workspace-message">
      <p>{presentation.body}</p>
      {presentation.sources.length > 0 && (
        <section className="ol-workspace-message__sources" aria-label="本轮来源">
          <strong>来源</strong>
          <ul>
            {presentation.sources.map(source => (
              <li key={`${source.kind}:${source.id}`}>
                {source.url ? (
                  <button
                    type="button"
                    onClick={() => void openExternalHttpsSource(source.url!)}
                    title={source.url}
                  >
                    <span>{source.label}</span>
                    <ExternalLink size={14} aria-hidden="true" />
                  </button>
                ) : (
                  <span>{source.label}</span>
                )}
                <small>
                  {source.kind === "web"
                    ? "外部内容未背书"
                    : source.kind === "tool"
                      ? "只读工具已核验"
                      : "本轮文件已核验"}{" "}
                  · {source.detail}
                </small>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
