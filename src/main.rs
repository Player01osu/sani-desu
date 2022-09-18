use std::process::Command;

fn main() {
    Command::new("dmenu").spawn().unwrap();
    println!("Hello, world!");
}
