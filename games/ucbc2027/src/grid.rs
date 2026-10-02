use std::ops::{Index, IndexMut};

use crate::coord::Coord;

/// A `width` by `height` board of `T`, row-major.
pub struct Grid<T> {
    width: usize,
    height: usize,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn filled(width: usize, height: usize, value: T) -> Self {
        Self {
            width,
            height,
            cells: vec![value; width * height],
        }
    }
}

impl<T> Grid<T> {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// `None` off the board.
    pub fn get(&self, at: Coord) -> Option<&T> {
        self.index(at).map(|i| &self.cells[i])
    }

    pub fn rows(&self) -> impl Iterator<Item = &[T]> {
        self.cells.chunks(self.width)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Coord, &T)> {
        let width = self.width;
        self.cells
            .iter()
            .enumerate()
            .map(move |(i, cell)| (Coord::new(i % width, i / width), cell))
    }

    fn index(&self, at: Coord) -> Option<usize> {
        (at.x < self.width && at.y < self.height).then_some(at.y * self.width + at.x)
    }
}

/// Panics off the board.
impl<T> Index<Coord> for Grid<T> {
    type Output = T;

    fn index(&self, at: Coord) -> &T {
        self.get(at)
            .unwrap_or_else(|| panic!("({}, {}) is off the board", at.x, at.y))
    }
}

/// Panics off the board.
impl<T> IndexMut<Coord> for Grid<T> {
    fn index_mut(&mut self, at: Coord) -> &mut T {
        let i = self
            .index(at)
            .unwrap_or_else(|| panic!("({}, {}) is off the board", at.x, at.y));
        &mut self.cells[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_row_major_and_rejects_off_board() {
        let mut grid = Grid::filled(3, 2, 0);
        grid[Coord::new(2, 1)] = 5;
        assert_eq!(grid.get(Coord::new(2, 1)), Some(&5));
        assert_eq!(grid.get(Coord::new(3, 0)), None);
        assert_eq!(grid.get(Coord::new(0, 2)), None);
        let rows: Vec<&[i32]> = grid.rows().collect();
        assert_eq!(rows, [&[0, 0, 0][..], &[0, 0, 5][..]]);
        assert_eq!(grid.iter().last(), Some((Coord::new(2, 1), &5)));
    }
}
