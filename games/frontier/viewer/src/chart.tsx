import { useState } from "react";

export interface Series {
  name: string;
  /** A CSS colour for the line; text keeps text colours. */
  colour: string;
  values: number[];
}

const W = 400;
const PAD = { left: 34, right: 8, top: 8, bottom: 18 };

/** A round top for the y axis: 1, 2, or 5 times a power of ten. */
function niceMax(max: number): number {
  if (max <= 0) return 1;
  const step = 10 ** Math.floor(Math.log10(max));
  return [1, 2, 5, 10].map((m) => m * step).find((n) => n >= max)!;
}

function compact(n: number): string {
  return n >= 1000 ? `${+(n / 1000).toFixed(1)}K` : String(n);
}

/** Lines over the match, one per series, sharing `ticks` for x. A hairline marks the
 * frame on show; hovering shows every series' value at the nearest sample. */
export function LineChart({
  series,
  ticks,
  current,
  height = 120,
  yMax,
  label,
}: {
  series: Series[];
  ticks: number[];
  current: number;
  height?: number;
  yMax?: number;
  label: string;
}) {
  const [hover, setHover] = useState<number | null>(null);
  const top = niceMax(yMax ?? Math.max(0, ...series.flatMap((s) => s.values)));
  const last = Math.max(1, ticks[ticks.length - 1] ?? 1);
  const x = (tick: number) => PAD.left + (tick / last) * (W - PAD.left - PAD.right);
  const y = (v: number) => PAD.top + (1 - v / top) * (height - PAD.top - PAD.bottom);
  const nearest = (tick: number) => {
    let best = 0;
    ticks.forEach((t, i) => Math.abs(t - tick) < Math.abs(ticks[best]! - tick) && (best = i));
    return best;
  };
  const shown = hover ?? null;

  return (
    <div className="fr-chart">
      <svg
        viewBox={`0 0 ${W} ${height}`}
        role="img"
        aria-label={label}
        onPointerMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          const sx = ((e.clientX - r.left) / r.width) * W;
          setHover(nearest(((sx - PAD.left) / (W - PAD.left - PAD.right)) * last));
        }}
        onPointerLeave={() => setHover(null)}
      >
        {[0, top / 2, top].map((v) => (
          <g key={v}>
            <line className="fr-grid" x1={PAD.left} x2={W - PAD.right} y1={y(v)} y2={y(v)} />
            <text className="fr-axis" x={PAD.left - 4} y={y(v)} textAnchor="end" dominantBaseline="middle">
              {compact(v)}
            </text>
          </g>
        ))}
        <text className="fr-axis" x={PAD.left} y={height - 4}>
          tick 0
        </text>
        <text className="fr-axis" x={W - PAD.right} y={height - 4} textAnchor="end">
          {last}
        </text>
        <line className="fr-now" x1={x(current)} x2={x(current)} y1={PAD.top} y2={height - PAD.bottom} />
        {series.map((s) => (
          <polyline
            key={s.name}
            className="fr-line"
            stroke={s.colour}
            points={s.values.map((v, i) => `${x(ticks[i]!)},${y(v)}`).join(" ")}
          />
        ))}
        {shown == null ? null : (
          <g>
            <line className="fr-hair" x1={x(ticks[shown]!)} x2={x(ticks[shown]!)} y1={PAD.top} y2={height - PAD.bottom} />
            {series.map((s) => (
              <circle key={s.name} className="fr-dot" cx={x(ticks[shown]!)} cy={y(s.values[shown]!)} r={4} fill={s.colour} />
            ))}
          </g>
        )}
      </svg>
      {shown == null ? null : (
        <div className="fr-tip" style={{ left: `${(x(ticks[shown]!) / W) * 100}%` }}>
          <div className="ucbc-muted">{`tick ${ticks[shown]}`}</div>
          {series.map((s) => (
            <div key={s.name} className="fr-tip-row">
              <span className="fr-key" style={{ background: s.colour }} />
              <strong>{s.values[shown]}</strong>
              <span className="ucbc-muted">{s.name}</span>
            </div>
          ))}
        </div>
      )}
      <div className="fr-chart-legend">
        {series.map((s) => (
          <span key={s.name}>
            <span className="fr-key" style={{ background: s.colour }} />
            {s.name}
          </span>
        ))}
      </div>
    </div>
  );
}
