use std::fs;
use pdftodocx::pdf::PdfParser;
use pdftodocx::pdf::predictor::decode_predictor;

#[test]
fn test_png_predictor_paeth() {
    // Test synthetic row using Paeth (Filter Type 4)
    let raw = vec![
        4, 10, 20, 30, // Row 1: filter type 4
        4, 5, 5, 5,    // Row 2: filter type 4
    ];
    let decoded = decode_predictor(&raw, 15, 3, 1, 8);
    assert!(decoded.is_ok());
    let bytes = decoded.unwrap();
    assert_eq!(bytes.len(), 6);
}

#[test]
fn test_parse_sample_pdf() {
    let sample_path = "sample.pdf";
    if let Ok(pdf_bytes) = fs::read(sample_path) {
        let parser = PdfParser::new();
        let result = parser.extract_raw_pages(&pdf_bytes);
        assert!(result.is_ok(), "Failed to parse sample.pdf: {:?}", result.err());
        
        let (_doc, pages) = result.unwrap();
        assert!(!pages.is_empty(), "sample.pdf should have at least 1 page");
        
        for p in &pages {
            println!("Page {}: {} x {} pt, content stream length: {} bytes, fonts: {}, xobjects: {}",
                p.page_number, p.width, p.height, p.content_bytes.len(), p.fonts.len(), p.xobjects.len());
            assert!(p.width > 0.0);
            assert!(p.height > 0.0);
            assert!(!p.content_bytes.is_empty(), "Page content stream should not be empty");
        }
    } else {
        println!("Warning: sample.pdf not found, skipping integration test");
    }
}

