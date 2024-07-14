use crate::screen::TermChar;
use crossterm::style::Color;

pub const MAX_FAILED_SENT_ON_QUEUE: usize = 16;
pub const EMPTY_TERM_CHAR: TermChar = TermChar {
    character: ' ',
    foreground_color: Color::Reset,
    background_color: Color::Reset,
    empty: true,
};
