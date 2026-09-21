# ucbc-sdk

UCalgary Battlecode: the pure-Python `ucbc` package a bot imports, one typed handle per
game. `pip install ucbc` brings it along with the engine and the `ucbc` command (Linux);
install `ucbc-sdk` alone for types and completion on any platform.

```python
from ucbc.games.tictactoe import TicTacToeHandle


def step(handle: TicTacToeHandle) -> None:
    handle.place(*handle.empty_cells()[0])
```

https://ucbc.andrewheschl.ca
