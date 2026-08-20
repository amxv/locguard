pub mod app;
pub mod cli;
pub mod config;
pub mod discovery;
pub mod init;
pub mod output;
pub mod paths;
pub mod policy;
pub mod scanner;

use clap::{CommandFactory, Parser, error::ErrorKind};

use crate::cli::Cli;

pub fn run() -> u8 {
    match Cli::try_parse() {
        Ok(cli) => match app::execute(cli) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("Error: {error:#}");
                2
            }
        },
        Err(error) => {
            let code = match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
            let _ = error.print();
            code
        }
    }
}

pub fn command() -> clap::Command {
    Cli::command()
}
