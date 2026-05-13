interface CompletionRadarProps {
  variant: "completion";
  values: number[];
  size?: number;
}

interface SkillsRadarProps {
  variant: "skills";
  skills: { name: string; value: number }[];
}

type RadarChartProps = CompletionRadarProps | SkillsRadarProps;

const COMPLETION_LABELS = ["Identity", "Goals", "Capabilities", "State"];

function CompletionChart({ values, size = 120 }: { values: number[]; size?: number }) {
  const total = values.length;
  const center = size / 2;
  const radius = size * 0.38;
  const angleFor = (i: number) => (Math.PI * 2 * i) / total - Math.PI / 2;

  const framePoints = COMPLETION_LABELS.map((_, i) => {
    const a = angleFor(i);
    return `${center + radius * Math.cos(a)},${center + radius * Math.sin(a)}`;
  });

  const valuePoints = values.map((v, i) => {
    const a = angleFor(i);
    const r = radius * Math.max(0, Math.min(1, v / 100));
    return `${center + r * Math.cos(a)},${center + r * Math.sin(a)}`;
  });

  return (
    <svg width={size} height={size} className="shrink-0">
      <polygon points={framePoints.join(" ")} fill="none" stroke="#e5e7eb" strokeWidth={1} />
      {[0.25, 0.5, 0.75].map(scale => {
        const ring = values.map((_, i) => {
          const a = angleFor(i);
          const r = radius * scale;
          return `${center + r * Math.cos(a)},${center + r * Math.sin(a)}`;
        });
        return (
          <polygon
            key={scale}
            points={ring.join(" ")}
            fill="none"
            stroke="#f3f4f6"
            strokeWidth={1}
          />
        );
      })}
      {COMPLETION_LABELS.map((_, i) => {
        const a = angleFor(i);
        return (
          <line
            key={i}
            x1={center}
            y1={center}
            x2={center + radius * Math.cos(a)}
            y2={center + radius * Math.sin(a)}
            stroke="#e5e7eb"
            strokeWidth={1}
          />
        );
      })}
      <polygon
        points={valuePoints.join(" ")}
        fill="rgba(99,102,241,0.25)"
        stroke="#6366f1"
        strokeWidth={2}
      />
    </svg>
  );
}

function SkillsChart({ skills }: { skills: { name: string; value: number }[] }) {
  if (skills.length === 0) return <div className="text-sm text-gray-500">暂无能力数据</div>;
  const size = 200;
  const cx = size / 2;
  const cy = size / 2;
  const radius = 80;
  const total = skills.length;
  const points = skills.map((_, i) => {
    const angle = (Math.PI * 2 * i) / total - Math.PI / 2;
    const x = cx + radius * Math.cos(angle);
    const y = cy + radius * Math.sin(angle);
    return { x, y };
  });
  const valuePoints = skills.map((s, i) => {
    const angle = (Math.PI * 2 * i) / total - Math.PI / 2;
    const r = radius * Math.min(1, Math.max(0, s.value / 5));
    const x = cx + r * Math.cos(angle);
    const y = cy + r * Math.sin(angle);
    return { x, y };
  });
  const poly = valuePoints.map(p => `${p.x},${p.y}`).join(" ");
  return (
    <div className="flex flex-col items-center">
      <svg width={size} height={size}>
        <circle cx={cx} cy={cy} r={radius} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        <circle cx={cx} cy={cy} r={radius * 0.6} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        <circle cx={cx} cy={cy} r={radius * 0.3} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        {points.map((p, i) => (
          <line key={i} x1={cx} y1={cy} x2={p.x} y2={p.y} stroke="#e5e7eb" strokeWidth={1} />
        ))}
        <polygon points={poly} fill="rgba(99,102,241,0.2)" stroke="#6366f1" strokeWidth={2} />
        {valuePoints.map((p, i) => (
          <circle key={i} cx={p.x} cy={p.y} r={3} fill="#6366f1" />
        ))}
      </svg>
      <div className="flex flex-wrap justify-center gap-3 mt-3">
        {skills.map(s => (
          <div key={s.name} className="flex items-center gap-1 text-xs text-gray-600">
            <span className="w-2 h-2 rounded-full bg-indigo-500" />
            {s.name}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function RadarChart(props: RadarChartProps) {
  if (props.variant === "completion") {
    return <CompletionChart values={props.values} size={props.size} />;
  }
  return <SkillsChart skills={props.skills} />;
}
