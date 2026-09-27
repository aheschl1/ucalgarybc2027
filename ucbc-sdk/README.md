# ucbc-sdk

UCalgary Battlecode: the pure-Python `ucbc` package a bot imports, one typed handle per
game. `pip install ucbc` brings it along with the engine and the `ucbc` command (Linux);
install `ucbc-sdk` alone for types and completion on any platform.

```python
from ucbc.games.ucbc2027 import Ucbc2027Handle


def step(handle: Ucbc2027Handle) -> None:
    handle.noop()
```

