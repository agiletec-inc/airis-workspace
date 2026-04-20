use anyhow::Result;
use clap::{Command, CommandFactory};
use clap_complete::{Shell, generate};
use std::io;

pub fn run(shell: Shell) -> Result<()> {
    let mut cmd = crate::cli::Cli::command();
    print_completions(shell, &mut cmd);
    Ok(())
}

fn print_completions<G: clap_complete::Generator>(generator: G, cmd: &mut Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}
