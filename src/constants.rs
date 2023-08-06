use crate::screen::TermChar;
use crossterm::style::Color;


pub const MIN_WIDTH: u16 = 72;
pub const MIN_HEIGHT: u16 = 30;
pub const EMPTY_TERM_CHAR: TermChar = TermChar {character: ' ', foreground_color: Color::Reset, background_color: Color::Reset, empty: true};
