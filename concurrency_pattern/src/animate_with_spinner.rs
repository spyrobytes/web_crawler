/**
 * Animate with spinner
 * 
 * Spicing up Rust CLI with a spinner using `spinners` crate
 * 
 * @SEE https://github.com/FGRibreau/spinners
 */
use spinners::{Spinner, Spinners};
use std::thread::sleep;
use std::time::Duration;

fn main() {
    //let mut sp = Spinner::new(Spinners::Dots9, "Waiting for 3 seconds".into());
    let mut sp = Spinner::new(Spinners::Aesthetic, "Waiting for 3 seconds".into());
    sleep(Duration::from_secs(3));
    sp.stop();
}