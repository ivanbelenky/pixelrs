use std::io::Write;
use std::time::Duration;
use crossterm::cursor;
use crossterm::cursor::MoveTo;
use crossterm::terminal as terminal;
use crossterm::ExecutableCommand;
use crossterm::style::Color;
use crossterm::event::{self as event, MouseEvent, KeyEventKind, MouseButton, MouseEventKind, KeyCode, KeyEvent};
use crate::screen::{Screen, Item, Layer, Pixel};
use crate::constants::EMPTY_TERM_CHAR;

#[derive(PartialEq)]
enum Tool {
    BRUSH,
    ERASE,
    INK,
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
    color_selected: Color,
}


impl DrawTerm {
    pub fn new() -> Self {
        let (width, height): (u16, u16) = terminal::size().unwrap();
        let foreground: Layer = Layer::new_empty("foreground".to_string(), width, height, (0, 0));
        let background: Layer = Layer::new_empty("background".to_string(), width, height, (0, 0));
        let screen: Screen = Screen::new(vec![background, foreground]);
        let tool: Tool = Tool::BRUSH;
        let config: Config = Config::NONE;
        let color_selected: Color = Color::AnsiValue(0);
        DrawTerm { screen, tool, config, color_selected}
    }
    pub fn run(&mut self) {
        self._enter();
        let mut exit = false;
        while !exit{
            if event::poll(Duration::from_micros(100)).unwrap() {
                match event::read().unwrap() {
                    event::Event::Key(event) => exit = self.on_key_event(event),
                    event::Event::Mouse(event) => exit = self.on_mouse_event(event),
                    event::Event::Resize(width, height) => exit = self.on_resize_event(width, height),
                    _ => {}
                }
            }
        }
        self._exit();
    }
    fn _enter(&mut self){
        terminal::enable_raw_mode().unwrap();
        self.screen.term.execute(event::EnableMouseCapture).unwrap();
        self.clear_screen();
    }
    fn _exit(&mut self) {
        self.screen.term.execute(MoveTo(0, self.screen.height)).unwrap();
        self.screen.term.execute(event::DisableMouseCapture).unwrap();
        terminal::disable_raw_mode().unwrap();
        
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
            color_pixel.draw(&mut self.screen.term, (2*c, self.screen.height as i16-1));
        }
    }
    pub fn erase_ansi_colors(&mut self) {
        self.config = Config::NONE;
        self.screen.layers[1].items.retain(|item| item.name != "color_selection_pixels");
        for c in 0..32 {
            EMPTY_TERM_CHAR.draw(&mut self.screen.term, (c, self.screen.height as i16 - 1));
        }
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
                            }
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
        let (col, row) = (event.column, event.row);
        let mut to_remove_bg: Vec<&Item> = Vec::new();
        
        self.screen.term.execute(cursor::MoveTo(event.column, event.row)).unwrap();
        let item_on_foreground = self.screen.layers[1].get_item_at_index((col, row));
        
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
                        let pixel: Item = Item {name: "pixel".to_string(), offset: ((col & !(self.screen.layers[0].offset.0 as u16+1%2)) as i16, row as i16), chars: Pixel{color: self.color_selected}.to_chars()};
                        self.screen.layers[0].add_item(pixel.clone());
                        pixel.redraw(&mut self.screen.term, (0,0)); // hack for pixels to not worry on 
                        return false;
                    },
                    Tool::ERASE => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_index((col & !(self.screen.layers[0].offset.0 as u16+1%2), row));
                        match item {
                            Some(item) => {
                                item.erase(&mut self.screen.term, (0,0));
                                to_remove_bg.push(item);
                            },
                            None => {}
                        }
                        return false;
                    },
                    Tool::INK => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_index((col & !(self.screen.layers[0].offset.0 as u16+1%2), row));
                        match item {
                            Some(item) => {
                                self.color_selected = item.chars[0][0].background_color;
                                self.tool = Tool::BRUSH;
                            },
                            None => {self.tool = Tool::ERASE}
                        }
                        return false
                    }
                    _ => {}
                }
                //cleanup
                for item in to_remove_bg {
                    self.screen.layers[0].remove_item(Some(&item));
                }
                false
            }

            _ => false,
        }
    }
    fn on_resize_event(&mut self, width: u16, height: u16) -> bool {
        self.clear_screen();
        println!("Resized to {}x{}", width, height);
        false
    }
}
