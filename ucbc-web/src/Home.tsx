import { type CSSProperties, useEffect, useState } from "react";
import { Link } from "react-router";
import { get, type TeamElo, type User } from "./api";
import { buttonClass, cx } from "./ui";
import dinoA1 from "./assets/a01.png";
import dinoA2 from "./assets/a02.png";
import dinoB1 from "./assets/b01.png";
import dinoB2 from "./assets/b02.png";
import fossil from "./assets/fossil.png";

const STEPS = [
  ["Develop", "Write a python program that defines step(handle)."],
  ["Run", "Play your bot against an opponent locally."],
  ["Submit", "Upload your bot, queue matches, and climb the ladder."],
] as const;

const REPO = "https://github.com/aheschl1/ucalgarybc2027";

/** The landing page, public. */
export default function Home({ user }: { user?: User }) {
  return (
    <main className="animate-rise">
      <section className="relative overflow-hidden border-b border-line">
        <div className="grid-bg pointer-events-none absolute inset-0" />
        <div className="relative mx-auto grid max-w-6xl items-center gap-12 px-4 py-16 sm:px-6 sm:py-24 lg:grid-cols-[1fr_30rem]">
          <div>
            {/* <p className="mb-5 inline-flex items-center gap-2 rounded-full border border-line bg-bg px-3 py-1 text-xs text-muted">
              <span className="size-1.5 rounded-full bg-accent" />
              University of Calgary · 2027 season
            </p> */}
            <h1 className="text-4xl leading-[1.05] font-semibold tracking-tight text-balance sm:text-6xl">
              UCalgary
              <br />
              <span className="text-4xl leading-[1.05] font-semibold tracking-tight text-balance sm:text-6xl">Battlecode</span>
            </h1>
            <p className="mt-6 max-w-md text-base leading-relaxed text-muted">
               some text or something
            </p>
            <div className="mt-8 flex flex-wrap gap-3">
              <Link to="/platform" className={cx(buttonClass(), "h-10 px-5")}>
                Open platform
              </Link>
              <Link to="/docs" className={cx(buttonClass("secondary"), "h-10 px-5")}>
                Read the docs
              </Link>
            </div>
          </div>
          <Snippet />
        </div>
      </section>

      <section className="mx-auto grid max-w-6xl gap-12 px-4 py-16 sm:px-6 lg:grid-cols-[1fr_24rem]">
        <div>
          <ol className="grid gap-8 sm:grid-cols-3 sm:gap-6">
            {STEPS.map(([title, text], i) => (
              <li key={title}>
                <span className="font-mono text-xs text-accent">0{i + 1}</span>
                <h2 className="mt-2 font-semibold">{title}</h2>
                <p className="mt-1 leading-relaxed text-muted">{text}</p>
              </li>
            ))}
          </ol>
          <a
            href={REPO}
            target="_blank"
            rel="noreferrer"
            className="group mt-12 flex items-center gap-4 rounded-xl border border-line p-4 transition-colors hover:bg-panel"
          >
            <svg viewBox="0 0 16 16" className="size-6 shrink-0" fill="currentColor" aria-hidden>
              <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
            </svg>
            <span className="font-medium">Open on GitHub</span>
            <span className="ml-auto text-muted transition-colors group-hover:text-fg">
              →
            </span>
          </a>
        </div>
        <Standings teamId={user?.team_id} />
      </section>
    </main>
  );
}

/** The whole of a bot, and the command that plays it. */
function Snippet() {
  const k = "text-accent";
  const m = "text-muted";
  return (
    <div>
      <Arena />
      <div className="overflow-hidden rounded-xl border border-line bg-bg shadow-lg shadow-fg/[0.04]">
        <div className="border-b border-line bg-panel px-4 py-2.5">
          <span className="font-mono text-xs text-muted">bota/main.py</span>
        </div>
        <pre className="overflow-x-auto px-4 py-4 font-mono text-[13px] leading-relaxed">
          <span className={k}>def</span> <span className="font-semibold">step</span>(game):
          {"\n    "}dir = pick_direction()
          {"\n    "}x, y = game.me().pos.add(dir)
          {"\n    "}
          <span className={k}>if not</span> game.me().holding <span className={k}>and</span> game.item(x, y):
          {"\n        "}game.grab(x, y)
          {"\n    "}
          <span className={k}>elif</span> game.me().holding <span className={k}>and</span> should_drop():
          {"\n        "}game.drop(x, y)
          {"\n    "}game.move(dir)
        </pre>
        <div className="border-t border-line bg-panel px-4 py-3 font-mono text-xs">
          <span className={m}>$</span> ucbc run bota/ bota/ --view
        </div>
      </div>
    </div>
  );
}

// The strip above the code window, in tiles; the dinos and the skull each fill one.
const TILES = 12;

/** One team's dino: its two frames, where its eye is (of 16 pixels), its colour, and
 * which way the sprite faces as drawn. */
const LOOKS = [
  { frames: [dinoA1, dinoA2], eye: [11, 5], color: "#e52f2f", facing: 1 },
  { frames: [dinoB1, dinoB2], eye: [4, 5], color: "#5b6ee1", facing: -1 },
] as const;

type Walker = { x: number; facing: 1 | -1; steps: number };
type World = { dinos: [Walker, Walker]; skull: number | { held: 0 | 1 } };

const START: World = {
  dinos: [
    { x: 1, facing: 1, steps: 0 },
    { x: TILES - 2, facing: -1, steps: 0 },
  ],
  skull: Math.floor(TILES / 2),
};

