use std::{
    collections::HashMap,
    env::{self, consts::EXE_SUFFIX},
    fs,
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitCode},
    sync::LazyLock,
};

use anyhow::{Context, Result};
use tracing::{debug, instrument, trace};

#[cfg(windows)]
const SYSTEM_PATH_SPERATOR: &str = ";";
#[cfg(not(windows))]
const SYSTEM_PATH_SPERATOR: &str = ":";

type BuiltinFn = fn(Vec<String>) -> Result<Option<ExitCode>>;

static SYSTEM_PATH: LazyLock<String> = LazyLock::new(|| env::var("PATH").unwrap_or_default());
static BUILTINS: LazyLock<HashMap<&str, BuiltinFn>> = LazyLock::new(|| {
    HashMap::from([
        ("exit", builtin_exit as BuiltinFn),
        ("echo", builtin_echo as BuiltinFn),
        ("type", builtin_type as BuiltinFn),
        ("pwd", builtin_pwd as BuiltinFn),
        ("cd", builtin_cd as BuiltinFn),
    ])
});

fn main() -> Result<ExitCode> {
    let prompt = "$ ";

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    debug!(?SYSTEM_PATH);

    loop {
        print!("{prompt}");
        io::stdout().flush()?;

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        debug!(input);

        if input.trim().is_empty() {
            continue;
        }

        let cmd = parse(&input)?;
        debug!(?cmd);

        let program = cmd.program.as_str();
        if let Some(builtin_fn) = BUILTINS.get(program) {
            let exit_code = builtin_fn(cmd.args)?;
            if let Some(exit_code) = exit_code {
                return Ok(exit_code);
            }
        } else if find_executable(&SYSTEM_PATH, program).is_some() {
            debug!(program, "executing program");
            let output = Command::new(program).args(cmd.args).output().unwrap();

            io::stdout().write_all(&output.stdout).unwrap();
            io::stderr().write_all(&output.stderr).unwrap();
        } else {
            println!("{program}: command not found");
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
    debug!(executable_name, "searching for executable");

    let executable_name_with_suffix = format!("{executable_name}{EXE_SUFFIX}");

    for path in system_path.split(SYSTEM_PATH_SPERATOR) {
        trace!(
            executable_name,
            executable_name_with_suffix,
            path,
            "searching for executable"
        );
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };

        for entry in entries {
            let entry = entry.unwrap();
            if entry.file_name() == executable_name
                || entry.file_name() == executable_name_with_suffix.as_str()
            {
                return Some(entry.path());
            }
        }
    }

    None
}

fn builtin_exit(args: Vec<String>) -> Result<Option<ExitCode>> {
    let Some(exit_code) = args.first() else {
        return Ok(Some(ExitCode::SUCCESS));
    };

    let exit_code = exit_code
        .parse::<u8>()
        .context("unable to parse exit code, it must be a number in the range [0,255]")?;

    Ok(Some(exit_code.into()))
}

fn builtin_echo(args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'echo'");
    let output = args.join(" ");
    println!("{output}");
    Ok(None)
}

fn builtin_type(args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'type'");

    for arg in args {
        if BUILTINS.contains_key(&arg.as_str()) {
            println!("{arg} is a shell builtin")
        } else if let Some(executable_path) = find_executable(&SYSTEM_PATH, &arg) {
            println!("{arg} is {}", executable_path.display());
        } else {
            println!("{arg}: not found");
        }
    }

    Ok(None)
}

fn builtin_pwd(_args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'pwd'");

    println!("{}", env::current_dir()?.display());

    Ok(None)
}

fn builtin_cd(args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'cd'");

    let input = args.into_iter().next().expect("cd: too many arguments");

    let cwd = env::current_dir()?;
    let input_path = match fs::canonicalize(&input) {
        Ok(path) => path,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            println!("cd: {}: No such file or directory", input);
            return Ok(None);
        }
        Err(err) => return Err(err.into()),
    };

    env::set_current_dir(input_path)?;

    debug!(?cwd);

    Ok(None)
}
