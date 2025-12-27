# Ruble
Rust CLI to crawl folders for UBL XML invoices and generate PDFs, with optional extraction of embedded PDFs.

## Project layout
- `src/ruble` - Rust CLI source and tests

## Build
```bash
cd src/ruble
cargo build
```

## Run
```bash
cd src/ruble
cargo run -- <input-dir> --output <output-dir>
```

Example (scan current directory and emit PDFs next to each UBL file):
```bash
cd src/ruble
cargo run -- .
```

Example (run against the anonymized test bill):
```bash
cd src/ruble
cargo run -- /workspace/bills --output /workspace/bills/out
```

### Options
- `--output <dir>`: Write generated PDFs to a single output directory (defaults to each file's directory).
- `--extensions xml,ubl`: Comma-separated file extensions to treat as UBL.
- `--no-embedded`: Skip extracting embedded PDFs from the XML.

## Tests
```bash
cd src/ruble
cargo test
```
