# Concepts, Features, and Patterns used in this Project

## 1. Associated Items

*Associated Items* refers to a set of rules pertaining to *items* of various 
types. It is an extension to *trait generics*, and allows traits to internally 
define new *items*.

One such item is called an *associated type*, providing simpler usage patterns 
when the trait is generic _over its container type_.

Associated types are meant to solve the problem of needing many generic type
parameters, especially when parameterized over container types. For example,

```rust
struct Container(i32, i32); // A tuple struct containing two fields of type `i32`.

// A trait which checks if 2 items are stored inside of container.
// Also retrieves first or last value.
trait Contains<A, B> {
    fn contains(&self, _: &A, _: &B) -> bool; // Explicitly requires `A` and `B`.
    fn first(&self) -> i32; // Doesn't explicitly require `A` or `B`.
    fn last(&self) -> i32;  // Doesn't explicitly require `A` or `B`.
}

impl contains<i32, i32> for Container {
    // True if the numbers stored are equal.
    fn contains(&self, number_1: &i32, number_2: &i32) -> bool {
        (&self.0 == number_1) && (&self.1 == number_2)
    }

    // Grab the first number.
    fn first(&self) -> i32 { self.0 }

    // Grab the last number.
    fn last(&self) -> i32 { self.1 }
}

// `C` contains `A` and `B`. In light of that, having to express `A` and
// `B` again is a nuisance.
fn difference<A, B, C>(container: &C) -> i32 where
    C: contains<A, B> {
    container.last() - container.first()
}

fn main() {
    let number_1 = 3;
    let number_2 = 10;

    let container = Container(number_1, number_2);

    println!("Does container contain {} and {}: {}",
        &number_1, &number_2,
        container.contains(&number_1, &number_2));
    println!("First number: {}", container.first());
    println!("Last number: {}", container.last());

    println!("The difference is: {}", difference(&container));
}
```

Because `Contains` is generic, we are forced to *explicitly* state all of the 
generic types for `fn difference()`. In practice, we want a way to express 
that `A` and `B` are determined by the input `C`. As you will see in the next 
section, *associated types* provide exactly that capability.

### 1.1 Associated Types

The use of *Associated types* improves the overall readability of code by moving
inner types locally into a trait as output types. Syntax for the trait definition 
is as follows:

```rust
// `A` and `B` are defined in the trait via the `type` keyword.
// (Note: `type` in this context is different from `type` when used for
// aliases).
trait Contains {
    type A;
    type B;

    // Updated syntax to refer to these new types generically.
    fn contains(&self, _: &Self::A, _: &Self::B) -> bool;
}
```
Note that functions that use the trait `Contains` are no longer required to 
express `A` or` B` at all:

```rust
// Without using associated types
fn difference<A, B, C>(container: &C) -> i32 where
    C: Contains<A, B> { ... }

// Using associated types
fn difference<C: Contains>(container: &C) -> i32 { ... }
```

### 1.2 Associated Types in the Generic `Spider` trait

We can build a generic spider such as:

```rust
pub trait Spider<I>{

    fn name(&self) -> String;
    fn start_urls(&self) -> Vec<String>;
    async fn scrape(&self, url: &str) -> Result<(Vec<I>, Vec<String>), Error>;
    async fn process(&self, item: I) -> Result<(), Error>;
}

// For a function to use this trait, it must be generic over the type `I`.
fn use_spider<I, S: Spider<I>>(spider: S) {
    // ...
}
```

This is not very convenient. We can use *associated types* to improve the
readability of the code:

```rust
#[async_trait]
pub trait Spider {
    type Item;

    fn name(&self) -> String;
    fn start_urls(&self) -> Vec<String>;
    async fn scrape(&self, url: &str) -> Result<(Vec<Self::Item>, Vec<String>), Error>;
    async fn process(&self, item: Self::Item) -> Result<(), Error>;
}

// Then, we can use the trait like this:
fn use_spider<S: Spider>(spider: S) {
    // ...

}
```

Like with type parameters, you can add constraints to associated types:

```rust
pub trait Spider {
    type Item: Debug + Clone; // `Item` must implement `Debug` and `Clone`.

    fn name(&self) -> String;
    fn start_urls(&self) -> Vec<String>;
    async fn scrape(&self, url: &str) -> Result<(Vec<Self::Item>, Vec<String>), Error>;
    async fn process(&self, item: Self::Item) -> Result<(), Error>;
}
```


