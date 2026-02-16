use hashline_tools::{Cli, Commands, cmd_read, cmd_edit};
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Read { file_path, offset, limit } => {
            let result = cmd_read(&file_path, offset, limit)?;
            println!("{}", result);
        }
        Commands::Edit { file_path, edits, edits_stdin } => {
            let edits_json = if edits_stdin {
                use std::io::{self, Read};
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            } else {
                edits.ok_or("--edits or --edits-stdin required")?
            };
            let result = cmd_edit(&file_path, &edits_json)?;
            println!("{}", result);
        }
    }
    Ok(())
}