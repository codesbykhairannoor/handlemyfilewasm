use std::env;
use std::fs;
use std::process;
use std::time::Instant;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: pdftodocx <input.pdf> [output.docx]");
        process::exit(1);
    }

    let input_path = &args[1];
    let output_path = if args.len() >= 3 {
        args[2].clone()
    } else {
        input_path.replace(".pdf", ".docx")
    };

    println!(">>> High-Precision PDF to DOCX Converter (Pure Rust Engine)");
    println!(">>> Input: {}", input_path);
    println!(">>> Output: {}", output_path);

    let start = Instant::now();
    let pdf_bytes = match fs::read(input_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading PDF file '{}': {}", input_path, e);
            process::exit(1);
        }
    };

    let engine = pdftodocx::ConverterEngine::new();
    match engine.convert(&pdf_bytes) {
        Ok(docx_bytes) => {
            if let Err(e) = fs::write(&output_path, &docx_bytes) {
                eprintln!("Error saving DOCX to '{}': {}", output_path, e);
                process::exit(1);
            }
            let duration = start.elapsed();
            println!(
                ">>> Success! Wrote {} bytes to '{}' in {:.2?}",
                docx_bytes.len(),
                output_path,
                duration
            );
        }
        Err(e) => {
            eprintln!("Conversion failed: {}", e);
            process::exit(1);
        }
    }
}

