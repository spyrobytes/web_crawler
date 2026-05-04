/**
 * Playground for concurrency patterns and related concepts.
 * 
 * Read a file and print its contents.
 */
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;

fn main() -> std::io::Result<()> {
    let file = File::open("data/foo.txt")?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;

    println!("contents: {}", contents);

    Ok(())
}
