# Agent Notes: Ruble

## Purpose
- `ruble` is a Rust CLI that scans a directory for UBL XML invoices and generates a PDF summary for each.
- If an embedded PDF exists in the UBL (`EmbeddedDocumentBinaryObject` with `mimeCode="application/pdf"`), it is extracted as a second file.

## Key paths
- `src/ruble/src/main.rs` - CLI entrypoint and directory crawling.
- `src/ruble/src/lib.rs` - XML parsing, embedded PDF extraction, and PDF generation helpers.

## Commands
- Build: `cd src/ruble && cargo build`
- Run: `cd src/ruble && cargo run -- <input-dir> --output <output-dir>`
- Tests: `cd src/ruble && cargo test`

## Output naming
- Generated invoice: `invoice_<invoice_id>_generated.pdf`
- Embedded PDF: `invoice_<invoice_id>_embedded.pdf`
