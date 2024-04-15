use crossterm::{cursor, ExecutableCommand};
use crossterm::terminal::{self as terminal, Clear, ClearType};
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crate::constants::{MIN_WIDTH, MIN_HEIGHT, EMPTY_TERM_CHAR};
use std::io::Stdout;
use std::io::{stdout, Write};


pub struct Pixel {
    pub color: Color,
}

impl Pixel {
    pub fn new(color: Color) -> Pixel {
        Pixel { color }
    }
    pub fn to_chars(&self) -> Vec<Vec<TermChar>> {
        let char: TermChar = TermChar {
            character: ' ',
            foreground_color: self.color,
            background_color: self.color,
            empty: false,
        };
        vec![vec![char, char]]
    }
}


#[derive(Clone, Copy)]
pub struct TermChar {
    pub character: char,
    pub foreground_color: Color,
    pub background_color: Color,
    pub empty: bool,
}

impl TermChar {
    pub fn draw(&self, term: &mut Stdout, col_row: (i16, i16)) {
        let (col, row) = col_row;
        if col < 0 || row < 0 {
            return;
        }
        
        term.execute(cursor::MoveTo(col as u16, row as u16)).unwrap();
        term.execute(SetForegroundColor(self.foreground_color)).unwrap();
        term.execute(SetBackgroundColor(self.background_color)).unwrap();
        term.execute(Print(self.character.to_string())).unwrap();
    }
}

pub struct Item {
    pub name: String,
    pub offset: (i16, i16), // similar position but it highlights that is relative to the container (0,0)
    pub chars: Vec<Vec<TermChar>>,
}

//implement copy for Item
impl Clone for Item {
    fn clone(&self) -> Item {
        Item {
            name: self.name.clone(),
            offset: self.offset,
            chars: self.chars.clone(),
        }
    }
}


impl Item {
    pub fn draw (&self, term: &mut Stdout, col_row: (i16, i16)) {
        for (char_row, row_vec) in self.chars.iter().enumerate() {
            for (char_col, term_char) in row_vec.iter().enumerate() {
                term_char.draw(term, (char_col as i16 + col_row.0, char_row as i16 + col_row.1));
            }
        }
    }
    pub fn redraw(&self, term: &mut Stdout, c_offset: (i16, i16)) {
        let f_offset = (self.offset.0 + c_offset.0, self.offset.1 + c_offset.1);
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, term_char) in row_vec.iter().enumerate() {
                term_char.draw(term, (f_offset.0 + col as i16, f_offset.1 + row as i16));
            }
        }
    }

    pub fn erase(&self, term: &mut Stdout, c_offset: (i16, i16)) {
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, _) in row_vec.iter().enumerate() {
                EMPTY_TERM_CHAR.draw(term, (self.offset.0 + c_offset.0 + col as i16, self.offset.1 + c_offset.1 + row as i16));
            }
        }
    }
    
    pub fn get_filled_indexes(&self, c_offset: (i16, i16)) -> Vec<(i16, i16)> {
        // c_offset := container offset
        let mut indexes: Vec<(i16, i16)> = Vec::new();
        let f_offset: (i16, i16) = (self.offset.0 + c_offset.0, self.offset.1 + c_offset.1); //final offset
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, term_char) in row_vec.iter().enumerate() {
                if !term_char.empty {
                    indexes.push((f_offset.0 + col as i16, f_offset.1 + row as i16));
                }
            }
        }
        indexes
    }
}


pub struct Layer {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub offset: (i16, i16), // offset with respect to container screen
    pub items: Vec<Item>,
}

impl Layer {
    pub fn new_empty(name: String, width: u16, height: u16, offset: (i16, i16)) -> Layer {
        Layer {name, width, height, offset, items: Vec::new()}
    }

    pub fn relative_position(&self, col: u16, row: u16) -> (i16, i16) {
        // let item_position_on_screen = (col & !(self.screen.layers[0].offset.0 as u16+1%2), row);
        ((col as i16 - self.offset.0), row as i16 - self.offset.1)
    }

    pub fn add_item(&mut self, item: Item) {
        self.items.push(item);
    }
    pub fn remove_item(&mut self, item: Option<&Item>) {
        if let Some(item) = item {
            self.items.retain(|x| x.name != item.name);
        }
    }

    pub fn erase(&self, term: &mut Stdout) {
        for item in self.items.iter() {
            item.erase(term, self.offset);
        }
    }

    pub fn redraw(&mut self, term: &mut Stdout) {
        for item in self.items.iter_mut() {
            item.redraw(term, self.offset);
        }
    }

    pub fn move_layer(&mut self, term: &mut Stdout, displacement: (i16, i16)) {
        self.offset = (self.offset.0 + displacement.0, self.offset.1 + displacement.1);
    }

    pub fn get_filled_indexes(&self) -> Vec<(i16, i16)> {
        let mut indexes = Vec::new();
        for item in self.items.iter() {
            indexes.extend(item.get_filled_indexes(self.offset));
        }
        indexes
    }
    pub fn get_item_at_absolute(&self, abs: (u16, u16)) -> Option<&Item> {
        let casted_index = self.relative_position(abs.0, abs.1);

        for item in self.items.iter() {
            if item.get_filled_indexes(self.offset).contains(&casted_index) {
                return Some(item);
            }
        }
        None
    }
}

pub struct Screen {
    pub width: u16,
    pub height: u16,
    pub layers: Vec<Layer>,
    pub term: std::io::Stdout,
}

impl Screen {
    pub fn new(layers: Vec<Layer>) -> Screen {
        let term = stdout();
        let (width, height): (u16, u16) = terminal::size().unwrap();
        Screen {width, height, layers, term}
    }
    fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }
    fn redraw(&mut self) {
        for layer in self.layers.iter_mut() {
            layer.redraw(&mut self.term);
        }
    }
    fn first_filled_layer_at_index(&self, index: &(u16, u16)) -> Option<usize> {
        let casted_index = (index.0 as i16, index.1 as i16);
        for (i, layer) in self.layers.iter().enumerate() {
            if layer.get_filled_indexes().contains(&casted_index) {
                return Some(i);
            }
        }
        None
    }
    fn first_item_at_index(&self, index: (u16, u16)) -> Option<&Item> {
        for layer in self.layers.iter() {
            if let Some(item) = layer.get_item_at_absolute(index) {
                return Some(item);
            }
        }
        None
    }
    fn index_is_empty(&self, &index: &(u16, u16)) -> bool {
        self.first_filled_layer_at_index(&index).is_none()
    }
}
