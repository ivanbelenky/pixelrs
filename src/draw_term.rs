use std::io::Write;
use std::time::Duration;
use crossterm::cursor;
use crossterm::cursor::MoveTo;
use crossterm::terminal as terminal;
use crossterm::ExecutableCommand;
use crossterm::style::Color;
use crossterm::event::{self as event, MouseEvent, KeyEventKind, MouseButton, MouseEventKind, KeyCode, KeyEvent};
use crate::screen::TermChar;
use crate::screen::{Screen, Item, Layer, Pixel};
use crate::constants::EMPTY_TERM_CHAR;


#[derive(PartialEq)]
enum Tool {
    BRUSH,
    ERASE,
    INK,
    MOVE,
    TEXT
}

#[derive(PartialEq)]
enum Config {
    NONE,
    COLORSELECTION
}

pub struct DrawTerm {
    screen: Screen,
    tool: Tool,
    config: Config,
    cursor: Item,
    cursor_info: Item,
    resized: bool,
    typing: bool,
    color_selected: Color,
    last_cursor_position: (u16, u16)
}


impl DrawTerm {
    pub fn new() -> Self {
        let (width, height): (u16, u16) = terminal::size().unwrap();
        let foreground: Layer = Layer::new_empty("foreground".to_string(), width, height, (0, 0));
        let background: Layer = Layer::new_empty("background".to_string(), width, height, (0, 0));
        let screen: Screen = Screen::new(vec![background, foreground]);
        let tool: Tool = Tool::BRUSH;
        let config: Config = Config::NONE;
        
        let cursor: Item = Item { name: "cursor".to_string(), offset: (width as i16-1, 0), chars: vec![vec![EMPTY_TERM_CHAR]] };
        let cursor_info: Item = Item {name: "cursor_info".to_string(), offset: (width as i16 - 9, height as i16-1), chars: vec![vec![EMPTY_TERM_CHAR]]};
        let color_selected: Color = Color::AnsiValue(0);
        let last_cursor_position: (u16, u16) = (0, 0);
        let resized: bool = false;
        let typing: bool = false;
        DrawTerm { screen, tool, config, cursor, cursor_info, resized, typing, color_selected, last_cursor_position}
    }
    pub fn run(&mut self) {
        self._enter();
        let mut exit = false;
        while !exit{
            if event::poll(Duration::ZERO).unwrap() {
                match event::read().unwrap() {
                    event::Event::Key(event) => exit = self.on_key_event(event),
                    event::Event::Mouse(event) => exit = self.on_mouse_event(event),
                    event::Event::Resize(width, height) => exit = self.on_resize_event(width, height),
                    _ => {}
                }
                self.draw_layers()
            }
        }
        self._exit();
    }
    fn _enter(&mut self){
        terminal::enable_raw_mode().unwrap();
        self.screen.term.execute(event::EnableMouseCapture).unwrap();
        self.screen.term.execute(cursor::Hide).unwrap();
        self.clear_screen();
    }
    fn _exit(&mut self) {
        self.screen.term.execute(MoveTo(0, self.screen.height)).unwrap();
        self.screen.term.execute(event::DisableMouseCapture).unwrap();
        self.screen.term.execute(cursor::Show).unwrap();
        terminal::disable_raw_mode().unwrap();
        
    }
    pub fn draw_layers(&mut self) {
        //
    }
    pub fn clear_screen(&mut self) {
        self.screen.term.execute(terminal::Clear(terminal::ClearType::All)).unwrap();
        self.screen.term.flush().unwrap();
    }
    pub fn draw_ansi_colors(&mut self) { 
        self.config = Config::COLORSELECTION;
        for c in 0..16 {              
            let color_pixel: Item = Item {name: "color_selection_pixels".to_string(), offset: (2*c, self.screen.height as i16-1), chars: Pixel{color: Color::AnsiValue(c as u8)}.to_chars()};
            self.screen.layers[1].add_item(color_pixel.clone());
            // TODO, the line below should be replaced by basically pushing data to a buffer 2d buffer
            // at first probably you are going to plot back again here, instead of replacing the 
            // this function with just one that returns the buffer, or actually modifies it inplace + 
            // returns it for the sake of who knows what.
            color_pixel.draw(&mut self.screen.term, (2*c, self.screen.height as i16-1), self.screen.width, self.screen.height);
        }

    }
    pub fn erase_ansi_colors(&mut self) {
        self.config = Config::NONE;
        self.screen.layers[1].items.retain(|item| item.name != "color_selection_pixels");
        for c in 0..32 {
            EMPTY_TERM_CHAR.draw(&mut self.screen.term, (c, self.screen.height as i16 - 1), self.screen.width, self.screen.height);
        }
    }
    
