use clap::Parser;
use mordtui::{
    cli::{Cli, Command},
    engine,
    store::Store,
    tui,
};
fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut store = Store::open(&cli.db)?;
    if cli.command.is_none() || matches!(cli.command, Some(Command::Tui)) {
        return tui::run(&mut store, &cli);
    }
    let value = engine::execute(&mut store, &cli)?;
    println!(
        "{}",
        if cli.json {
            serde_json::to_string(&value)?
        } else {
            tui::pretty(&value)
        }
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
