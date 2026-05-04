# What does the trait bound 'static mean in Rust?

[SEE](https://codeandbitters.com/static-trait-bound/)


The `'static` lifetime is a special lifetime that indicates that a value has a
lifetime of the entire program. It is really clear what a `'static` reference is
and it is pretty straightforward to understand in this context.

However, when used as trait bounds, it can be a bit confusing, especially in 
generic functions. The compiler would suggest a `'static` trait bound 
to address a problem, even when references are not used in the function. My goal
here is to explore what `'static` really mean in this context, so that we can be 
confident (and take off the guess work) in using it.

Consider the following code:

```rust
use std::fs::File;
use std::io::Read;
use std::thread::{self, JoinHandle};

pub fn read_in_background(mut f: File) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        if let Ok(count) = f.read_to_end(&mut buf) {
            println!("read {} bytes from file.", count);
        }
    })
}

fn main() {

    let f = File::open("data/file1.txt").unwrap();
    let join_handle = read_in_background_with_static(f);
    
    // wait for the thread to finish
    join_handle.join().unwrap();

}
```

This code compiles and runs fine. However, if we try to make the function 
generic so that it can read from any type that implements the `Read` trait
(e.g. `File`, `TcpStream`, `Cursor`, etc.), things get a bit more complicated.


```rust, will not compile
pub fn read_in_background_generic<T: Read>(mut _f: T) -> JoinHandle<()> {
    todo!("consider futher restricting `T` to `T: Send + 'static`");
    thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        if let Ok(count) = f.read_to_end(&mut buf) {
            println!("read {} bytes from file.", count);
        }
    })
}
```

The above code is our first attempt to make the function generic. It may look
good at first, but it won't compile!

The problem: `T` cannot be Send between threads safely.
Compiler suggestion: consider further restricting `T` to `T: Send + 'static`

```rust
pub fn read_in_background_with_static<T: Read + Send + 'static>(mut f: T) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = Vec::<u8>::new();
        if let Ok(count) = f.read_to_end(&mut buf) {
            println!("read {} bytes from file.", count);
        }
    })
}
```
Now, the code compiles and runs fine. But what does the `'static` trait bound
really mean here? Why is it needed? What happens if we remove it? Let's find out.

## What does the `'static` trait bound mean?

In Rust, the `'static` trait bound in the context of generic functions, 
particularly when used with threading, serves a specific and important purpose. 
It's not just about the *lifetime of references*, but more about ensuring the 
*safety* and *validity* of data across thread boundaries.

The followings are keys to understanding `'static` in generic functions:

1. Lifetime of Data in Threads: When you spawn a new thread, Rust needs to 
guarantee that any data used inside that thread lives at least as long as the 
thread itself. This is crucial because the parent thread might end before the 
child thread, and if the child thread is using data from the parent thread, 
it could lead to undefined behavior.

2. Ensuring Safety Across Threads: The 'static bound ensures that the data 
either:

    - Lives for the entire duration of the program ('static lifetime), or
    - Is owned by the thread (hence, not borrowed).

The point is that, when we create a generic function that spawns a new thread,
we need to convince the compiler that our function must work for *all possible* 
types: in our code example, types that satisfy the bounds `Read` + `Send` traits.
But some of those types might be *references* or might include *references*
internally. That is why you see the compiler suggesting the `'static` trait
bound, because it is the only way to guarantee that the data is valid for the
entire duration of the thread. And we're done here!
