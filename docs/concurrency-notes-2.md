# Concurrency In Rust Notes 2/2

References:

- Hands-On Functional Programming in Rust. Andrew Johnson. Packt Publishing. 2018


## Understanding Send and Sync traits

To really get the nuances of `Send` and `Sync` traits, we need to understand 
the concept of ownership in Rust and how they affects thread safety. `Send` and 
`Sync` traits are best understood in the context of variable sharing across 
threads. Let's try to share a variable across threads, as follows:

```rust
use std::thread;

fn main() {
   let a = vec![1, 2, 3];

   thread::spawn(|| {
      println!("a = {:?}", a); // `a` is borrowed here
   });
}
```

This code will not compile, and the error message is:

```text
error[E0373]: closure may outlive the current function, but it borrows `a`, which 
is owned by the current function
```
This error indicates the following:

- Referencing variable `a` from inside the closure is okay
- The closure lives longer than variable `a`

Closures sent to threads must have a `static` lifetime, meaning that they can
live for the entire duration of the program. Variable `a` is a local variable, 
and thus will go out of scope before the `static` closure. 

To fix this error, it is common to `move` the variable `a` into the closure. Thus, 
`a` will inherit the same lifetime as the closure:

```rust
use std::thread;

fn main() {
   let a = vec![1, 2, 3];

   thread::spawn(move || {
      println!("a = {:?}", a); // ok, `a` is moved into the closure
   });

   // `a` is out of scope here
}
```

Things work fine now. But let's try and share variable `a` across two threads:

```rust
use std::thread;

fn main() {
   let a = vec![1, 2, 3];

   thread::spawn(move || {
      println!("a = {:?}", a); // ok, `a` is moved into the closure
   });

   thread::spawn(move || {
      println!("a = {:?}", a); // error, capture of moved value: `a`
   });
}
```

"Capture of moved value"? Using a variable in a closure is called *capturing*.
So the problem is that moving variable `a` into the first closure invalidates it
for further use. Capturing variables can be by value, reference, or mutable 
reference.

One way to fix the two thread problem is to use `static` variables:

```rust
use std::thread;

fn main() {
    // static variables have the scope of the entire program
   static A: [u8; 100] = [22; 100]; 

   thread::spawn(|| {
      A[3];
   });

   thread::spawn(|| {
      A[3]
   });
}
```

However, `static` variables have some limitations when used with threads:

- Reading from static variables is safe, but mutating static variables is unsafe
- Static variables are disallowed from allocating heap memory directly, so they
can be difficult to work with.

A better way to fix scope problems is to use a *thread-safe* reference counter, 
such as `Arc` (Atomic reference counter). This allows for shared ownership of
data across threads in a safe manner.

```rust
use std::thread;
use std::sync::Arc;

fn main() {
   let a = Arc::new(vec![1, 2, 3]);
   let b = a.clone();

   let handle_1 = thread::spawn(move || {
      println!("a = {:?}", a);
   });

   let handle_2 = thread::spawn(move || {
      println!("b = {:?}", b);
   });

   // main thread waits for spawned threads to finish
   handle_1.join().unwrap();
   handle_2.join().unwrap();
}
```

If shared data should be mutated, then a `Mutex` lock can allow thread-safe 
locking. Another useful lock is the `std::sync::RwLock`. Mutex-protected shared
data is shown below:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
   // `a` is a mutex-protected vector
   let a = Arc::new(Mutex::new(vec![1, 2, 3]));

   // Spawn the first thread and store its join handle
   let handle1 = {
      let a = Arc::clone(&a);
      thread::spawn(move || {
         let mut a = a.lock().unwrap(); // acquire lock
         (*a)[1] = 2;
      })
   };

    // Spawn the second thread and store its join handle
    let handle2 = {
        let a = Arc::clone(&a);
        thread::spawn(move || {
            let mut a = a.lock().unwrap();
            (*a)[1] = 3;
        })
    };

    // Wait for both threads to complete
    handle1.join().unwrap();
    handle2.join().unwrap();

    // `main` thread print the final state of the vector
    // Since the vector is mutex-protected, we need to lock it first
    // before we can access it.
    let a = a.lock().unwrap();
    println!("{:?}", *a);
}

