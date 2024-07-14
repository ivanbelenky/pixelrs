use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use std::collections::VecDeque;
use std::thread;

use crossterm::cursor;
use crossterm::cursor::MoveTo;
use crossterm::terminal as terminal;
use crossterm::ExecutableCommand;
use crossterm::style::Color;
use crossterm::event::{self as event, MouseEvent, KeyEventKind, MouseButton, MouseEventKind, KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};
use serde_json::{to_string, from_str};

use crate::screen::TermChar;
use crate::screen::{Screen, Item, Layer, Pixel};
use crate::constants::{EMPTY_TERM_CHAR, MAX_FAILED_SENT_ON_QUEUE};


#[derive(PartialEq)]
enum Tool {
    Brush,
    Erase,
    Ink,
    Move,
    Text
}

#[derive(PartialEq)]
enum Config {
    None,
    ColorSelection,
    Connection
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
    last_cursor_position: (u16, u16),
}

#[derive(Serialize, Deserialize)]
enum Update {
    TermChar(SerializableTermChar),
    Erase(SerializableErase),
    Sync(SerializebleSync),
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct SerializableErase {
    abs_x: i32,
    abs_y: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct SerializableTermChar {
    abs_x: i32,
    abs_y: i32,
    character: char,
    foreground_color: u8,
    background_color: u8,
    empty: bool,
}

impl SerializableTermChar {
    fn from_pixel(pixel: Item, x: i32, y: i32) -> Self {
        let color = pixel.chars[0][0].background_color;
        let mut color_code: u8 = 0;
        
        if let Color::AnsiValue(c) = color {
            color_code = c;
        }

        SerializableTermChar{
            abs_x: x, 
            abs_y: y,
            character: ' ', 
            foreground_color: color_code,
            background_color: color_code,
            empty: false
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SerializebleSync {
    items: Vec<SerializableTermChar>,
}


pub struct Client {
    client: TcpStream,
    _addr: String,
    _live: bool,
    pubsub: VecDeque<Vec<u8>>,
}


impl Client {
    // create and connect non blocking to the addr specified
    pub fn new(addr: &String) -> Self {
        let mut attempts = 0;
        let max_attempts = 5;
        let socket_client: TcpStream;

        loop {
            attempts += 1;
            println!("Attempting to connect to {}... (Attempt {}/{})\n", addr, attempts, max_attempts);
            thread::sleep(Duration::from_secs(1));

            match TcpStream::connect(addr) {
                Ok(stream) => {
                    socket_client = stream;
                    break;
                }
                Err(e) => {
                    println!("Failed to connect: {}. Attempt: {}\n", e, attempts);
                    if attempts >= max_attempts {
                        panic!("Failed to connect after {} attempts\n", max_attempts);
                    } else { thread::sleep(Duration::from_secs(1));}
                }
            }
        }

        socket_client
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        println!("Successfully connected to {}", addr);

        Client {
            client: socket_client,
            _addr: addr.clone(),
            _live: true,
            pubsub: VecDeque::new(),
        }
    }
    
    // plain bytes return from other clients in the shared session
    fn read_server_update(&mut self) -> Option<Vec<u8>> {
        let mut server_buff: Vec<u8> = vec![0; 1024];
        match self.client.read(&mut server_buff) {
            Ok(n) => {
                server_buff.truncate(n);
                Some(server_buff)
            }
            Err(_) => {
                None
            }
        }
    }
    
    // write to server queued updates from current client
    // failed sents are pushed back for next run 
    fn broadcast_client_updates(&mut self) {
        let mut failed: VecDeque<Vec<u8>> = VecDeque::new();
        while !self.pubsub.is_empty() { 
            let update = self.pubsub.pop_front();
            if let Some(update) = update {
               match self.client.write_all(&update) {
                Ok(_) => {},
                Err(e) => {
                    println!("Failed to send update to server: {}", e);
                    failed.push_back(update);
                    break;
                }
               }
            }
        }
        while !failed.is_empty() {
            self.pubsub.push_back(failed.pop_front().unwrap().to_vec());
        }
        
        let remove_n: usize = MAX_FAILED_SENT_ON_QUEUE - failed.len();
        for _ in 0..remove_n {
            failed.pop_front();
        };
    }

    // publish serialized update the client pubsub queue
    // the update event is going to be serialized and pushed to the queue
    // for later processing
    fn publish(&mut self, update: Update) {
        let serialized: Vec<u8> = match update {
            Update::TermChar(tc) => {
                to_string(&Update::TermChar(tc))
                    .expect("failed to deserialize term char")
                    .into_bytes()
            }
            Update::Erase(erase) => {
                to_string(&Update::Erase(erase))
                    .expect("failed to serialize erase")
                    .into_bytes()
            },
            Update::Sync(s) => {
                to_string(&Update::Sync(s))
                    .expect("failed to serialize sync")
                    .into_bytes()
            }
        };
        self.pubsub.push_back(serialized);
    }
}




impl DrawTerm {
    pub fn new() -> Self {
        let (width, height): (u16, u16) = terminal::size().unwrap();
        let foreground: Layer = Layer::new_empty("foreground".to_string(), width, height, (0, 0));
        let background: Layer = Layer::new_empty("background".to_string(), width, height, (0, 0));
        let screen: Screen = Screen::new(vec![background, foreground]);
        let tool: Tool = Tool::Brush;
        let config: Config = Config::None;
        
        let cursor: Item = Item { name: "cursor".to_string(), offset: (width as i32-1, 0), chars: vec![vec![EMPTY_TERM_CHAR]] };
        let cursor_info: Item = Item {name: "cursor_info".to_string(), offset: (width as i32 - 9, height as i32-1), chars: vec![vec![EMPTY_TERM_CHAR]]};
        let color_selected: Color = Color::AnsiValue(0);
        let last_cursor_position: (u16, u16) = (0, 0);
        let resized: bool = false;
        let typing: bool = false;
        DrawTerm { screen, tool, config, cursor, cursor_info, resized, typing, color_selected, last_cursor_position}
    }

    
    pub fn run(&mut self, addr: Option<String>) {
    
        self._enter();
        let mut exit = false;
        
        let mut client: Option<Client> = None;
        if let Some(addr) = addr {
            client = Some(Client::new(&addr));
        }
        self.clear_screen();

        let mut updates: VecDeque<Vec<u8>> = VecDeque::new();
        while !exit{
            // network session client handler
            if let Some(client) = &mut client {
                let server_update = client.read_server_update();
                if let Some(server_update) = server_update {
                    updates.push_back(server_update);
                }
                client.broadcast_client_updates();
            }
            
            let must_update: bool = !updates.is_empty();
            self.on_netowrk_update_events(&mut updates, &mut client);
            if must_update {
                self.screen.layers[0].draw_buffer(&mut self.screen.term, self.screen.width, self.screen.height);
            }
            
            // local client event handler
            if event::poll(Duration::ZERO).unwrap() {
                match event::read().unwrap() {
                    event::Event::Key(event) => exit = self.on_key_event(event, &client),
                    event::Event::Mouse(event) => exit = self.on_mouse_event(event, &mut client),
                    event::Event::Resize(width, height) => exit = self.on_resize_event(width, height),
                    _ => {}
                }
            }
        }
        self._exit();
    }

    fn _enter(&mut self) {
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
    
    pub fn clear_screen(&mut self) {
        self.screen.term.execute(terminal::Clear(terminal::ClearType::All)).unwrap();
        self.screen.term.flush().unwrap();
    }

    pub fn draw_ansi_colors(&mut self) { 
        self.config = Config::ColorSelection;
        for c in 0..16 {              
            let color_pixel: Item = Item {
                name: "color_selection_pixels".to_string(), 
                offset: (2*c, self.screen.height as i32-1), 
                chars: Pixel{color: Color::AnsiValue(c as u8)}.to_chars()
            };
            self.screen.layers[1].add_item(color_pixel.clone());
            color_pixel.draw(&mut self.screen.term, (2*c, self.screen.height as i32-1), self.screen.width, self.screen.height);
        }

    }

    pub fn erase_ansi_colors(&mut self) {
        self.config = Config::None;
        self.screen.layers[1].items.retain(|item| item.name != "color_selection_pixels");
        for c in 0..32 {
            EMPTY_TERM_CHAR.draw(
                &mut self.screen.term,
                (c, self.screen.height as i32 - 1), 
                self.screen.width, 
                self.screen.height
            );
        }
    }
    
    pub fn cursor_term_char(&self) -> TermChar {
        match self.tool {
            Tool::Brush => { 
                let mut fg_color = self.color_selected;
                if self.color_selected == Color::AnsiValue(0){ fg_color = Color::White };    
                TermChar {
                    character: 'B',
                    foreground_color: fg_color,
                    background_color: Color::Reset,
                    empty: false,
                }
            },
            Tool::Erase => TermChar {
                character: 'E',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::Ink => TermChar {
                character: 'I',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::Move => TermChar {
                character: 'M',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
            Tool::Text => TermChar {
                character: 'T',
                foreground_color: Color::White,
                background_color: Color::Reset,
                empty: false,
            },
        }
    }
    pub fn create_cursor_info_chars(&self, (col, row): (i32, i32)) -> Vec<Vec<TermChar>> {
        // make col and row //2 values
        let col = col/2;
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
        vec![chars]
    }

}


pub trait EventHandlers {
    // event handlers must return bool | null
    fn on_key_event(&mut self, event: KeyEvent, client: &Option<Client>) -> bool;
    fn on_mouse_event(&mut self, event: MouseEvent, client: &mut Option<Client>) -> bool;
    fn on_resize_event(&mut self, width: u16, height: u16) -> bool;
    fn on_netowrk_update_events(&mut self, updates: &mut VecDeque<Vec<u8>>, client: &mut Option<Client>);
}


impl EventHandlers for DrawTerm {
    fn on_key_event(&mut self, event: KeyEvent, client: &Option<Client>) -> bool {

        if self.typing {
            match event.code {
                KeyCode::Char(c) => {
                    let char: Item = Item {
                        name: "char".to_string(), 
                        offset: self.screen.layers[0].relative_position(self.last_cursor_position.0 , self.last_cursor_position.1), 
                        chars: vec![vec![TermChar {character: c, foreground_color: self.color_selected, background_color: Color::Reset, empty: false}, EMPTY_TERM_CHAR]]
                    };
                    self.screen.layers[0].add_item(char.clone());
                    char.draw(&mut self.screen.term, (self.last_cursor_position.0 as i32, self.last_cursor_position.1 as i32), self.screen.width, self.screen.height);
                    self.last_cursor_position = (self.last_cursor_position.0+2, self.last_cursor_position.1);
                    self.screen.term.execute(MoveTo(self.last_cursor_position.0, self.last_cursor_position.1)).unwrap();
                },
                KeyCode::Enter | KeyCode::Esc => {
                    self.typing = false;
                    self.tool = Tool::Brush;
                    self.screen.term.execute(cursor::Hide).unwrap();
                },
                KeyCode::Backspace => {
                    let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute(((self.last_cursor_position.0-2) as i32, self.last_cursor_position.1 as i32));
                    if let Some(item) = item {
                        item.erase(&mut self.screen.term, self.screen.layers[0].offset, self.screen.width, self.screen.height);
                        let items: Vec<Item> = self.screen.layers[0].items.clone();
                        self.screen.layers[0].items = items.into_iter().filter(|i| i.offset != item.offset).collect();
                        self.last_cursor_position = (self.last_cursor_position.0-2, self.last_cursor_position.1);
                        self.screen.term.execute(MoveTo(self.last_cursor_position.0, self.last_cursor_position.1)).unwrap();
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
                                self.tool = Tool::Erase;
                                false
                            },
                            'b' => {
                                self.tool = Tool::Brush;
                                false
                            },
                            'i' => {
                                self.tool = Tool::Ink;
                                false
                            }
                            'c' => {
                                match self.config {
                                    Config::ColorSelection => {
                                        self.erase_ansi_colors();
                                        return false;   
                                    },
                                    Config::Connection => {
                                        return false;
                                    },
                                    _ => {},
                                }
                                if self.tool == Tool::Erase {self.tool = Tool::Brush};
                                self.draw_ansi_colors();
                                false
                            },
                            'm' => {
                                self.tool = Tool::Move;
                                false
                            },
                            'a' => {
                                self.tool = Tool::Text;
                                false
                            },
                            'x' => {
                                match self.config {
                                    Config::Connection => { 
                                        self.config = Config::None;
                                        self.clear_screen();
                                        self.screen.term.execute(event::EnableMouseCapture).unwrap();
                                        self.screen.layers[0].draw_buffer(&mut self.screen.term, self.screen.width, self.screen.height);
                                    },
                                    _ => {
                                        self.config = Config::Connection;
                                        self.clear_screen();
                                        self.screen.term.execute(MoveTo(0, 0)).unwrap();
                                        match client {
                                            Some(ref client) => {
                                                println!("{}", client._addr);
                                            },
                                            None => {
                                                println!("No server available. Rerun with host port options");                                                
                                            },
                                        } 
                                    },
                                }
                                false
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

    fn on_mouse_event(&mut self, event: MouseEvent, mut client: &mut Option<Client>) -> bool {
        // dont use mouse events when creating connections or monitoring them
        if self.config == Config::Connection { return false };
        
        let (col, row) = (event.column & !(event.column%2), event.row);
        self.screen.term.execute(MoveTo(col, row)).unwrap();

        if self.resized {
            self.resized = false;
            self.screen.layers[0].redraw(&mut self.screen.term, self.screen.width, self.screen.height);
            self.screen.layers[1].redraw(&mut self.screen.term, self.screen.width, self.screen.height);       
        }

        let item_on_foreground = self.screen.layers[1].get_item_at_absolute((col as i32, row as i32));
        

        match event.kind {
            event::MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(event::MouseButton::Left) => {
                if item_on_foreground.is_some() {
                    let item_on_fg = item_on_foreground.unwrap();    
                    if item_on_fg.name == "color_selection_pixels" {
                        // given that items are represented by 2D matrix of TermChar
                        // the only way to get the color is by checking the first element
                        // grabbing it and ressetting the color menu
                        self.color_selected = item_on_fg.chars[0][0].background_color;                        
                        self.erase_ansi_colors();
                    }
                    return false;
                };

                match self.tool {
                    Tool::Brush => {
                        // the x,y are absolute, because there is no compounding of
                        // layers one on top of the other. Just (screen(bg_layer(item)))
                        let (abs_x, abs_y) = self.screen.layers[0].relative_position(col, row);
                        let pixel: Item = Item{
                            name: "P".to_string(),
                            offset: (abs_x, abs_y), 
                            chars: Pixel{color: self.color_selected}.to_chars(),
                        };

                        self.screen.layers[0].add_item(pixel.clone());
                        
                        if let Some(client) = &mut client {
                            client.publish(
                                Update::TermChar(
                                    SerializableTermChar::from_pixel(
                                        pixel.clone(), abs_x, abs_y
                                    )
                                )
                            );
                        }
                        
                        pixel.draw(&mut self.screen.term, (col as i32, row as i32), self.screen.width, self.screen.height);
                    },
                    Tool::Erase => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((col as i32, row as i32));
                        if let Some(item) = item {
                            item.erase(&mut self.screen.term, self.screen.layers[0].offset, self.screen.width, self.screen.height);
                            let items: Vec<Item> = self.screen.layers[0].items.clone();
                            
                            if let Some(client) = &mut client {
                                client.publish(Update::Erase(SerializableErase{abs_x: item.offset.0, abs_y: item.offset.1}));
                            }
                            
                            self.screen.layers[0].items = items.into_iter().filter(|i| i.offset != item.offset).collect();

                        }
                    },
                    Tool::Ink => {
                        let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((col as i32, row as i32));
                        match item {
                            Some(item) => {
                                self.color_selected = item.chars[0][0].background_color;
                                self.tool = Tool::Brush;
                            },
                            None => {self.tool = Tool::Erase}
                        }
                    },
                    Tool::Move => {
                        let distance_to_move =  ((col as i32 - self.last_cursor_position.0 as i32), row as i32 - self.last_cursor_position.1 as i32);
                        self.screen.layers[0].move_layer(distance_to_move);
                        self.screen.layers[0].draw_buffer(&mut self.screen.term, self.screen.width, self.screen.height);
                    },
                    Tool::Text => {
                        if !self.typing {
                            self.typing = true;
                            self.last_cursor_position = (col, row);
                            self.screen.term.execute(cursor::Show).unwrap();
                            self.screen.term.execute(MoveTo(col, row)).unwrap();
                        }  
                    },
                }
            },
            _ => {}
        }

        self.cursor.erase(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);
        self.cursor.chars = vec![vec![self.cursor_term_char()]];
        self.cursor.redraw(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);

        self.cursor_info.erase(&mut self.screen.term, (0,0), self.screen.width, self.screen.height);
        self.cursor_info.chars = self.create_cursor_info_chars((col as i32 -self.screen.layers[0].offset.0 , row as i32-self.screen.layers[0].offset.1 ));
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
        self.cursor_info.offset = (width as i32 - 9, height as i32-1);
        self.cursor.offset = (width as i32-1, 0);
        self.resized = true;
        
        false
    }

    fn on_netowrk_update_events(&mut self, updates: &mut VecDeque<Vec<u8>>, _client: &mut Option<Client>) {
        while !updates.is_empty(){
            let update_serialized_bytes = updates.pop_front().unwrap();
            let update_serialized: String = String::from_utf8(update_serialized_bytes).unwrap();
            
            let update: Update = match from_str(&update_serialized) {
                Ok(u) => {u}
                Err(e) => {
                    println!("Failed to deserialize update: {}", e);
                    continue;
                }
            };

            match update {
                Update::TermChar(tc) => {
                    let pixel_char = TermChar{
                        character: tc.character, 
                        foreground_color: Color::AnsiValue(tc.foreground_color), 
                        background_color: Color::AnsiValue(tc.background_color), 
                        empty: tc.empty
                    };
                    
                    let item: Item = Item {
                        name: "pixel".to_string(),
                        offset: (tc.abs_x, tc.abs_y),
                        chars: vec![vec![pixel_char, pixel_char]]
                    };

                    self.screen.layers[0].add_item(item.clone());
                },
                Update::Erase(erase) => {
                    let (offx, offy) = self.screen.layers[0].offset;
                    let item: Option<&Item> = self.screen.layers[0].get_item_at_absolute((erase.abs_x+offx, erase.abs_y+offy));
                    if let Some(item) = item {
                        item.erase(&mut self.screen.term, self.screen.layers[0].offset, self.screen.width, self.screen.height);
                        let items: Vec<Item> = self.screen.layers[0].items.clone();
                        self.screen.layers[0].items = items.into_iter().filter(|i| i.offset != item.offset).collect();
                    }
                },
                _ => (),
            }
        }
    }


}


