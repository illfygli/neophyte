mod range;

use crate::{
    rendering::Motion,
    ui::grid::{CellContents, GridContents},
    util::vec2::Vec2,
};
use range::Range;
use std::collections::VecDeque;

pub struct ScrollingGrids {
    scrolling: VecDeque<GridPart>,
    t: f32,
}

impl ScrollingGrids {
    #[allow(unused)]
    pub fn new(grid: GridContents) -> Self {
        let mut scrolling = VecDeque::new();
        scrolling.push_back(GridPart::new(grid));
        Self { scrolling, t: 0. }
    }

    pub fn finish_scroll(&mut self) {
        self.scrolling.drain(1..);
        assert_eq!(self.scrolling.len(), 1);
    }

    pub fn t(&self) -> f32 {
        self.t
    }

    pub fn advance(&mut self, delta_seconds: f32) -> Motion {
        if self.t.abs() < 0.025 {
            self.t = 0.0;
            self.finish_scroll();
            Motion::Still
        } else {
            let sign = if self.t.is_sign_positive() { -1.0 } else { 1.0 };
            let mag = self.t.abs() * delta_seconds + 0.25;
            let mag = mag.ln_1p().powf(1.5);
            let mag = mag.min(self.t.abs());
            self.t += sign * mag;
            Motion::Animating
        }
    }

    pub fn push(&mut self, grid: GridContents, offset: i32) {
        // TODO: Add desired screen region
        let sign = if offset.is_positive() { 1 } else { -1 };
        let mag = offset.abs().min(grid.size.y.into());
        let offset = mag * sign;
        let mut coverage = Range::until(grid.size.y.into());
        self.t += offset as f32;
        self.scrolling.retain_mut(|part| {
            part.offset -= offset;
            let grid_range = Range::until(part.grid.size.y.into()) + part.offset;
            let uncovered = grid_range.cover(coverage);
            coverage = coverage.union(grid_range);
            if let Some(uncovered) = uncovered {
                let grid_range = uncovered - part.offset;
                part.start = grid_range.start.try_into().unwrap();
                part.end = grid_range.end.try_into().unwrap();
                // Useful when resizing the window
                part.grid.size.y == grid.size.y
            } else {
                false
            }
        });
        self.scrolling.push_front(GridPart::new(grid));
    }

    pub fn replace(&mut self, grid: GridContents) {
        *self.scrolling.front_mut().unwrap() = GridPart::new(grid);
    }

    pub fn rows<'a, 'b: 'a>(
        &'a self,
    ) -> impl Iterator<Item = (i32, impl Iterator<Item = CellContents<'a>> + '_ + Clone)> + '_ + Clone
    {
        self.scrolling.iter().rev().flat_map(|part| {
            part.grid
                .rows()
                .enumerate()
                .skip(part.start)
                .take(part.end - part.start)
                .map(|(i, cells)| (i as i32 + part.offset, cells))
        })
    }

    pub fn size(&self) -> Vec2<u16> {
        self.scrolling.back().unwrap().grid.size
    }

    pub fn offset(&self, cell_height: f32) -> Vec2<i32> {
        Vec2::new(0, (self.t() * cell_height) as i32)
    }
}

struct GridPart {
    grid: GridContents,
    offset: i32,
    start: usize,
    end: usize,
}

impl GridPart {
    pub fn new(grid: GridContents) -> Self {
        Self {
            offset: 0,
            start: 0,
            end: grid.size.y as usize,
            grid,
        }
    }
}
