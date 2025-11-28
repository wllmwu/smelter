use clap::Parser;

#[derive(Parser)]
struct CliArguments {
    path: std::path::PathBuf,
}

fn main() {
    let args = CliArguments::parse();
    println!("path: {:?}", args.path);
}
