use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Cell {
    Empty,
    X,
    O,
}

impl From<Cell> for u8 {
    fn from(c: Cell) -> u8 {
        c as u8
    }
}

impl TryFrom<u8> for Cell {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Cell::Empty),
            1 => Ok(Cell::X),
            2 => Ok(Cell::O),
            _ => Err(format!("cell value out of range: {v}")),
        }
    }
}

impl Cell {
    pub fn symbol(self) -> char {
        match self {
            Cell::Empty => '.',
            Cell::X => 'X',
            Cell::O => 'O',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("cell ({0}, {1}) is off the board")]
    OutOfBounds(u32, u32),
    #[error("cell ({0}, {1}) is occupied")]
    Occupied(u32, u32),
}

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

/// 3x3 board, row-major.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub cells: [Cell; 9],
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    pub fn new() -> Self {
        Self {
            cells: [Cell::Empty; 9],
        }
    }

    pub fn get(&self, row: u32, col: u32) -> Option<Cell> {
        (row < 3 && col < 3).then(|| self.cells[(row * 3 + col) as usize])
    }

    pub fn place(&mut self, row: u32, col: u32, mark: Cell) -> Result<(), PlaceError> {
        match self.get(row, col) {
            None => Err(PlaceError::OutOfBounds(row, col)),
            Some(Cell::Empty) => {
                self.cells[(row * 3 + col) as usize] = mark;
                Ok(())
            }
            Some(_) => Err(PlaceError::Occupied(row, col)),
        }
    }

    pub fn winner(&self) -> Option<Cell> {
        LINES.iter().find_map(|line| {
            let first = self.cells[line[0]];
            (first != Cell::Empty && line.iter().all(|&i| self.cells[i] == first)).then_some(first)
        })
    }

    pub fn is_full(&self) -> bool {
        self.cells.iter().all(|&c| c != Cell::Empty)
    }

    pub fn empty_cells(&self) -> Vec<(u32, u32)> {
        (0..9u32)
            .filter(|&i| self.cells[i as usize] == Cell::Empty)
            .map(|i| (i / 3, i % 3))
            .collect()
    }

    /// Three lines of `X`, `O`, and `.`.
    pub fn render(&self) -> String {
        self.cells
            .chunks(3)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(cells: &str) -> Board {
        let mut b = Board::new();
        for (i, ch) in cells.chars().enumerate() {
            b.cells[i] = match ch {
                'X' => Cell::X,
                'O' => Cell::O,
                _ => Cell::Empty,
            };
        }
        b
    }

    #[test]
    fn every_line_wins() {
        for line in LINES {
            let mut b = Board::new();
            for i in line {
                b.cells[i] = Cell::O;
            }
            assert_eq!(b.winner(), Some(Cell::O), "{line:?}");
        }
        assert_eq!(filled("XOXXOOOXX").winner(), None);
    }

    #[test]
    fn draw_is_full_without_winner() {
        let b = filled("XOXXOOOXX");
        assert!(b.is_full());
        assert_eq!(b.winner(), None);
        assert!(b.empty_cells().is_empty());
    }

    #[test]
    fn place_validates() {
        let mut b = Board::new();
        b.place(1, 1, Cell::X).unwrap();
        assert_eq!(b.place(1, 1, Cell::O), Err(PlaceError::Occupied(1, 1)));
        assert_eq!(b.place(3, 0, Cell::O), Err(PlaceError::OutOfBounds(3, 0)));
        assert_eq!(b.empty_cells().len(), 8);
    }

    #[test]
    fn cells_serialize_as_ints() {
        let b = filled("X.O......");
        assert_eq!(
            serde_json::to_string(&b).unwrap(),
            r#"{"cells":[1,0,2,0,0,0,0,0,0]}"#
        );
        let back: Board = serde_json::from_str(r#"{"cells":[1,0,2,0,0,0,0,0,0]}"#).unwrap();
        assert_eq!(back, b);
        assert!(serde_json::from_str::<Board>(r#"{"cells":[7,0,0,0,0,0,0,0,0]}"#).is_err());
    }

    #[test]
    fn renders() {
        assert_eq!(filled("X.O.X.O.X").render(), "X.O\n.X.\nO.X");
    }
}
