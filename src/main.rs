use std::{
    io::{self, Write},
    process::ExitCode,
};

use anyhow::{Context, Result};

fn main() -> Result<ExitCode> {
    let prompt = "$ ";

    loop {
        print!("{prompt}");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.is_empty() {
            continue;
        }

        let cmd = parse(&input)?;

        match cmd.program.as_str() {
            "exit" => {
                let Some(exit_code) = cmd.args.first() else {
                    return Ok(ExitCode::SUCCESS);
                };

                let exit_code = exit_code.parse::<u8>().context(
                    "unable to parse exit code, it must be a number in the range [0,255]",
                )?;

                return Ok(exit_code.into());
            }
            "echo" => {
                let output = cmd.args.join(" ");
                println!("{output}\n");
            }
            program => println!("{program}: command not found"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cmd {
    program: String,
    args: Vec<String>,
}

fn parse(input: &str) -> Result<Cmd> {
    let mut components = input.split_whitespace();
    let program = components
        .next()
        .expect("expected input to not be empty")
        .to_owned();

    let args = components.map(|s| s.to_owned()).collect();

    Ok(Cmd { program, args })
}
