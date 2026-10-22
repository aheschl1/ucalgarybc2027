import { type CSSProperties, type ReactNode } from "react";
import { assets, dinoTier } from "@ucbc/viewer-ucbc2027/assets";
import { cx } from "./ui";

const { empty: dirt, wall, fossil, lab, unused } = assets;
const { lava, artifact, beacon } = unused;
const [aLab, bLab] = lab as [string, string];

export const sprites = { dirt, wall, lava, artifact, beacon, fossil, aLab, bLab };

/** Team `a` or `b`'s dino at `level`, in its first frame. */
export const dino = (team: "a" | "b", level: number) =>
  assets.dino[team === "a" ? 0 : 1]![dinoTier(level)]![0]!;

export function Sprite({
  src,
  className,
  style,
}: {
  src: string;
  className?: string;
  style?: CSSProperties;
}) {
  return <img src={src} alt="" className={cx("pixel size-full", className)} style={style} />;
}

/** What one tile shows above its ground. */
function Item({ tile }: { tile: string }) {
  const d = tile.match(/^([ab])(\d)(f?)$/);
  if (d)
    return (
      <>
        <Sprite src={dino(d[1] as "a" | "b", Number(d[2]))} />
        {d[3] && <Sprite src={fossil} className="absolute -top-[10%] -right-[10%] !size-1/2" />}
      </>
    );
  if (tile === "f") return <Sprite src={fossil} className="p-[10%]" />;
  if (tile === "*") return <Sprite src={artifact} />;
  if (tile === "*o")
    return (
      <>
        <Sprite src={artifact} />
        <Sprite src={beacon} className="absolute inset-0 p-[15%]" />
      </>
    );
  return null;
}

/**
 * A board drawn from rows of space-separated tiles: `.` dirt, `#` wall, `~` lava,
 * `*` artifact, `*o` a beacon on one, `f` fossil, `a1`..`b4` a team's dino of that level
 * (`a1f` holding a fossil), `A`/`B` a lab's top-left tile and `_` the rest of it.
 * A trailing `+` highlights the tile.
 */
export function Tiles({ rows, caption }: { rows: string[]; caption?: ReactNode }) {
  const grid = rows.map((r) => r.trim().split(/\s+/));
  const width = grid[0]!.length;
  const cell = (x: number, y: number, span = 1): CSSProperties => ({
    gridColumn: `${x + 1} / span ${span}`,
    gridRow: `${y + 1} / span ${span}`,
  });

  const tiles = grid.flatMap((row, y) =>
    row.map((raw, x) => {
      const lit = raw.endsWith("+");
      return { x, y, lit, tile: lit ? raw.slice(0, -1) : raw };
    }),
  );

  return (
    <figure className="my-4">
      <div
        className="grid max-w-full overflow-hidden rounded-lg border border-line"
        style={{ gridTemplateColumns: `repeat(${width}, minmax(0, 1fr))`, width: `${width * 2.25}rem` }}
        aria-hidden
      >
        {tiles.map(({ x, y, tile }) => (
          <Sprite
            key={`g${x},${y}`}
            src={tile === "#" ? wall : tile === "~" ? lava : dirt}
            className="aspect-square"
            style={cell(x, y)}
          />
        ))}
        {tiles.map(
          ({ x, y, lit }) =>
            lit && <div key={`h${x},${y}`} className="bg-accent/25 ring-1 ring-accent/70 ring-inset" style={cell(x, y)} />,
        )}
        {tiles.map(
          ({ x, y, tile }) =>
            (tile === "A" || tile === "B") && (
              <Sprite key={`l${x},${y}`} src={tile === "A" ? aLab : bLab} style={cell(x, y, 2)} />
            ),
        )}
        {tiles.map(({ x, y, tile }) => (
          <div key={`i${x},${y}`} className="relative" style={cell(x, y)}>
            <Item tile={tile} />
          </div>
        ))}
      </div>
      {caption && <figcaption className="mt-2 text-sm text-muted">{caption}</figcaption>}
    </figure>
  );
}
