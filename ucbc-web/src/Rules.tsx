import { Link } from "react-router";
import { Rule, Section } from "./prose";
import { dino, Sprite, sprites, Tiles } from "./Tiles";

const SECTIONS = [
  ["overview", "overview"],
  ["board", "the board"],
  ["turns", "turns"],
  ["bones", "bones"],
  ["dinos", "dinos"],
  ["fossils", "fossils"],
  ["artifacts", "artifacts"],
  ["merge", "merge and split"],
  ["attack", "attacking"],
  ["vision", "vision"],
  ["winning", "winning"],
] as const;

const LEGEND = [
  [sprites.dirt, "empty"],
  [sprites.wall, "wall"],
  [sprites.aLab, "lab"],
  [sprites.fossil, "fossil"],
  [dino("a", 1), "red dino"],
  [dino("b", 1), "blue dino"],
  [sprites.lava, "lava"],
  [sprites.artifact, "artifact"],
  [sprites.beacon, "beacon"],
] as const;

const DAMAGE = [
  [1, 1, 2],
  [2, 1, 1],
  [3, 1, 4],
  [4, 1, 9],
  [1, 4, 4],
] as const;

function Legend() {
  return (
    <div className="my-3 grid grid-cols-3 gap-2 sm:grid-cols-9">
      {LEGEND.map(([src, label]) => (
        <div key={label} className="flex flex-col items-center gap-1 text-xs text-muted">
          <Sprite src={src} className="!size-10" />
          {label}
        </div>
      ))}
    </div>
  );
}

function Level({ team, level }: { team: "a" | "b"; level: number }) {
  return (
    <span className="flex items-center gap-2">
      <Sprite src={dino(team, level)} className="!size-6" />
      level {level}
    </span>
  );
}

