use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use walkdir::WalkDir;

use ruble::{create_invoice_pdf, extract_embedded_pdf, parse_ubl_invoice, EmbeddedPdf};

#[derive(Parser, Debug)]
#[command(name = "ruble", version, about = "Convert UBL invoices to PDFs")]
struct Cli {
    /// Input directory to scan for UBL files
    #[arg(default_value = ".")]
    input: PathBuf,

    /// Output directory for generated PDFs (defaults to each file's directory)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Comma-separated list of file extensions to treat as UBL
    #[arg(long, default_value = "xml,ubl")]
    extensions: String,

    /// Skip extracting embedded PDFs
    #[arg(long)]
    no_embedded: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let extensions = parse_extensions(&cli.extensions);
    let mut processed = 0usize;
    let mut failures = 0usize;

    for entry in WalkDir::new(&cli.input).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !matches_extension(path, &extensions) {
            continue;
        }

        match process_file(path, cli.output.as_ref(), !cli.no_embedded) {
            Ok(_) => processed += 1,
            Err(err) => {
                failures += 1;
                eprintln!("ERROR {}: {err:#}", path.display());
            }
        }
    }

    println!("Processed {processed} file(s) with {failures} failure(s).");
    if failures > 0 {
        anyhow::bail!("One or more files failed to process");
    }
    Ok(())
}

fn parse_extensions(input: &str) -> HashSet<String> {
    input
        .split(',')
        .map(|ext| ext.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|ext| !ext.is_empty())
        .collect()
}

fn matches_extension(path: &Path, extensions: &HashSet<String>) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => extensions.contains(&ext.to_ascii_lowercase()),
        None => false,
    }
}

fn process_file(path: &Path, output_root: Option<&PathBuf>, extract_embedded: bool) -> Result<()> {
    let xml = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let data = parse_ubl_invoice(&xml).with_context(|| "parse UBL invoice")?;

    let out_dir = output_root
        .map(PathBuf::from)
        .or_else(|| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let invoice_id = if data.invoice_number.is_empty() {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("invoice")
            .to_string()
    } else {
        data.invoice_number.clone()
    };

    let generated_pdf = out_dir.join(format!("invoice_{invoice_id}_generated.pdf"));
    create_invoice_pdf(&data, &generated_pdf)?;
    println!("OK Generated PDF: {}", generated_pdf.display());

    if extract_embedded {
        if let Some(embedded) = extract_embedded_pdf(&xml)? {
            let embedded_path = out_dir.join(format!("invoice_{invoice_id}_embedded.pdf"));
            write_embedded_pdf(&embedded, &embedded_path)?;
            println!("OK Embedded PDF: {}", embedded_path.display());
        }
    }

    Ok(())
}

fn write_embedded_pdf(embedded: &EmbeddedPdf, output_path: &Path) -> Result<()> {
    fs::write(output_path, &embedded.bytes)
        .with_context(|| format!("write {}", output_path.display()))
}
