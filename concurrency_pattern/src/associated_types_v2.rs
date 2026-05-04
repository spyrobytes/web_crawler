/**
 * Associated Types - Concepts and Examples (part 2/2)
 * 
 * The use of *Associated types* improves the overall readability of code by 
 * moving inner types locally into a trait as output types. This is similar to
 * the way that generic types parameters are used in functions to abstract over
 * concrete types.
 * 
 */
// A tuple struct containing two fields of type `i32`.
struct Container(i32, i32); 

// `A` and `B` are defined in the trait via the `type` keyword.
// (Note: `type` in this context is different from `type` when used for
// aliases).
trait Contains {
    // associated types `A` and `B`. The syntax `type A;` defines an
    // associated type named `A` without specifying its concrete type.
    // Note that this is not a type alias, though both use the `type`
    // keyword.
    type A;
    type B;

    // Updated syntax to refer to these new types generically.
    fn contains(&self, _: &Self::A, _: &Self::B) -> bool;
    fn first(&self) -> i32; // Doesn't explicitly require `A` or `B`.
    fn last(&self) -> i32;  // Doesn't explicitly require `A` or `B`.
}

impl Contains for Container {
    // we provide a definition of the associated types to match the concrete
    // type `Container`. This is the only change!
    type A = i32; // associated type binding
    type B = i32; // associated type binding
    // True if the numbers stored are equal.
    fn contains(&self, number_1: &i32, number_2: &i32) -> bool {
        (&self.0 == number_1) && (&self.1 == number_2)
    }

    // Grab the first number.
    fn first(&self) -> i32 { self.0 }

    // Grab the last number.
    fn last(&self) -> i32 { self.1 }
}

// utility functions that use the trait `Contains` are no longer required to
// express `A` or `B` at all. `C` has the associated types `A` and `B` in its
// definition.
fn difference<C: Contains>(container: &C) -> i32 {
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