/** Dino `i`'s next move, from `roll` in [0, 1): grab the skull in front of it, drop the
 * one it holds, step forward, or turn round. It never leaves the strip or walks into
 * the other dino or the skull. */
function act(w: World, i: 0 | 1, roll: number): World {
  const me = w.dinos[i];
  const ahead = me.x + me.facing;
  const free =
    ahead >= 0 && ahead < TILES && ahead !== w.dinos[i === 0 ? 1 : 0].x && ahead !== w.skull;
  const dinos = [...w.dinos] as World["dinos"];
  const holding = typeof w.skull === "object" && w.skull.held === i;

  if (ahead === w.skull && roll < 0.6) return { ...w, skull: { held: i } };
  if (holding && free && roll < 0.25) return { ...w, skull: ahead };
  if (free && roll < 0.75) {
    dinos[i] = { ...me, x: ahead, steps: me.steps + 1 };
  } else {
    dinos[i] = { ...me, facing: me.facing === 1 ? -1 : 1 };
  }
  return { ...w, dinos };
}

/** Two dinos pottering about over a skull, each on its own random clock. */
function Arena() {
  const [world, setWorld] = useState(START);
  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timers = ([0, 1] as const).map((i) => {
      let timer: ReturnType<typeof setTimeout>;
      const wait = () => {
        timer = setTimeout(() => {
          const roll = Math.random();
          setWorld((w) => act(w, i, roll));
          wait();
        }, 1200 + Math.random() * 3500);
      };
      wait();
      return () => clearTimeout(timer);
    });
    return () => timers.forEach((stop) => stop());
  }, []);

  // A tile's left edge: the strip's width less one sprite, shared out between tiles.
  const left = (x: number) => `calc((100% - 3rem) * ${x / (TILES - 1)})`;
  return (
    <div className="relative mx-4 h-12" aria-hidden>
      {typeof world.skull === "number" && (
        <img
          src={fossil}
          alt=""
          className="pixel absolute bottom-0.5 size-6 translate-x-3"
          style={{ left: left(world.skull) }}
        />
      )}
      {world.dinos.map((d, i) => (
        <Dino
          key={i}
          look={LOOKS[i]!}
          walker={d}
          holding={typeof world.skull === "object" && world.skull.held === i}
          style={{ left: left(d.x) }}
        />
      ))}
    </div>
  );
}

/** A dino on the strip. It shows its second frame for a moment after each step, blinks
 * now and then (its one-pixel eye covered in its body's colour), and, as the replay
 * viewer draws it, holds the skull at half size in its bottom-right quarter. */
function Dino({
  look,
  walker,
  holding,
  style,
}: {
  look: (typeof LOOKS)[number];
  walker: Walker;
  holding: boolean;
  style: CSSProperties;
}) {
  const [shut, setShut] = useState(false);
  const [striding, setStriding] = useState(false);

  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    let timer: ReturnType<typeof setTimeout>;
    const wait = () => {
      timer = setTimeout(() => {
        setShut(true);
        timer = setTimeout(() => {
          setShut(false);
          wait();
        }, 150);
      }, 2000 + Math.random() * 5000);
    };
    wait();
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    if (walker.steps === 0) return;
    setStriding(true);
    const timer = setTimeout(() => setStriding(false), 250);
    return () => clearTimeout(timer);
  }, [walker.steps]);

  const [x, y] = look.eye;
  return (
    <span
      className="absolute bottom-0 size-12 transition-[left] duration-300 ease-out"
      style={{ ...style, transform: walker.facing === look.facing ? undefined : "scaleX(-1)" }}
    >
      <img src={look.frames[striding ? 1 : 0]} alt="" className="pixel size-full" />
      {shut && (
        <span
          className="absolute size-[6.25%]"
          style={{ left: `${x * 6.25}%`, top: `${y * 6.25}%`, background: look.color }}
        />
      )}
      {holding && <img src={fossil} alt="" className="pixel absolute right-0 bottom-0 size-1/2" />}
    </span>
  );
}

/** The top of the leaderboard. */
function Standings({ teamId }: { teamId?: number }) {
  const [teams, setTeams] = useState<TeamElo[] | null>(null);
  useEffect(() => {
    get<TeamElo[]>("/teams/elo").then(setTeams, () => setTeams([]));
  }, []);
  const top = teams?.filter((t) => t.matches > 0).slice(0, 5);

  return (
    <aside className="rounded-xl border border-line">
      <header className="flex items-center justify-between border-b border-line px-4 py-3">
        <h2 className="font-semibold">Standings</h2>
        <Link to="/leaderboard" className="text-xs text-muted hover:text-fg">
          Full leaderboard →
        </Link>
      </header>
      {top && top.length === 0 ? (
        <p className="px-4 py-8 text-center text-muted">No ranked matches yet.</p>
      ) : (
        <ol className="divide-y divide-line">
          {(top ?? Array.from({ length: 5 }, () => null)).map((t, i) => (
            <li
              key={t?.id ?? i}
              className={cx("flex items-center gap-3 px-4 py-2.5", t?.id === teamId && "bg-accent-soft")}
            >
              <span className={cx("w-4 font-mono text-xs tabular-nums", i === 0 ? "text-accent" : "text-muted")}>
                {i + 1}
              </span>
              {t ? (
                <>
                  <span className="truncate font-medium">{t.name}</span>
                  <span className="ml-auto font-mono text-xs tabular-nums">{t.elo}</span>
                </>
              ) : (
                <span className="h-3.5 w-1/2 animate-pulse rounded bg-hover" />
              )}
            </li>
          ))}
        </ol>
      )}
    </aside>
  );
}