export default function Rules() {
  return (
    <main className="docs mx-auto grid w-full max-w-6xl grid-cols-[minmax(0,1fr)] gap-12 px-4 py-8 sm:px-6 sm:py-10 md:grid-cols-[11rem_minmax(0,42rem)]">
      <nav className="text-sm md:sticky md:top-24 md:self-start">
        <p className="mb-2 text-xs font-medium tracking-wide text-muted uppercase">Game rules</p>
        <ul className="mb-6 flex flex-col gap-0.5">
          {SECTIONS.map(([id, title]) => (
            <li key={id}>
              <a href={`#${id}`} className="block rounded-md px-2 py-1 text-muted hover:bg-hover hover:text-fg">
                {title}
              </a>
            </li>
          ))}
        </ul>
        <Link to="/docs" className="block rounded-md px-2 py-1 text-muted hover:bg-hover hover:text-fg">
          Docs →
        </Link>
      </nav>

      <article>
        <Section id="overview" title="overview">
          <p>
            Two teams each have a lab that spends bones to spawn dinos. Dinos bring fossils home
            to earn more bones, claim artifacts for new powers, merge to grow stronger, and fight.
            A game lasts 1000 ticks, and each bot/dino runs once per tick.
          </p>
          <Tiles
            rows={[
              "A  _  .  .  #  .  .  #  .  .  B  _",
              "_  _  a1 .  #  f  f  #  .  b1 _  _",
              ".  .  .  .  .  .  .  .  .  .  .  .",
              ".  f  .  #  .  a2 b2 .  #  .  f  .",
              "~  .  .  #  .  .  .  .  #  .  .  ~",
              "~  .  f  .  .  *  *  .  .  f  .  ~",
            ]}
          />
          <p className="dim">
            To write and run a bot, see the <Link to="/docs">docs</Link>.
          </p>
        </Section>

        <Section id="board" title="the board">
          <p>
            A grid up to 100×100, mirrored so both teams start alike. Each tile holds at most one
            thing.
          </p>
          <Legend />
          <Rule title="Diagonals count as one away.">
            Every range is a square: 1 away is the 8 tiles around you.
          </Rule>
          <Tiles
            rows={[".  .  .  .  .", ".  .+ .+ .+ .", ".  .+ a1 .+ .", ".  .+ .+ .+ .", ".  .  .  .  ."]}
            caption="The tiles 1 away from a dino."
          />
          <p>
            Dinos can't stand on walls, labs, artifacts or beacons, but can slip diagonally between
            two walls. Lava kills any dino that steps on it.
          </p>
        </Section>

        <Section id="turns" title="turns">
          <p>
            Every tick, each bot takes a turn in the order it was spawned, so a random team's
            lab goes first. Per turn, a bot can:
          </p>
          <ul>
            <li>move once,</li>
            <li>attack once,</li>
            <li>merge or split once,</li>
            <li>and, if it's a lab, spawn once.</li>
          </ul>
          <p className="dim">Extra actions are refused, but your code keeps running.</p>
        </Section>

        <Section id="bones" title="bones">
          <p>Bones are shared by the whole team. You start with 50.</p>
          <div className="stats">
            <div>
              <b>+1</b>
              <span>per tick</span>
            </div>
            <div>
              <b>+1</b>
              <span>per tick per fossil home</span>
            </div>
            <div>
              <b>10</b>
              <span>to spawn, +5% each time</span>
            </div>
            <div>
              <b>5</b>
              <span>per attack</span>
            </div>
          </div>
        </Section>

        <Section id="dinos" title="dinos">
          <p>
            The lab can spawn a level 1 dino with 10 health on any free tile around it.
          </p>
          <Tiles
            rows={[".+ .+ .+ .+", ".+ A  _  .+", ".+ _  _  .+", ".+ .+ .+ .+"]}
            caption="Where a lab can spawn."
          />
        </Section>

        <Section id="fossils" title="fossils">
          <p>
            Grab a fossil to carry it; a dino holds one thing at a time. Drop it on your lab and
            your income goes up by 1 per tick. If a dino dies, what it held falls where it stood,
            for anyone to take.
          </p>
          <Tiles rows={["A  _  .  .  f", "_  _  a1f .  ."]} caption="Bring fossils home." />
        </Section>

        <Section id="artifacts" title="artifacts">
          <p>
            Claim an artifact by placing a beacon on it, for bones. A beacon starts at level 1 with
            high health; spend more bones to level it up. Enemies can attack it, with the same
            damage as against dinos.
          </p>
          <Tiles rows={[".  .  .", "*o a1 .", ".  .  ."]} caption="A claimed artifact." />
          <p>Each claimed artifact is worth 100 points and powers up your dinos:</p>
          <table>
            <tbody>
              <tr>
                <td>Long arm</td>
                <td>+1 attack range</td>
              </tr>
              <tr>
                <td>Long neck</td>
                <td>+1 vision</td>
              </tr>
              <tr>
                <td>Tooth</td>
                <td>more damage</td>
              </tr>
            </tbody>
          </table>
          <p>A level <em>n</em> beacon powers up your dinos of level <em>n</em> and below.</p>
        </Section>

        <Section id="merge" title="merge and split">
          <p>
            <strong>Merge</strong> with a teammate 1 away: you move onto its tile and absorb it.
            Levels add, health adds (up to 50).
          </p>
          <div className="flex items-start gap-4">
            <Tiles rows={["a1 a1"]} caption="level 1 + level 1" />
            <span className="my-4 leading-9 text-muted">→</span>
            <Tiles rows={[".  a2"]} caption="level 2" />
          </div>
          <p>
            <strong>Split</strong> a dino above level 1 into two whose levels add up to the
            original. Both keep its health. The new one acts from next tick.
          </p>
          <div className="flex items-start gap-4">
            <Tiles rows={["a3 ."]} caption="level 3" />
            <span className="my-4 leading-9 text-muted">→</span>
            <Tiles rows={["a2 a1"]} caption="level 2 + level 1" />
          </div>
        </Section>

        <Section id="attack" title="attacking">
          <p>
            Attack an enemy 1 away for 5 bones. Bigger hits smaller for the level gap squared.
            Equals deal 2. Smaller hits bigger for half that, rounded down. Always at least 1.
          </p>
          <table className="flags">
            <tbody>
              <tr className="dim">
                <td>attacker</td>
                <td>target</td>
                <td>damage</td>
              </tr>
              {DAMAGE.map(([a, t, d]) => (
                <tr key={`${a}-${t}`}>
                  <td>
                    <Level team="a" level={a} />
                  </td>
                  <td>
                    <Level team="b" level={t} />
                  </td>
                  <td>{d}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p>
            A dino at 0 health dies at once. Anything damaged can be healed for bones, about 1 bone
            per health.
          </p>
          <Rule title="Labs have 100 health. Lose yours and you lose the game." />
        </Section>

        <Section id="vision" title="vision">
          <p>
            A bot sees a square around it: 2 from a lab's edge, 3 for a dino, plus 1 for every 2
            levels after the first, plus any artifact bonus. Looking further raises an error.
          </p>
          <p>
            Your own level and health, and your team's bones, fossils and artifacts, are always
            known. The enemy's totals and bot counts never are.
          </p>
        </Section>

        <Section id="winning" title="winning">
          <p>Destroy the enemy lab to win at once. Otherwise, after 1000 ticks, the team with:</p>
          <ol className="numbered">
            <li>the most points: 1 per unspent bone, 100 per claimed artifact,</li>
            <li>then the most fossils home,</li>
            <li>then the highest-level dino,</li>
            <li>then a coin toss.</li>
          </ol>
          <p>
            Or save up a huge pile of bones and buy an <strong>extinction event</strong>.
          </p>
        </Section>
      </article>
    </main>
  );
}