    pub fn cursor_term_char(&self) -> TermChar {
        match self.tool {
            Tool::BRUSH => { 
                let mut fg_color = self.color_selected;
                if self.color_selected == Color::AnsiValue(0){ fg_color = Color::White };    
                TermChar {
                    character: 'B',
                    foreground_color: fg_color,
                    background_color: Color::Reset,
                    empty: false,
                }
            },
            Tool::ERASE => TermChar {
                character: 'E',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::INK => TermChar {
                character: 'I',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::MOVE => TermChar {
                character: 'M',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::TEXT => TermChar {
                character: 'T',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
        }
    }
    pub fn create_cursor_info_chars(&self, (col, row): (i16, i16)) -> Vec<Vec<TermChar>> {
        // make col and row //2 values
        let col = col/2;
        let row  = row;
        let cursor_info_str: String = format!("{:04} {:04}", col, row);
        let mut chars: Vec<TermChar> = Vec::new();
        for c in cursor_info_str.chars() {
            chars.push(TermChar {
                character: c,
                foreground_color: Color::Reset,
                background_color: Color::Reset,
                empty: false,
            });
        }
        return vec![chars];
    }

}


pub trait EventHandlers {
    // event handlers must return bool | null
    fn on_key_event(&mut self, event: KeyEvent) -> bool;
    fn on_mouse_event(&mut self, event: MouseEvent) -> bool;
    fn on_resize_event(&mut self, width: u16, height: u16) -> bool;
}


impl EventHandlers for DrawTerm {
    fn on_key_event(&mut self, event: KeyEvent) -> bool {
        if self.typing == true {
            match event.code {
                KeyCode::Char(c) => {
                    let char: Item = Item {
                        name: "char".to_string(), 
                        offset: self.screen.layers[0].relative_position(self.last_cursor_position.0 , self.last_cursor_position.1), 
                        chars: vec![vec![TermChar {character: c, foreground_color: self.color_selected, background_color: Color::Reset, empty: false}, EMPTY_TERM_CHAR]]
                    };
                    self.screen.layers[0].add_item(char.clone());
                    char.draw(&mut self.screen.term, (self.last_cursor_position.0 as i16, self.last_cursor_position.1 as i16), self.screen.width, self.screen.height);
                    self.last_cursor_position = (self.last_cursor_position.0+2, self.last_cursor_position.1);
                    self.screen.term.execute(MoveTo(self.last_cursor_position.0, self.last_cursor_position.1)).unwrap();
                },
                KeyCode::Enter | KeyCode::Esc => {
                    self.typing = false;
                    self.tool = Tool::BRUSH;
                    self.screen.term.execute(cursor::Hide).unwrap();
                },
                KeyCode::Backspace => {
                    let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((self.last_cursor_position.0-2, self.last_cursor_position.1));
                    match item {
                        Some(item) => {
                            item.erase(&mut self.screen.term, self.screen.layers[0].offset, self.screen.width, self.screen.height);
                            let items: Vec<Item> = self.screen.layers[0].items.clone();
                            self.screen.layers[0].items = items.into_iter().filter(|i| i.offset != item.offset).collect();
                            self.last_cursor_position = (self.last_cursor_position.0-2, self.last_cursor_position.1);
                            self.screen.term.execute(MoveTo(self.last_cursor_position.0, self.last_cursor_position.1)).unwrap();
                        },
                        None => {}
                    }
                },
                _ => {}
            }
            
            return false;
        }
        match event.kind {
            KeyEventKind::Press => {
                match event.code {
                    KeyCode::Char(c) => {
                        match c {
                            'q' => true,
                            'e' => {
                                self.tool = Tool::ERASE;
                                false
                            },
                            'b' => {
                                self.tool = Tool::BRUSH;
                                false
                            },
                            'i' => {
                                self.tool = Tool::INK;
                                false
                            }
                            'c' => {
                                if self.config == Config::COLORSELECTION {
                                    self.erase_ansi_colors();
                                    return false;
                                };
                                if self.tool == Tool::ERASE {self.tool = Tool::BRUSH};
                                self.draw_ansi_colors();
                                return false
                            },
                            'm' => {
                                self.tool = Tool::MOVE;
                                false
                            },
                            'a' => {
                                self.tool = Tool::TEXT;
                                false
                            },
                            _ => false,
                        }
                    },
                    _ => false,
                }
            },
            _ => false
        }
    }
    fn on_mouse_event(&mut self, event: MouseEvent) -> bool {
        let (col, row) = (event.column.clone() & !(event.column%2), event.row.clone());
        self.screen.term.execute(MoveTo(col, row)).unwrap();

        if self.resized {
            self.resized = false;
            self.screen.layers[0].redraw(&mut self.screen.term, self.screen.width, self.screen.height);
            self.screen.layers[1].redraw(&mut self.screen.term, self.screen.width, self.screen.height);       
        }

        let item_on_foreground = self.screen.layers[1].get_item_at_absolute((col, row));
        

        match event.kind {
            event::MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(event::MouseButton::Left) => {
                if item_on_foreground.is_some() {
                    let item_on_fg = item_on_foreground.unwrap();
                    
                    if item_on_fg.name == "color_selection_pixels" { 
                        self.color_selected = item_on_fg.chars[0][0].background_color;
                        self.erase_ansi_colors();
                    }
                    return false;
                };

                match self.tool {
                    Tool::BRUSH => {
                        let pixel: Item = Item {name: "pixel".to_string(), offset: self.screen.layers[0].relative_position(col, row), chars: Pixel{color: self.color_selected}.to_chars()};
                        self.screen.layers[0].add_item(pixel.clone());
                        pixel.draw(&mut self.screen.term, (col as i16, row as i16), self.screen.width, self.screen.height);
                    },
                    Tool::ERASE => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((col, row));
                        match item {
                            Some(item) => {
                                item.erase(&mut self.screen.term, self.screen.layers[0].offset, self.screen.width, self.screen.height);
                                let items: Vec<Item> = self.screen.layers[0].items.clone();
                                self.screen.layers[0].items = items.into_iter().filter(|i| i.offset != item.offset).collect();
                            },
                            None => {}
                        }
                    },
                    Tool::INK => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((col, row));
                        match item {
                            Some(item) => {
                                self.color_selected = item.chars[0][0].background_color;
                                self.tool = Tool::BRUSH;
                            },
                            None => {self.tool = Tool::ERASE}
                        }
                    },
                    Tool::MOVE => {
                        let distance_to_move =  ((col as i16 - self.last_cursor_position.0 as i16), row as i16 - self.last_cursor_position.1 as i16);
                        self.screen.layers[0].erase(&mut self.screen.term, self.screen.width, self.screen.height);
                        self.screen.layers[0].move_layer(distance_to_move);
                        self.screen.layers[0].draw_buffer(&mut self.screen.term, self.screen.width, self.screen.height);
                        self.screen.layers[1].redraw(&mut self.screen.term, self.screen.width, self.screen.height);
                    },
                    Tool::TEXT => {
                        if !self.typing {
                            self.typing = true;
                            self.last_cursor_position = (col, row);
                            self.screen.term.execute(cursor::Show).unwrap();
                            self.screen.term.execute(MoveTo(col, row)).unwrap();
                        }  
                    },
                    _ => {}
                }
            },
            _ => {}
        }

        self.cursor.erase(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);
        self.cursor.chars = vec![vec![self.cursor_term_char()]];
        self.cursor.redraw(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);

        self.cursor_info.erase(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);
        self.cursor_info.chars = self.create_cursor_info_chars((col as i16 -self.screen.layers[0].offset.0 , row as i16-self.screen.layers[0].offset.1 ));
        self.cursor_info.redraw(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);

        if !self.typing {
            self.last_cursor_position = (col, row);
        }
        false
    }
    fn on_resize_event(&mut self, width: u16, height: u16) -> bool {
        self.clear_screen();
        
        self.screen.width = width;
        self.screen.height = height;
        self.cursor_info.offset = (width as i16 - 9, height as i16-1);
        self.cursor.offset = (width as i16-1, 0);
        self.resized = true;
        
        false
    }
}
