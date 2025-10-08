use std::env;
use std::process::{exit, Command};

fn main() {
    let mut args = env::args();
    let _binary = args.next();
    let Some(task) = args.next() else {
        print_usage();
        exit(1);
    };

    let extra: Vec<String> = args.collect();

    let status = match task.as_str() {
        "fmt" => run(fmt(extra)),
        "lint" => run(lint(extra)),
        "dev:check" => run(dev_check(extra)),
        _ => {
            eprintln!("unknown task: {task}");
            print_usage();
            exit(1);
        }
    };

    if !status {
        exit(1);
    }
}

fn fmt(extra: Vec<String>) -> Command {
    let mut cmd = cargo();
    cmd.arg("fmt").arg("--all");
    cmd.args(extra);
    cmd
}

fn lint(extra: Vec<String>) -> Command {
    let mut cmd = cargo();
    cmd.arg("clippy")
        .arg("--workspace")
        .arg("--all-targets")
        .arg("--all-features")
        .arg("--")
        .arg("-D")
        .arg("warnings");
    cmd.args(extra);
    cmd
}

fn dev_check(extra: Vec<String>) -> Command {
    let mut cmd = cargo();
    cmd.arg("check").arg("--workspace");
    cmd.args(extra);
    cmd
}

fn run(mut cmd: Command) -> bool {
    println!("+ {:?}", cmd);
    match cmd.status() {
        Ok(status) => status.success(),
        Err(err) => {
            eprintln!("failed to run command: {err}");
            false
        }
    }
}

fn cargo() -> Command {
    Command::new("cargo")
}

fn print_usage() {
    eprintln!("xtask <command> [args...]\n\nCommands:\n  fmt        Run 'cargo fmt --all'\n  lint       Run 'cargo clippy --all-targets --all-features -- -D warnings'\n  dev:check  Run 'cargo check --workspace'");
}
