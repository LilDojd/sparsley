//! Box-drawing glyphs for borders that mix light and heavy lines.

/// The weight of a line leaving a junction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Line {
    Blank,
    Light,
    Heavy,
}

impl Line {
    pub(crate) fn weight(heavy: bool) -> Self {
        if heavy { Self::Heavy } else { Self::Light }
    }
}

/// Returns the glyph whose lines leave up, down, left and right with the
/// given weights.
pub(crate) fn glyph(up: Line, down: Line, left: Line, right: Line) -> char {
    GLYPHS
        .iter()
        .find(|(_, arms)| *arms == [up, down, left, right])
        .map(|&(glyph, _)| glyph)
        .expect("no box-drawing glyph for these arms")
}

use Line::{Blank as B, Heavy as H, Light as L};

/// Glyphs and their arms (up, down, left, right), generated from the
/// Unicode character names.
#[rustfmt::skip]
const GLYPHS: [(char, [Line; 4]); 68] = [
    ('─', [B, B, L, L]),
    ('━', [B, B, H, H]),
    ('│', [L, L, B, B]),
    ('┃', [H, H, B, B]),
    ('┌', [B, L, B, L]),
    ('┍', [B, L, B, H]),
    ('┎', [B, H, B, L]),
    ('┏', [B, H, B, H]),
    ('┐', [B, L, L, B]),
    ('┑', [B, L, H, B]),
    ('┒', [B, H, L, B]),
    ('┓', [B, H, H, B]),
    ('└', [L, B, B, L]),
    ('┕', [L, B, B, H]),
    ('┖', [H, B, B, L]),
    ('┗', [H, B, B, H]),
    ('┘', [L, B, L, B]),
    ('┙', [L, B, H, B]),
    ('┚', [H, B, L, B]),
    ('┛', [H, B, H, B]),
    ('├', [L, L, B, L]),
    ('┝', [L, L, B, H]),
    ('┞', [H, L, B, L]),
    ('┟', [L, H, B, L]),
    ('┠', [H, H, B, L]),
    ('┡', [H, L, B, H]),
    ('┢', [L, H, B, H]),
    ('┣', [H, H, B, H]),
    ('┤', [L, L, L, B]),
    ('┥', [L, L, H, B]),
    ('┦', [H, L, L, B]),
    ('┧', [L, H, L, B]),
    ('┨', [H, H, L, B]),
    ('┩', [H, L, H, B]),
    ('┪', [L, H, H, B]),
    ('┫', [H, H, H, B]),
    ('┬', [B, L, L, L]),
    ('┭', [B, L, H, L]),
    ('┮', [B, L, L, H]),
    ('┯', [B, L, H, H]),
    ('┰', [B, H, L, L]),
    ('┱', [B, H, H, L]),
    ('┲', [B, H, L, H]),
    ('┳', [B, H, H, H]),
    ('┴', [L, B, L, L]),
    ('┵', [L, B, H, L]),
    ('┶', [L, B, L, H]),
    ('┷', [L, B, H, H]),
    ('┸', [H, B, L, L]),
    ('┹', [H, B, H, L]),
    ('┺', [H, B, L, H]),
    ('┻', [H, B, H, H]),
    ('┼', [L, L, L, L]),
    ('┽', [L, L, H, L]),
    ('┾', [L, L, L, H]),
    ('┿', [L, L, H, H]),
    ('╀', [H, L, L, L]),
    ('╁', [L, H, L, L]),
    ('╂', [H, H, L, L]),
    ('╃', [H, L, H, L]),
    ('╄', [H, L, L, H]),
    ('╅', [L, H, H, L]),
    ('╆', [L, H, L, H]),
    ('╇', [H, L, H, H]),
    ('╈', [L, H, H, H]),
    ('╉', [H, H, H, L]),
    ('╊', [H, H, L, H]),
    ('╋', [H, H, H, H]),
];
