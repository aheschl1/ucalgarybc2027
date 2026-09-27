# ucbc

UCalgary Battlecode: the package bots import, the match engine, and the `ucbc` command.

```bash
pip install ucbc        # Linux x86_64 and aarch64, Python 3.12+
```

The engine sandboxes bots with Linux facilities, so there are no macOS or Windows wheels:
use WSL2 or a Linux container there.

A bot is a directory with a `main.py` defining `step`:

```python
from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    handle.noop()
```

```bash
ucbc run path/to/mine path/to/other --view
```
