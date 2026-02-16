use hashline_tools::{Cli, Commands, cmd_read, cmd_edit};
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Read { file_path, offset, limit } => {
            let result = cmd_read(&file_path, offset, limit)?;
            println!("{}", result);
        }
        Commands::Edit { file_path, edits } => {
            let result = cmd_edit(&file_path, &edits)?;
            println!("{}", result);
        }
    }
    Ok(())
}
