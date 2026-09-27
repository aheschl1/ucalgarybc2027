# docs

You write a bot in Python, run it on your own machine, then upload it here and queue
matches against other people's bots. This page takes you from nothing to your first
submitted bot.

- [requirements](#requirements)
- [setup](#setup)
- [write a bot](#write-a-bot)
- [run it locally](#run-it-locally)
- [debugging with print](#debugging-with-print)
- [limits](#limits)
- [submit](#submit)
- [teams](#teams)
- [watch the match](#watch-the-match)

## requirements

- Python 3.12 or newer
- Linux, x86_64 or aarch64

The engine sandboxes bots with Linux facilities, so `ucbc` has no macOS or Windows
builds. On Windows use [WSL2](https://learn.microsoft.com/windows/wsl/install); on macOS
use a Linux container or VM. Everything below is then run inside it.

## setup

Make a directory for your bots and install `ucbc` into an environment there. Use
whichever of the two you prefer.

### pip and venv

```bash
mkdir ucbc-bots && cd ucbc-bots
python3 -m venv .venv
source .venv/bin/activate
pip install ucbc
ucbc --help
```

Run `source .venv/bin/activate` again in each new terminal.

### uv

```bash
mkdir ucbc-bots && cd ucbc-bots
uv init --bare --python 3.12
uv add ucbc
uv run ucbc --help
```

With [uv](https://docs.astral.sh/uv/) there is nothing to activate: put `uv run` in
front of every `ucbc` command on this page.

## write a bot

A bot is a directory containing a `main.py` that defines `step(handle)`. There is no
class to extend and nothing to register.

```python
# mybot/main.py
from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    handle.noop()
```

The engine calls `step` each time it is your bot's turn to act. The handle is how the
bot sees the game and acts on it; your editor will list what the game's handle offers.

A match is a few sets. Module globals and the `handle.memory` dict persist from step to
step within a set, and each set starts fresh. `handle.seed` differs per bot and per set
but is fixed by the match's seed, so seeding from it keeps a match reproducible:

```python
import random

def step(handle):
    rng = handle.memory.setdefault("rng", random.Random(handle.seed))
    ...
```

## run it locally

```bash
ucbc run mybot mybot --view
```

`ucbc run` plays one bot directory against another, here your bot against itself, and
prints each set's result. `--view` then opens the replay in your browser, the same
viewer the site uses.

- `--seed N` changes the match's seed (default 0)
- `--sets N` changes how many sets are played (default 3)
- `--replay FILE` saves the replay; `ucbc view FILE` opens it later

## debugging with print

Whatever your bot prints is captured into the replay. The viewer shows it beside the
step that printed it, for local matches and for matches run here, and
`ucbc run ... --show-bot-output` prints it in the terminal:

```
[set 1 tick 1 team mybot bot 0] tick 1
```

This is the only way to see inside your bot during a match on the site, so print
freely.

## limits

Matches on the site run under the same limits as `ucbc run`:

- **500 ms per step.** A bot still running at the deadline is paused where it is, and
  the step ends without an action. Loading `main.py` gets the same budget.
- **1 GiB of memory.** Past it, allocations raise `MemoryError`.
- **No files, no network.** Once loaded, a bot cannot open anything.
- **`main.py` only.** It cannot import other files from your bot's directory, or
  third-party packages.

Because nothing can be opened, only these standard-library modules can be imported:

`abc` `array` `bisect` `cmath` `collections` `collections.abc` `contextlib` `copy`
`dataclasses` `decimal` `enum` `fractions` `functools` `heapq` `itertools` `math`
`numbers` `operator` `pprint` `random` `re` `statistics` `string` `textwrap`
`threading` `time` `types` `typing` `weakref`

An upload may be at most 1 MiB zipped and 8 MiB unpacked.

## submit

1. [Create an account](/register), or log in.
2. Zip your bot so that `main.py` is at the top of the zip, not inside a folder:

   ```bash
   cd mybot
   zip -r ../mybot.zip .
   ```

   Without `zip` installed, `python3 -m zipfile -c ../mybot.zip main.py` does the same.
3. On the [home page](/), under **submissions**, choose the zip, optionally give it a
   name, leave the game as `ucbc2027`, and press **upload zip**.
4. Under **matches**, pick one of your team's submissions, pick an opponent (any
   submission, including your own), set a seed if you like, and press **queue match**.

Everyone can see a submission's name, team and uploader, and can queue matches
against it. Nobody else can see its code. To update a bot, upload a new zip:
submissions are never changed once uploaded.

## teams

Every account starts on a team of its own, named after you. To play as a group, one of
you starts a team on the [profile](/profile) page and the others paste its join code
into **join** on theirs. Everyone on a team shares its bots and matches: any member can
queue a match with any of the team's bots, and any member can replace the join code.

Joining or starting a team moves you out of your last one, and its bots and matches
stay with it. Team names are unique and cannot be changed.

## watch the match

A queued match appears under **matches** and moves from `queued` to `running` to
`done`. Expand it to see who won each set and why, and press **watch** to step through
the replay, with your bot's output beside each step. Your own bots and matches
are also listed on your [profile](/profile).
