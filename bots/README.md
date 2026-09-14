# Bots

One folder per game. A bot is a directory containing `main.py` that defines
`step(game)`. The engine calls it once per step; for tic-tac-toe that is once per turn,
and `game` is a `ucbc.games.tictactoe.TicTacToeGame`. Place exactly one mark per step. Module globals and
`game.memory` persist across steps within a set; each set starts fresh. `print` output
is captured into the replay and shown with `ucbc run --show-bot-output`.

`main.py` may not import sibling files yet. `tictactoe/random` and
`tictactoe/first_empty` are the reference bots; everything under `testing/` exists to
exercise failure paths.
