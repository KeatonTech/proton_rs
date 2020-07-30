extern crate strum;
#[macro_use]
extern crate strum_macros;

#[cfg(test)]
#[macro_use]
pub mod test_macros;

pub mod compute_graph;
pub mod node;

fn main() {
    println!("Hello, world!");
}
