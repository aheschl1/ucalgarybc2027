import { useMemo, useState } from "react";

export interface Series {
  name: string;
  /** A CSS colour for the line or area; text keeps text colours. */
  colour: string;
  values: number[];
}

/** A labelled moment on the x axis, such as a commander's death. */
export interface Marker {
  tick: number;
  label: string;
}

const W = 400;
const PAD = { left: 34, right: 8, top: 14, bottom: 18 };

/** Gridlines at a step of 1, 2, or 5 times a power of ten, three or four of them, up to
 * just above `max`. */
export function axis(max: number): number[] {
  const rough = Math.max(max, 1) / 3;
  const power = 10 ** Math.floor(Math.log10(rough));
  const step = [1, 2, 5, 10].map((m) => m * power).find((s) => s >= rough)!;
  const top = Math.max(step, Math.ceil(max / step) * step);
  return Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
}

function compact(n: number): string {
  return n >= 1000 ? `${+(n / 1000).toFixed(1)}K` : String(+n.toPrecision(6));
}

/** Lines over the match, one per series, or with `stacked` areas piled up from the
 * first series, sharing `ticks` for x. A hairline marks the frame on show; hovering
 * shows every series' value at the nearest sample. */
export function LineChart({
  series,
  ticks,
  current,
  height = 100,
  yMax,
  stacked = false,
  outline,
  markers = [],
  label,
}: {
  series: Series[];
  ticks: number[];
  current: number;
  height?: number;
  yMax?: number;
  stacked?: boolean;
  /** The colour of the stack's top edge. */
  outline?: string;
  markers?: Marker[];
  label: string;
}) {
  const [hover, setHover] = useState<number | null>(null);
  // With `stacked`, each series' values are the running sum up to it.
  const tops = useMemo(
    () =>
      stacked
        ? series.map((_, k) => ticks.map((_, i) => series.slice(0, k + 1).reduce((a, s) => a + s.values[i]!, 0)))
        : series.map((s) => s.values),
    [series, ticks, stacked],
  );
  const grid = useMemo(
    () => axis(yMax ?? tops.reduce((m, v) => v.reduce((a, b) => Math.max(a, b), m), 0)),
    [tops, yMax],
  );
  const top = grid[grid.length - 1]!;
  const last = Math.max(1, ticks[ticks.length - 1] ?? 1);
  const x = (tick: number) => PAD.left + (tick / last) * (W - PAD.left - PAD.right);
  const y = (v: number) => PAD.top + (1 - v / top) * (height - PAD.top - PAD.bottom);
  const { paths, edge } = useMemo(() => {
    const along = (values: number[]) => values.map((v, i) => `${x(ticks[i]!)},${y(v)}`);
    const paths = tops.map((values, k) => {
      const line = along(values);
      if (!stacked) return line.join(" ");
      const below = k > 0 ? along(tops[k - 1]!).reverse() : [`${x(last)},${y(0)}`, `${x(0)},${y(0)}`];
      return [...line, ...below].join(" ");
    });
    return { paths, edge: along(tops[tops.length - 1] ?? []).join(" ") };
    // x and y follow from these.
  }, [tops, ticks, top, height, stacked]);
  const nearest = (tick: number) => {
    let best = 0;
    for (let i = 1; i < ticks.length; i++) {
      if (Math.abs(ticks[i]! - tick) < Math.abs(ticks[best]! - tick)) best = i;
    }
    return best;
  };
  const hx = hover == null ? 0 : x(ticks[hover]!);

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
        {grid.map((v) => (
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
        {series.map((s, i) =>
          stacked ? (
            <polygon key={s.name} className="fr-area" style={{ fill: s.colour }} points={paths[i]} />
          ) : (
            <polyline key={s.name} className="fr-line" style={{ stroke: s.colour }} points={paths[i]} />
          ),
        )}
        {stacked && outline ? <polyline className="fr-line" style={{ stroke: outline }} points={edge} /> : null}
        <line className="fr-now" x1={x(current)} x2={x(current)} y1={PAD.top} y2={height - PAD.bottom} />
        {markers.map((m) => (
          <g key={`${m.tick}${m.label}`}>
            <line className="fr-mark" x1={x(m.tick)} x2={x(m.tick)} y1={PAD.top - 4} y2={height - PAD.bottom} />
            <text className="fr-axis" x={x(m.tick) - 3} y={PAD.top - 5} textAnchor="end">
              {m.label}
            </text>
          </g>
        ))}
        {hover == null ? null : (
          <g>
            <line className="fr-hair" x1={hx} x2={hx} y1={PAD.top} y2={height - PAD.bottom} />
            {tops.map((values, k) => (
              <circle key={k} className="fr-dot" cx={hx} cy={y(values[hover]!)} r={4} style={{ fill: series[k]!.colour }} />
            ))}
          </g>
        )}
      </svg>
      {hover == null ? null : (
        <div className={`fr-tip${hx > W * 0.6 ? " fr-tip-left" : ""}`} style={{ left: `${(hx / W) * 100}%` }}>
          <div className="ucbc-muted">{`tick ${ticks[hover]}`}</div>
          {series.map((s) => (
            <div key={s.name} className="fr-tip-row">
              <span className="fr-key" style={{ background: s.colour }} />
              <strong>{s.values[hover]}</strong>
              <span className="ucbc-muted">{s.name}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Keys for the series of one or more charts, shown once: strokes for lines, squares
 * for areas. */
export function Keys({ entries, areas = false }: { entries: [string, string][]; areas?: boolean }) {
  return (
    <div className="fr-chart-legend">
      {entries.map(([name, colour]) => (
        <span key={name}>
          <span className={areas ? "fr-key-area" : "fr-key"} style={{ background: colour }} />
          {name}
        </span>
      ))}
    </div>
  );
}
