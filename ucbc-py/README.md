# ucbc

UCalgary Battlecode: the match engine, the bot runtime, and the `ucbc` command. Installs
`ucbc-sdk`, the package bots import, alongside.

```bash
pip install ucbc        # Linux x86_64 and aarch64, Python 3.12+
```

The engine sandboxes bots with Linux facilities, so there are no macOS or Windows wheels:
use WSL2 or a Linux container there. `pip install ucbc-sdk` works anywhere, for types and
completion.

A bot is a directory with a `main.py` defining `step`:

```python
from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
```

```bash
ucbc run path/to/mine path/to/other --view
```
