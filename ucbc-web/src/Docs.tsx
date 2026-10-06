import { useState, type ReactNode } from "react";
import { Link } from "react-router";

type Os = "linux" | "windows";

const STEPS = [
  ["install", "install"],
  ["write", "write a bot"],
  ["run", "run it locally"],
  ["submit", "submit"],
  ["watch", "watch"],
] as const;

const REFERENCE = [
  ["limits", "limits"],
  ["print", "debugging with print"],
  ["maps", "maps"],
  ["teams", "teams"],
] as const;

function Section({
  id,
  n,
  title,
  children,
}: {
  id: string;
  n?: number;
  title: string;
  children: ReactNode;
}) {
  return (
    <section id={id}>
      <h2>
        {n !== undefined && <span className="n">{n}</span>}
        {title}
      </h2>
      {children}
    </section>
  );
}

function Rule({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="rule">
      <strong>{title}</strong>
      {children && <div>{children}</div>}
    </div>
  );
}

function Code({ children, label }: { children: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    navigator.clipboard.writeText(children).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  return (
    <div className="code">
      {label && <span className="label">{label}</span>}
      <button type="button" onClick={copy}>
        {copied ? "copied" : "copy"}
      </button>
      <pre>
        <code>{children}</code>
      </pre>
    </div>
  );
}

function Tabs<K extends string>({
  value,
  options,
  onChange,
}: {
  value: K;
  options: readonly (readonly [K, string])[];
  onChange: (k: K) => void;
}) {
  return (
    <div className="tabs">
      {options.map(([k, label]) => (
        <button
          key={k}
          type="button"
          className={k === value ? "on" : ""}
          onClick={() => onChange(k)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

const UV = `uv init ucbc-bots
cd ucbc-bots
uv add ucbc`;

const pip = {
  linux: `python3 -m venv .venv
source .venv/bin/activate
pip install ucbc`,
  windows: `py -m venv .venv
.venv\\Scripts\\Activate.ps1
pip install ucbc`,
};

const zip = {
  linux: `cd mybot && zip -r ../mybot.zip .`,
  windows: `Compress-Archive -Path mybot\\* -DestinationPath mybot.zip`,
};

const BOT = `# mybot/main.py
from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    handle.noop()`;

const STATE = `import random

from ucbc.handle import ActionError


class Bot:
    def __init__(self, handle):
        self.rng = random.Random(handle.seed)

    def step(self, handle):
        try:
            ...  # an action
        except ActionError:
            ...  # refused: nothing lost, try another


bot = None


def step(handle):
    global bot
    if bot is None:
        bot = Bot(handle)
    bot.step(handle)`;

export default function Docs() {
  const [os, setOs] = useState<Os>(
    navigator.userAgent.includes("Windows") ? "windows" : "linux",
  );
  const [tool, setTool] = useState<"uv" | "pip">("uv");
  const shell = os === "windows" ? "PowerShell" : "terminal";

  return (
    <main className="docs mx-auto grid w-full max-w-6xl gap-12 px-4 py-8 sm:px-6 sm:py-10 md:grid-cols-[11rem_minmax(0,42rem)]">
      <nav className="text-sm md:sticky md:top-24 md:self-start">
        <p className="mb-2 text-xs font-medium tracking-wide text-muted uppercase">
          Get started
        </p>
        <ol className="mb-6 flex flex-col gap-0.5">
          {STEPS.map(([id, title], i) => (
            <li key={id}>
              <a href={`#${id}`} className="flex gap-2 rounded-md px-2 py-1 text-muted hover:bg-hover hover:text-fg">
                <span className="w-3 tabular-nums">{i + 1}</span>
                {title}
              </a>
            </li>
          ))}
        </ol>
        <p className="mb-2 text-xs font-medium tracking-wide text-muted uppercase">
          Reference
        </p>
        <ul className="flex flex-col gap-0.5">
          {REFERENCE.map(([id, title]) => (
            <li key={id}>
              <a href={`#${id}`} className="block rounded-md px-2 py-1 text-muted hover:bg-hover hover:text-fg">
                {title}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <article>
        <div className="os">
          <span className="dim">commands for</span>
          <Tabs
            value={os}
            options={[
              ["linux", "Linux"],
              ["windows", "Windows"],
            ]}
            onChange={setOs}
          />
        </div>

        <Section id="install" n={1} title="install">
          <table>
            <tbody>
              <tr>
                <td>Python</td>
                <td>3.12 or newer</td>
              </tr>
              <tr>
                <td>Linux</td>
                <td>x86_64, aarch64</td>
              </tr>
              <tr>
                <td>Windows</td>
                <td>x86_64</td>
              </tr>
              <tr>
                <td>macOS</td>
                <td className="dim">
                  not supported; use a Linux VM or container
                </td>
              </tr>
            </tbody>
          </table>
          <p>Install <code>ucbc</code> with either:</p>
          <Tabs
            value={tool}
            options={[
              ["uv", "uv"],
              ["pip", "pip + venv"],
            ]}
            onChange={setTool}
          />
          <Code label={shell}>{tool === "uv" ? UV : pip[os]}</Code>
        </Section>

        <Section id="write" n={2} title="write a bot">
          <p>
            A bot is a folder with a <code>main.py</code> that defines{" "}
            <code>step(handle)</code>. Nothing to extend or register.
          </p>
          <Code>{BOT}</Code>
          <p>
            The engine calls <code>step</code> on each of your bot's turns. The
            handle is how it sees the game and acts; your editor lists what it
            offers.
          </p>
          <Rule title="Every unit runs its own copy of main.py.">
            Each has its own globals. Units share nothing.
          </Rule>
          <Rule title="Keep state in a global.">
            Globals last from step to step within a set. Each set starts fresh.
          </Rule>
          <Rule title="A refused action raises ActionError.">
            Nothing is forfeited; catch it and try something else.
          </Rule>
          <Rule title="Seed randomness from handle.seed.">
            It differs per unit and per set but is fixed by the match seed, so
            matches replay exactly.
          </Rule>
          <Code>{STATE}</Code>
        </Section>

        <Section id="run" n={3} title="run it locally">
          <Code label={shell}>ucbc run mybot mybot --view</Code>
          <p>
            Plays one bot folder against another (here, against itself),
            prints each set's result, and opens the replay in your browser.
          </p>
          <table className="flags">
            <tbody>
              <tr>
                <td>
                  <code>--view</code>
                </td>
                <td>open the replay in the viewer</td>
              </tr>
              <tr>
                <td>
                  <code>--sets N</code>
                </td>
                <td>sets to play; default 1, the site plays 3</td>
              </tr>
              <tr>
                <td>
                  <code>--seed N</code>
                </td>
                <td>match seed; default 0</td>
              </tr>
              <tr>
                <td>
                  <code>--replay FILE</code>
                </td>
                <td>
                  save the replay; <code>ucbc view FILE</code> opens it later
                </td>
              </tr>
              <tr>
                <td>
                  <code>--map FILE</code>
                </td>
                <td>
                  play on your own <a href="#maps">map</a>
                </td>
              </tr>
              <tr>
                <td>
                  <code>--show-bot-output</code>
                </td>
                <td>print bot output in the terminal</td>
              </tr>
            </tbody>
          </table>
          <Rule title="Local matches are site matches.">
            Same engine, same <a href="#limits">limits</a>. Same bots and seed
            give the same result on any machine.
          </Rule>
        </Section>

        <Section id="submit" n={4} title="submit">
          <ol className="numbered">
            <li>
              <Link to="/register">Create an account</Link>, or log in.
            </li>
            <li>
              Zip the <em>contents</em> of your bot folder:
              <Code label={shell}>{zip[os]}</Code>
            </li>
            <li>
              On the <Link to="/">home page</Link>, under{" "}
              <strong>Submit a bot</strong>, choose the zip, optionally name it,
              leave the game as <code>ucbc2027</code>, and press{" "}
              <strong>Upload</strong>.
            </li>
            <li>
              Under <strong>Request a match</strong>, pick one of your team's
              submissions and an opponent (any submission, yours included), set
              a seed if you like, and press <strong>Queue match</strong>.
            </li>
          </ol>
          <Rule title="main.py must be at the top of the zip, not in a folder.">
            At most 1 MiB zipped, 8 MiB unpacked.
          </Rule>
          <Rule title="Submissions never change.">
            To update a bot, upload a new zip.
          </Rule>
          <p className="dim">
            Everyone sees a submission's name, team and uploader, and can play
            against it. Nobody else sees its code.
          </p>
        </Section>

        <Section id="watch" n={5} title="watch">
          <p>
            A queued match appears under <strong>Your matches</strong>:
          </p>
          <p className="states">
            <code>queued</code> → <code>running</code> → <code>done</code>
          </p>
          <p>
            Click it to see who won each set and why. Press{" "}
            <strong>Watch</strong> to step through the replay. Your bots and
            matches are also on your <Link to="/profile">profile</Link>.
          </p>
        </Section>

        <Section id="limits" title="limits">
          <p>The same for every bot, locally and on the site.</p>
          <div className="stats">
            <div>
              <b>3 ms</b>
              <span>per step</span>
            </div>
            <div>
              <b>1 GiB</b>
              <span>memory</span>
            </div>
            <div>
              <b>stdlib</b>
              <span>imports</span>
            </div>
            <div>
              <b>offline</b>
              <span>no internet</span>
            </div>
          </div>
          <ul>
            <li>
              <strong>Go over 3 ms and, resume next turn.</strong> What
              your bot already did still counts.
            </li>
            <li>
              <strong>Time counts bytecode, not the clock,</strong> so
              it is the same on every machine.
            </li>
            <li>
              <strong>Import your own files and the standard library.</strong>{" "}
              No pip packages.
            </li>
          </ul>
        </Section>

        <Section id="print" title="debugging with print">
          <Rule title="print is the only way to see inside a bot on the site." />
          <p>
            Output is saved in the replay. In the viewer, click a unit to see
            what it printed. Locally, <code>--show-bot-output</code> also
            prints it in the terminal:
          </p>
          <Code>[set 1 tick 1 team mybot bot 0] tick 1</Code>
        </Section>

        <Section id="maps" title="maps">
          <Code label={shell}>{`ucbc editor my.map
ucbc run mybot mybot --map my.map --view`}</Code>
          <p>
            The editor opens in your browser; <strong>Save</strong> writes the
            file. Site matches use the standard map.
          </p>
        </Section>

        <Section id="teams" title="teams">
          <p>
            Every account starts on a team of its own. To play as a group, one
            of you starts a team on the <Link to="/profile">profile</Link> page
            and the others paste its join code into <strong>join</strong>.
          </p>
          <ul>
            <li>Members share the team's bots and matches.</li>
            <li>Any member can queue matches and replace the join code.</li>
            <li>
              Joining or starting a team moves you out of your last one; its
              bots and matches stay with it.
            </li>
            <li>Team names are unique and cannot be changed.</li>
          </ul>
        </Section>
      </article>
    </main>
  );
}
