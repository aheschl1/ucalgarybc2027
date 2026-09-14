//! Tic-tac-toe on the ucbc engine: one bot per team, one move per tick. This crate is
//! the game only; bots live outside it.

pub mod game;
pub mod rules;

pub use game::{Action, BoardView, Placed, Query, TicTacToe};
pub use rules::{Board, Cell, PlaceError};
