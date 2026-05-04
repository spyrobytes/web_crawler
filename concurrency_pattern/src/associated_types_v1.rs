/**
 * Associated Types - Concepts and Examples (part 1/2)
 * 
 * This is a simple program that demonstrates the need for *Associated Types*
 * in Rust. The program is a simple container that holds two numbers and
 * implements a trait that checks if two numbers are stored inside of the
 * container. There is also a utility function that calculates the difference
 * between the two numbers.
 *
 */
// A tuple struct containing two fields of type `i32`.
struct Container(i32, i32); 

// A trait which checks if 2 items are stored inside of container.
// The trait is parameterized over `A` and `B` to generically
// Also retrieves first or last value.
trait Contains<A, B> {
    fn contains(&self, _: &A, _: &B) -> bool; // Explicitly requires `A` and `B`.
    fn first(&self) -> i32; // Doesn't explicitly require `A` or `B`.
    fn last(&self) -> i32;  // Doesn't explicitly require `A` or `B`.
}

impl Contains<i32, i32> for Container {
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
// `B` again is a nuisance.  In practice, we want a way to express 
// that `A` and `B` are determined by the input `C`.
fn difference<A, B, C>(container: &C) -> i32 where
    C: Contains<A, B> {
    container.last() - container.first()
}

fn main() {
    let number_1 = 3;
    let number_2 = 10;

    // Instantiate a Container
    let container = Container(number_1, number_2);

    println!("Does container contain {} and {}: {}",
        &number_1, &number_2,
        container.contains(&number_1, &number_2));
    println!("First number: {}", container.first()); // 3
    println!("Last number: {}", container.last()); // 10

    println!("The difference is: {}", difference(&container)); // 7
}