```

So why is mutation allowed after the `lock`, but not before? The answer is 
`Send` and `Sync` traits. The `Mutex` type implements both `Send` and `Sync`,
which means that it can be sent across threads and shared between threads.

`Send` and `Sync` are *marker traits*. A marker trait does not implement any 
functionality; however, it indicates that a type has some property. These two 
properties tell the compiler what behavior should be allowed with regards to 
sharing data between threads.

These are the rules regarding thread data sharing:

- A type is `Send` if it is safe to send it to another thread
  (i.e., it is safe to transfer ownership between threads)
- A type is `Sync` if it is safe to share between multiple threads
   (i.e., it is safe to share a reference between threads)

To make mutable data that can be shared across threads, whatever data type, you 
use must implement `Sync`. The standard Rust library has some thread-safe 
concurrency primitives, such as `Mutex`, for this purpose. 

To implement Sync for a type, just implement the trait with no body:

```rust
use std::thread;

struct MyBox(u8); // tuple struct

unsafe impl Send for MyBox {}
unsafe impl Sync for MyBox {}

static A: MyBox = MyBox(22);

fn main() {
   thread::spawn(move || {
      A.0
   });
   thread::spawn(move || {
      A.0
   });
}
```
The `Send` and `Sync` traits are always unsafe to implement. Thankfully, both of 
these marker traits are generally derived by the compiler, so you will very rarely 
need to manually derive them.

To summarize, when sharing data across threads, `move`, `channel`, `Arc`, and
`Mutex` will get you through most situations.  The `std::marker` traits `Sync` 
and `Send` are embedded in the language itself (not a library).

### The `Send` trait

The `Send` marker trait indicates that ownership of values of the type 
implementing `Send` can be transferred between threads.  Almost every Rust 
type is `Send`. The exceptions are:

- Rc<T> (reference counted pointer)
This cannot be `Send` because if you cloned an `Rc<T>` value and tried to 
transfer ownership of the clone to another thread, both threads might update 
the reference count at the same time. If you tried to do this, you'll get the 
error "the trait Send is not implemented for `Rc<Mutex<i32>>`". But when you
use `Arc` (which implements the `Send` trait) instead of `Rc`, the code will 
compile.


```rust
use std::rc::Rc;
use std::sync::Mutex;
use std::thread;

fn main() {
   let counter = Rc::new(Mutex::new(0));
   let counter_clone = Rc::clone(&counter);

   // The cloned `Rc` is moved into the thread
   // so ownership is transferred to the thread
   let handle = thread::spawn(move || {
      // `Rc` is not `Send`, so the compiler will not allow this
      let mut num = counter_clone.lock().unwrap();
      *num += 1;
   });

   handle.join().unwrap();
   println!("counter = {}", counter.lock().unwrap());
}
```
The follwoing code will compile, because `Arc` is `Send`:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
   let counter = Arc::new(Mutex::new(0));
   let counter_clone = Arc::clone(&counter);

   let handle = thread::spawn(move || {
      let mut num = counter_clone.lock().unwrap();
      *num += 1;
   });

   handle.join().unwrap();
   println!("counter = {}", counter.lock().unwrap());
}
```

### The `Sync` trait

The `Sync` marker trait indicates that it is safe for the type implementing 
`Sync` to be referenced from multiple threads. Similar to `Send`, primitive 
types are `Sync`, and types composed entirely of types that are `Sync` are also 
`Sync`.

The smart pointer `Rc<T>` is also not `Sync` for the same reasons that it’s not 
`Send`. The `RefCell<T>` type and the family of related `Cell<T>` types are not 
`Sync`.

 The smart pointer `Mutex<T>` is `Sync` and can be used to share access with 
 multiple threads.