use clap::Parser;
use mdread::cli::Cli;

fn main() {
    let cli = Cli::parse();
    println!("{cli:?}");
}
