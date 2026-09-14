use serde::{Deserialize, Serialize};
use ucbc_engine::{
    ActionError, BotFailure, BotRef, EngineError, Game, GameStatus, Outcome, QueryError, SetSetup,
    TeamId,
};

use crate::rules::{Board, Cell};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Query {
    Board,
}

/// What a bot sees when it asks for the board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardView {
    #[serde(flatten)]
    pub board: Board,
    pub you: Cell,
    pub to_move: Cell,
    pub turn: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Place { row: u32, col: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// Two teams, one bot each (bot id equals team id). Every tick both bots step, the
/// set's first team first, and that team plays X.
pub struct TicTacToe {
    board: Board,
    /// Step order; `marks[i]` is the mark of `order[i]`.
    order: [TeamId; 2],
    marks: [Cell; 2],
    state: State,
}

impl TicTacToe {
    fn bot_of(team: TeamId) -> BotRef {
        BotRef::new(u64::from(team.0), team.0)
    }

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
    type Query = Query;
    type QueryResponse = BoardView;
    type Action = Action;
    type ActionResponse = Placed;
    type Snapshot = Board;

    fn create(setup: &SetSetup) -> Result<Self, EngineError> {
        if setup.teams != 2 {
            return Err(EngineError::Config(format!(
                "tictactoe needs exactly 2 teams, got {}",
                setup.teams
            )));
        }
        let first = setup.first_team;
        Ok(TicTacToe {
            board: Board::new(),
            order: [first, Self::opponent(first)],
            marks: [Cell::X, Cell::O],
            state: State::Playing { placed: false },
        })
    }

    fn schedule(&mut self) -> Vec<BotRef> {
        self.order.iter().map(|&t| Self::bot_of(t)).collect()
    }

    fn handle_query(&self, bot: BotRef, query: Query) -> Result<BoardView, QueryError> {
        match query {
            Query::Board => Ok(BoardView {
                board: self.board,
                you: self.mark(bot.team),
                to_move: self.marks[(self.turn() % 2) as usize],
                turn: self.turn(),
            }),
        }
    }

    fn apply_action(&mut self, bot: BotRef, action: Action) -> Result<Placed, ActionError> {
        if matches!(self.state, State::Playing { placed: true }) {
            return Err(ActionError::Invalid("already placed this turn".into()));
        }
        let Action::Place { row, col } = action;
        let mark = self.mark(bot.team);
        self.board
            .place(row, col, mark)
            .map_err(|e| ActionError::Invalid(e.to_string()))?;
        self.state = if self.board.winner() == Some(mark) {
            State::Over(Outcome::win(bot.team))
        } else if self.board.is_full() {
            State::Over(Outcome::draw("board full"))
        } else {
            State::Playing { placed: true }
        };
        Ok(Placed { row, col, mark })
    }

    fn end_step(&mut self, bot: BotRef) {
        match self.state {
            State::Playing { placed: true } => self.state = State::Playing { placed: false },
            State::Playing { placed: false } => {
                self.forfeit(bot.team, format!("team {} did not place a mark", bot.team));
            }
            State::Over(_) => {}
        }
    }

    fn bot_failed(&mut self, bot: BotRef, failure: &BotFailure) {
        self.forfeit(bot.team, format!("team {}: {}", bot.team, failure.detail()));
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
