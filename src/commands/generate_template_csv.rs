use std::fs;

use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::cli::GenerateTemplateCsvArgs;

const TEMPLATE_CSV: &str =
    "benefit,category,merchant,amount,description,purchaseDate,receiptPath\n";

pub fn run(args: GenerateTemplateCsvArgs) -> Result<()> {
    if args.output_path.exists() {
        bail!(
            "File '{}' already exists. Please delete it first, or set a different `--output-path`.",
            args.output_path.display()
        );
    }
    fs::write(&args.output_path, TEMPLATE_CSV).with_context(|| {
        format!(
            "Failed to write template CSV to {}",
            args.output_path.display()
        )
    })?;
    println!(
        "{}",
        format!("Wrote template CSV to {}", args.output_path.display()).green()
    );
    Ok(())
}
