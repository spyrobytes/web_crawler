/**
 * Static Trait Bound
 * 
 * What does the trait bound 'static mean in Rust?
 * 
 */
use std::fs::File;
use std::io::Read;
use std::thread::{self, JoinHandle};

fn main() {
    let f = File::open("data/file1.txt").unwrap();
    // let join_handle = read_in_background(f);
    let join_handle = read_in_background_with_static(f);
    
    // wait for the thread to finish
    //thread::sleep(Duration::from_millis(50));
    // alternative to sleep: join the thread
    join_handle.join().unwrap();

}

// Read a file in the background and print the number of bytes read
// This function works fine
pub fn read_in_background(mut f: File) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        if let Ok(count) = f.read_to_end(&mut buf) {
            println!("read {} bytes from file.", count);
        }
    })
}

// let's make read_in_background generic across a broader set of types,
// not just File. This is our first iteration: it may look good at first 
// glance, but it won't compile!
// The problem: `T` cannot be Send between threads safely;
// Compiler suggestion: consider further restricting `T` to `T: Send + 'static`
#[allow(unreachable_code)]
pub fn read_in_background_generic<T: Read>(mut _f: T) -> JoinHandle<()> {
    todo!("consider further restricting `T` to `T: Send + 'static`");
    // thread::spawn(move || {
    //     let mut buf = Vec::<u8>::new();
    //     if let Ok(count) = f.read_to_end(&mut buf) {
    //         println!("read {} bytes from file.", count);
    //     }
    // })
}

// Now the compiler is happy
// We can now write tests that use std::io::Cursor, which implements Read
// just like File does.
pub fn read_in_background_with_static<T: Read + Send + 'static>(mut f: T) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        if let Ok(count) = f.read_to_end(&mut buf) {
            println!("read {} bytes from file.", count);
        }
    })
}

// Let's write a test that uses std::io::Cursor, which implements Read
// We don't want the test to actually create a real file, so we use Cursor
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_in_background() {
        let cursor = Cursor::new("hello world".to_string());
        let join_handle = read_in_background_with_static(cursor);
        join_handle.join().unwrap();
    }
}