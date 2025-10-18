use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use kayton_emitter_bc::emit;
use kayton_front::parse_to_hir;
use kayton_front::{diagnostics::Diagnostic, source::SourceMap};
use kayton_sema::fast::analyze;
use kayton_vm::{run_module, Value, VmError};

#[derive(Parser)]
#[command(name = "kayton", author, version, about = "Kayton language CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse, type-check, emit bytecode, and run a program
    Run { file: PathBuf },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { file } => run_program(file),
    }
}

fn run_program(path: PathBuf) -> Result<()> {
    let parse = parse_to_hir(&path)?;
    report_diagnostics(&parse.diagnostics, &parse.source_map)?;

    let analysis = analyze(&parse.module);
    report_diagnostics(&analysis.diagnostics, &parse.source_map)?;

    let bytecode = emit(&parse.module, &analysis).context("failed to emit bytecode")?;
    let value = run_module(&bytecode, "main").map_err(map_vm_error)?;
    if !matches!(value, Value::Unit) {
        println!("{}", format_value(&value));
    }
    Ok(())
}

fn report_diagnostics(diags: &[Diagnostic], source_map: &SourceMap) -> Result<()> {
    if diags.is_empty() {
        return Ok(());
    }
    for diag in diags {
        if let Some(file) = source_map.get(diag.span.source) {
            let (line, col) = if diag.span.start == 0 {
                (1, 1)
            } else {
                file.line_col(diag.span.start as usize)
            };
            eprintln!(
                "{:?}: {} ({}:{}:{})",
                diag.severity,
                diag.message,
                file.path.display(),
                line,
                col
            );
        } else {
            eprintln!("{:?}: {}", diag.severity, diag.message);
        }
        for note in &diag.notes {
            eprintln!("  note: {note}");
        }
    }
    Err(anyhow!("encountered diagnostics"))
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Int(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::String(s) => s.clone(),
        Value::Unit => String::new(),
    }
}

fn map_vm_error(err: VmError) -> anyhow::Error {
    match err {
        VmError::EntryNotFound(name) => anyhow!("entry function `{name}` not found"),
        other => anyhow!(other),
    }
}
