use anyhow::{Context, Result};
use clap::Parser as CliParser;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

#[derive(CliParser)]
struct CliArguments {
    path: std::path::PathBuf,
}

fn main() -> Result<()> {
    let args = CliArguments::parse();
    let path = args.path;

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Couldn't read file `{}`", path.display()))?;

    let source_type =
        SourceType::from_path(path).with_context(|| format!("Couldn't identify source type"))?;
    let allocator = Allocator::default();
    let parser_result = Parser::new(&allocator, &content, source_type).parse();
    let program = parser_result.program;

    println!("{:#?}", program);

    Ok(())
}
