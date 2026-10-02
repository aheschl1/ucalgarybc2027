use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, Answer, BotFailure, BotId, BotManager, EngineError, Game, GameStatus, Outcome,
    QueryError, Request, SetSetup, TeamId,
};

use crate::rules::{Board, Cell};

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Queries {
    /// The board and whose turn it is.
    Board(Request<(), BoardView>),
}

/// What a bot sees when it asks for the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BoardView {
    #[serde(flatten)]
    pub board: Board,
    /// The mark this bot plays.
    pub you: Cell,
    pub to_move: Cell,
    /// Marks placed so far this set.
    pub turn: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Place this bot's mark. Refused if the cell is taken or a mark was already
    /// placed this step.
    Place(Request<Spot, Placed>),
}

#[derive(Deserialize, JsonSchema)]
pub struct Spot {
    pub row: u32,
    pub col: u32,
}

/// The mark that was placed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Placed {
    pub row: u32,
    pub col: u32,
    pub mark: Cell,
}

enum State {
    /// Whether the stepping bot has placed its mark this step.
    Playing {
        placed: bool,
    },
    Over(Outcome),
}

/// Two teams, one bot each. Every tick both bots step, the set's first team first,
/// and that team plays X.
pub struct TicTacToe {
    bots: BotManager<()>,
    board: Board,
    /// Step order; `marks[i]` is the mark of `order[i]`.
    order: [TeamId; 2],
    marks: [Cell; 2],
    state: State,
}

impl TicTacToe {
    fn opponent(team: TeamId) -> TeamId {
        TeamId(1 - team.0)
    }

    fn mark(&self, team: TeamId) -> Cell {
        self.marks[usize::from(team != self.order[0])]
    }

    fn turn(&self) -> u32 {
        9 - self.board.empty_cells().len() as u32
    }

    /// A forfeit only ends a set still in play; a completed outcome stands.
    fn forfeit(&mut self, loser: TeamId, detail: String) {
        if matches!(self.state, State::Playing { .. }) {
            self.state = State::Over(Outcome::forfeit(Self::opponent(loser), detail));
        }
    }
}

impl Game for TicTacToe {
    const NAME: &'static str = "tictactoe";
    type Query = Queries;
    type Action = Action;
    type Snapshot = Board;
    type Bot = ();

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        let &[first, second] = setup.teams.as_slice() else {
            return Err(EngineError::Config(format!(
                "tictactoe needs exactly 2 teams, got {}",
                setup.teams.len()
            )));
        };
        let mut bots = BotManager::new();
        bots.spawn(first, ());
        bots.spawn(second, ());
        Ok(TicTacToe {
            bots,
            board: Board::new(),
            order: [first, second],
            marks: [Cell::X, Cell::O],
            state: State::Playing { placed: false },
        })
    }

    fn bots(&self) -> &BotManager<()> {
        &self.bots
    }

    fn bots_mut(&mut self) -> &mut BotManager<()> {
        &mut self.bots
    }

    fn handle_query(&self, bot: BotId, query: Queries) -> Result<Answer, QueryError> {
        match query {
            Queries::Board(q) => Ok(q.reply(BoardView {
                board: self.board,
                you: self.mark(self.bots[bot].team()),
                to_move: self.marks[(self.turn() % 2) as usize],
                turn: self.turn(),
            })),
        }
    }

    fn apply_action(&mut self, bot: BotId, action: Action) -> Result<Answer, ActionError> {
        if matches!(self.state, State::Playing { placed: true }) {
            return Err(ActionError::Invalid("already placed this turn".into()));
        }
        let Action::Place(a) = action;
        let (row, col) = (a.row, a.col);
        let team = self.bots[bot].team();
        let mark = self.mark(team);
        self.board
            .place(row, col, mark)
            .map_err(|e| ActionError::Invalid(e.to_string()))?;
        self.state = if self.board.winner() == Some(mark) {
            State::Over(Outcome::win(team))
        } else if self.board.is_full() {
            State::Over(Outcome::draw("board full"))
        } else {
            State::Playing { placed: true }
        };
        Ok(a.reply(Placed { row, col, mark }))
    }

    fn end_step(&mut self, bot: BotId) {
        let team = self.bots[bot].team();
        match self.state {
            State::Playing { placed: true } => self.state = State::Playing { placed: false },
            State::Playing { placed: false } => {
                self.forfeit(team, format!("team {team} did not place a mark"));
            }
            State::Over(_) => {}
        }
    }

    fn bot_failed(&mut self, bot: BotId, failure: &BotFailure) {
        let team = self.bots[bot].team();
        self.forfeit(team, format!("team {team}: {}", failure.detail()));
    }

    fn end_tick(&mut self) {}

    fn status(&self) -> GameStatus {
        match &self.state {
            State::Playing { .. } => GameStatus::InProgress,
            State::Over(outcome) => GameStatus::Complete(outcome.clone()),
        }
    }

    fn snapshot(&self) -> Board {
        self.board
    }
}
