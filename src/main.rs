use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use anyhow::{Context, Result};
use tracing::{debug, instrument};

#[cfg(windows)]
const SYSTEM_PATH_SPERATOR: &str = ";";
#[cfg(not(windows))]
const SYSTEM_PATH_SPERATOR: &str = ":";

fn main() -> Result<ExitCode> {
    let prompt = "$ ";

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let system_path = env::var("PATH")?;
    debug!(system_path);

    loop {
        print!("{prompt}");
        io::stdout().flush()?;

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        debug!(input);

        if input.is_empty() {
            continue;
        }

        let cmd = parse(&input)?;
        debug!(?cmd);

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
                debug!("executable builtin command 'echo'");
                let output = cmd.args.join(" ");
                println!("{output}");
            }
            "type" => {
                debug!("executable builtin command 'type'");
                for arg in cmd.args {
                    if ["exit", "echo", "type"].contains(&arg.as_str()) {
                        println!("{arg} is a shell builtin")
                    } else if let Some(executable_path) = find_executable(&system_path, &arg) {
                        println!("{arg} is {}", executable_path.display());
                    } else {
                        println!("{arg}: not found");
                    }
                }
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

#[instrument]
fn parse(input: &str) -> Result<Cmd> {
    let mut components = input.split_whitespace();
    let program = components
        .next()
        .expect("expected input to not be empty")
        .to_owned();

    let args = components.map(|s| s.to_owned()).collect();

    Ok(Cmd { program, args })
}

fn find_executable(system_path: &str, executable_name: &str) -> Option<PathBuf> {
    let executable_name_with_exe = format!("{executable_name}.exe");

    for path in system_path.split(SYSTEM_PATH_SPERATOR) {
        debug!("searching path {path} for executable");
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };

        for entry in entries {
            let entry = entry.unwrap();
            if entry.file_name() == executable_name
                || entry.file_name() == executable_name_with_exe.as_str()
            {
                return Some(entry.path());
            }
        }
    }

    None
}
