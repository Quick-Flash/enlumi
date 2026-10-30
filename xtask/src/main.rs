mod bundle;
mod mavlink;
mod perf;
mod perf_trace;
#[path = "../../build/revision.rs"]
mod revision;

use std::{
    env,
    io::{self, Write},
    path::Path,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        None => menu(),
        Some(command) => {
            let mut remaining: Vec<_> = args.collect();
            if remaining.first().is_some_and(|arg| arg == "--") {
                remaining.remove(0);
            }
            dispatch(command, &remaining)
        }
    }
}

fn dispatch(command: &str, args: &[String]) -> Result<(), String> {
    match command {
        "run" | "run-gui" => cargo(&["run", "--locked", "-p", "enlumi", "--"], args),
        "run-release" | "run-gui-release" => cargo(
            &["run", "--locked", "--release", "-p", "enlumi", "--"],
            args,
        ),
        "build" => cargo(&["build", "--locked", "--workspace"], args),
        "build-release" => cargo(&["build", "--locked", "--release", "--workspace"], args),
        "bundle" if args.is_empty() => bundle::build(),
        "perf-summary" if args.len() == 1 => perf::summarize(Path::new(&args[0])),
        "perf-trace" if args.len() == 1 => perf_trace::summarize(Path::new(&args[0])),
        "update-mavlink" if args.len() <= 1 => mavlink::update(args.first().map(String::as_str)),
        "test" => cargo(&["test", "--locked", "--workspace"], args),
        "check" => cargo(&["check", "--locked", "--workspace"], args),
        "pre-commit" if args.is_empty() => {
            cargo(&["fmt", "--all", "--", "--check"], &[])?;
            cargo(&["check", "--locked", "--workspace"], &[])?;
            cargo(
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
                &[],
            )?;
            cargo(&["test", "--locked", "--workspace"], &[])
        }
        "help" | "-h" | "--help" => {
            println!(
                "cargo xtask [run-gui | run-gui-release | build | build-release | bundle | test | check | pre-commit | update-mavlink [version] | perf-summary <frames.csv> | perf-trace <latency.jsonl>]"
            );
            println!("No command: interactive menu. GUI commands accept log path after --.");
            Ok(())
        }
        _ => Err(format!(
            "unknown command or unsupported arguments: {command}"
        )),
    }
}

fn cargo(args: &[&str], extra: &[String]) -> Result<(), String> {
    println!("running: cargo {} {}", args.join(" "), extra.join(" "));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask belongs to enlumi workspace");
    let status = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args(args)
        .args(extra)
        .status()
        .map_err(|error| format!("cannot start cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed: {status}", args.join(" ")))
    }
}

fn menu() -> Result<(), String> {
    let commands = [
        "run-gui",
        "run-gui-release",
        "build",
        "build-release",
        "bundle",
        "test",
        "check",
        "pre-commit",
        "update-mavlink",
    ];
    println!("Enlumi xtask");
    for (index, command) in commands.iter().enumerate() {
        println!("{}. {command}", index + 1);
    }
    print!("Select command: ");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| error.to_string())?;
    let index = input
        .trim()
        .parse::<usize>()
        .map_err(|_| "invalid menu choice")?;
    let command = index
        .checked_sub(1)
        .and_then(|index| commands.get(index))
        .ok_or("invalid menu choice")?;
    dispatch(command, &[])
}
