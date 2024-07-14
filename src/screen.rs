use crossterm::{cursor, ExecutableCommand};
use crossterm::terminal::{self as terminal};
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crate::constants::EMPTY_TERM_CHAR;
use std::io::Stdout;
use std::io::stdout;


pub struct Pixel {
    pub color: Color,
}

#[allow(dead_code)]
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
    pub fn draw(&self, term: &mut Stdout, col_row: (i32, i32), width: u16, height: u16) {
        let (col, row) = col_row;
        if col < 0 || row < 0 {
            return;
        }
        if col >= width as i32 || row >= height as i32 {
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
    // items are contained in a layer and they have an offset with respect to it.
    // similar position but it highlights that is relative to the container (0,0)
    // offset is container_relative_xy
    pub offset: (i32, i32), 
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
    // TODO: this should improve, I need to basically return buffers, containing a "string" made
    // up of the characters that the Item is made of
    pub fn draw (&self, term: &mut Stdout, col_row: (i32, i32), width: u16, height: u16) {
        for (char_row, row_vec) in self.chars.iter().enumerate() {
            for (char_col, term_char) in row_vec.iter().enumerate() {
                term_char.draw(term, (char_col as i32 + col_row.0, char_row as i32 + col_row.1), width, height)
            }
        }
    }
    pub fn redraw(&self, term: &mut Stdout, c_offset: (i32, i32), width: u16, height: u16) {
        let f_offset = (self.offset.0 + c_offset.0, self.offset.1 + c_offset.1);
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, term_char) in row_vec.iter().enumerate() {
                term_char.draw(term, (f_offset.0 + col as i32, f_offset.1 + row as i32), width, height);
            }
        }
    }

    pub fn draw_buffer(&self, buffer: &mut [Vec<String>], c_offset: (i32, i32), width: u16, height: u16){
        let f_offset: (i32, i32) = (self.offset.0 + c_offset.0, self.offset.1 + c_offset.1);
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, term_char) in row_vec.iter().enumerate() {
                let x = f_offset.0 + (col as i32);
                let y = f_offset.1 + (row as i32);
                //println!("{}-{}", x,y);
                if x < 0 || y < 0 || y >= height as i32 || x >= width as i32 {
                    continue;
                }
                if let Color::AnsiValue(c) = term_char.background_color {
                    buffer[y as usize][x as usize] = format!("\x1b[48;5;{}m \x1b[49m", c)
                }
            }
        }
    }

    // each item is essentially a matrix of chars
    // this matrix is represented as Vec<Vec<TermChar>> where the rows
    // are the y direction and the columns are the x direction
    // the screen position returns the absolute position of the starting, (0,0)
    // the value returned may be out of the screen, so it is up to the caller to check
    pub fn screen_position(&self, containers_offsets: Vec<(i32, i32)>) -> (i32, i32) {
        let mut x: i32 = self.offset.0;
        let mut y: i32 = self.offset.1;
        for (c_x, c_y) in containers_offsets.iter() {
            x += c_x;
            y += c_y;
        }
        (x, y)
    }

    // this is draw_erase, it will draw the empty char in the position of the item
    pub fn erase(&self, term: &mut Stdout, c_offset: (i32, i32), width: u16, height: u16) {
        let (x0, y0) = self.screen_position(vec![c_offset]);
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, _) in row_vec.iter().enumerate() {
                EMPTY_TERM_CHAR.draw(
                    term, 
                    // item_relative_to_container_layer + container_relative_to_screen
                    (x0 + col as i32, y0 + row as i32),
                    width, 
                    height
                );
            }
        }
    }
    
    pub fn get_filled_indexes(&self, c_offset: (i32, i32)) -> Vec<(i32, i32)> {
        let mut indexes: Vec<(i32, i32)> = Vec::new();
        let (x0, y0) = self.screen_position(vec![c_offset]);
        for (row, row_vec) in self.chars.iter().enumerate() {
            for (col, term_char) in row_vec.iter().enumerate() {
                if !term_char.empty {
                    indexes.push((x0 + col as i32, y0 + row as i32));
                }
            }
        }
        indexes
    }
}


#[allow(dead_code)]
pub struct Layer {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub offset: (i32, i32), // offset with respect to container screen
    pub items: Vec<Item>,
}

#[allow(dead_code)]
impl Layer {
    pub fn new_empty(name: String, width: u16, height: u16, offset: (i32, i32)) -> Layer {
        Layer {name, width, height, offset, items: Vec::new()}
    }

    // relative position of (col, row) to the self
    pub fn relative_position(&self, col: u16, row: u16) -> (i32, i32) {
        (col as i32 - self.offset.0, row as i32 - self.offset.1)
    }

    pub fn add_item(&mut self, item: Item) {
        self.items.push(item);
    }
    
    pub fn remove_item(&mut self, item: Option<&Item>) {
        if let Some(item) = item {
            self.items.retain(|x| x.name != item.name);
        }
    }

    pub fn buffer_to_string(&mut self, buffer: Vec<Vec<String>>) -> String {
        buffer.into_iter().flatten().collect()
    }

    pub fn draw_buffer(&mut self, term: &mut Stdout, width: u16, height: u16){
        let mut buffer: Vec<Vec<String>> = vec![vec![' '.to_string(); width as usize]; height as usize];
        for item in self.items.iter_mut() {
            item.draw_buffer(&mut buffer, self.offset, width, height);
        }
        let layer_str: String = self.buffer_to_string(buffer);
        term.execute(cursor::MoveTo(0, 0)).unwrap();
        term.execute(Print(layer_str)).unwrap();
    }   
    
    pub fn redraw(&mut self, term: &mut Stdout, width: u16, height: u16) {
        for item in self.items.iter_mut() {
            item.redraw(term, self.offset, width, height);
        }
    }

    pub fn move_layer(&mut self,  displacement: (i32, i32)) {
        self.offset = (self.offset.0 + displacement.0, self.offset.1 + displacement.1);
    }

    pub fn get_filled_indexes(&self) -> Vec<(i32, i32)> {
        let mut indexes = Vec::new();
        for item in self.items.iter() {
            indexes.extend(item.get_filled_indexes(self.offset));
        }
        indexes
    }
    pub fn get_item_at_absolute(&self, (abs_x, abs_y): (i32, i32)) -> Option<&Item> {
        self.items
            .iter()
            .find(|&item| item.get_filled_indexes(self.offset).contains(&(abs_x, abs_y)))
    }

}

pub struct Screen {
    pub width: u16,
    pub height: u16,
    pub layers: Vec<Layer>,
    pub term: std::io::Stdout,
}

#[allow(dead_code)]
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
            layer.redraw(&mut self.term, self.width, self.height);
        }
    }
    fn first_filled_layer_at_index(&self, index: &(u16, u16)) -> Option<usize> {
        let casted_index = (index.0 as i32, index.1 as i32);
        for (i, layer) in self.layers.iter().enumerate() {
            if layer.get_filled_indexes().contains(&casted_index) {
                return Some(i);
            }
        }
        None
    }
    
    fn first_item_at_col_row(&self, (col, row): (u16, u16)) -> Option<&Item> {
        for layer in self.layers.iter() {
            if let Some(item) = layer.get_item_at_absolute((col as i32, row as i32)) {
                return Some(item);
            }
        }
        None
    }
    fn index_is_empty(&self, &index: &(u16, u16)) -> bool {
        self.first_filled_layer_at_index(&index).is_none()
    }
}
