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

        let input = input.trim_start().trim_newline();

        if input.is_empty() {
            continue;
        }

        let cmd = parse(input).context("unable to parse prompt")?;
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
    assert!(!input.is_empty());

    let mut components = Vec::new();

    let mut current_component = String::new();
    let mut it = input.char_indices().peekable();
    while let Some((i, ch)) = it.next() {
        if ch == '\'' {
            let (closing_quote_index, _) = it
                .by_ref()
                .find(|(_, ch)| *ch == '\'')
                .context("did not find a closing single quote")?;

            components.push(input[i + 1..closing_quote_index].to_owned());
        } else if ch == '"' {
            let (closing_quote_index, _) = it
                .by_ref()
                .find(|(_, ch)| *ch == '"')
                .context("did not find a closing double quote")?;

            components.push(input[i + 1..closing_quote_index].to_owned());
        } else if ch == '\\' {
            let (_, next_char) = it
                .next()
                .context("expected a character after '\', but got nothing")?;

            current_component.push(next_char);
        } else if ch.is_whitespace() {
            components.push(current_component);
            current_component = String::new();
            // ignore all following whitespace
            while let Some((_, search_char)) = it.peek() {
                if !search_char.is_whitespace() {
                    break;
                }

                it.next();
            }
        } else {
            current_component.push(ch);
        }
    }

    if !current_component.is_empty() {
        components.push(current_component);
    }

    let program = components.remove(0);

    Ok(Cmd {
        program,
        args: components,
    })
}

fn find_executable(search_path: &str, executable_name: &str) -> Option<PathBuf> {
    debug!(executable_name, "searching for executable");

    let executable_name_with_suffix = format!("{executable_name}{EXE_SUFFIX}");

    for path in search_path.split(SYSTEM_PATH_SPERATOR) {
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

#[instrument]
fn builtin_exit(args: Vec<String>) -> Result<Option<ExitCode>> {
    let Some(exit_code) = args.first() else {
        return Ok(Some(ExitCode::SUCCESS));
    };

    let exit_code = exit_code
        .parse::<u8>()
        .context("unable to parse exit code, it must be a number in the range [0,255]")?;

    Ok(Some(exit_code.into()))
}

#[instrument]
fn builtin_echo(args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'echo'");
    let output = args.join(" ");
    println!("{output}");
    Ok(None)
}

#[instrument]
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

#[instrument]
fn builtin_pwd(_args: Vec<String>) -> Result<Option<ExitCode>> {
    debug!("executable builtin command 'pwd'");

    println!("{}", env::current_dir()?.display());

    Ok(None)
}

#[instrument]
fn builtin_cd(args: Vec<String>) -> Result<Option<ExitCode>> {
    assert!(args.len() == 1);

    debug!("executable builtin command 'cd'");

    let input = args
        .into_iter()
        .next()
        .expect("expected path to be passed to 'cd' command");

    if input == "~" {
        let home = env::var("HOME")?;
        debug!(home, "changing to home directory");

        env::set_current_dir(home)?;
        return Ok(None);
    }

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

trait StrExt {
    fn trim_newline(&self) -> &Self;
}

impl StrExt for str {
    fn trim_newline(&self) -> &Self {
        let mut output = self;
        if let Some(stripped) = output.strip_suffix("\n") {
            output = stripped;
            if let Some(stripped) = output.strip_suffix("\r") {
                output = stripped;
            }
        }

        output
    }
}
