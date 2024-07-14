use core::panic;
use std::env;

use std::process::{Command, Stdio};
mod screen;
mod draw_term;
mod constants;


fn main() {
    let args: Vec<_> = env::args().collect();
    let mut addr: Option<String> = None;

    if args.len() == 4 {

        let host = args[2].clone();
        let port = args[3].parse::<u16>().unwrap();
        addr = Some(format!("{}:{}", host, port));
        
        if args[1] == "serve" {
            let _server_process = Command::new("../pixelrs-server/target/debug/pixelrs-server")
                .arg(host.clone())
                .arg(port.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("Failed to start server process");
        }
        else if args[1] == "connect" {
            println!("Connecting to {}", addr.clone().expect(""));
        }
        else {
            panic!("Unknown options");
        }
    }

    let mut draw_term = draw_term::DrawTerm::new();
    draw_term.run(addr);
}