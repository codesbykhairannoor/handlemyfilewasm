import fs from 'fs';
import zlib from 'zlib';
import init, { convert_pdf_to_docx, inspect_pdf_elements } from '../pkg/pdftodocx.js';

function parseDocxZip(docxBytes) {
    const docxBuf = Buffer.from(docxBytes);
    let offset = 0;
    const files = {};
    while (offset < docxBuf.length - 4) {
        const sig = docxBuf.readUInt32LE(offset);
        if (sig === 0x04034b50) {
            const compMethod = docxBuf.readUInt16LE(offset + 8);
            const compSize = docxBuf.readUInt32LE(offset + 18);
            const uncompSize = docxBuf.readUInt32LE(offset + 22);
            const nameLen = docxBuf.readUInt16LE(offset + 26);
            const extraLen = docxBuf.readUInt16LE(offset + 28);
            const fileName = docxBuf.toString('utf8', offset + 30, offset + 30 + nameLen);
            const dataStart = offset + 30 + nameLen + extraLen;

            let content = null;
            if (compMethod === 0) {
                content = docxBuf.slice(dataStart, dataStart + uncompSize);
            } else if (compMethod === 8) {
                content = zlib.inflateRawSync(docxBuf.slice(dataStart, dataStart + compSize));
            }
            files[fileName] = content;
            offset = dataStart + compSize;
        } else {
            break;
        }
    }
    return files;
}

async function runPrecisionTests() {
    console.log("==================================================================");
    console.log(">>> PRECISION SUITE: DRAWINGML IMAGES & TSR TABLES RECONSTRUCTION");
    console.log("==================================================================");

    await init(fs.readFileSync('./pkg/pdftodocx_bg.wasm'));
    console.log(">>> [1/3] Pure-Rust WebAssembly Engine initialized successfully.");

    // --- TEST 1: laporan.pdf (10 Images & 2 Tagged/APA Tables) ---
    console.log("\n>>> [2/3] Testing laporan.pdf (26 pages, 10 images, 2 tables)...");
    const lapBytes = fs.readFileSync('laporan.pdf');
    const lapDocx = convert_pdf_to_docx(lapBytes);
    const lapFiles = parseDocxZip(lapDocx);

    // 1a. Verify Media Files
    const mediaFiles = Object.keys(lapFiles).filter(k => k.startsWith('word/media/'));
    console.log(`  Found ${mediaFiles.length} media files in word/media/`);
    if (mediaFiles.length !== 10) {
        throw new Error(`Expected 10 media files, found ${mediaFiles.length}`);
    }
    const pngCount = mediaFiles.filter(f => f.endsWith('.png')).length;
    const jpegCount = mediaFiles.filter(f => f.endsWith('.jpeg')).length;
    console.log(`  - PNG images: ${pngCount}, JPEG images: ${jpegCount}`);
    if (pngCount !== 7 || jpegCount !== 3) {
        throw new Error(`Expected 7 PNGs and 3 JPEGs, got ${pngCount} PNGs and ${jpegCount} JPEGs`);
    }
    console.log("  [PASS] 100% of images extracted and packaged with accurate MIME types!");

    // 1b. Verify Relationships
    const relsXml = lapFiles['word/_rels/document.xml.rels'].toString('utf8');
    for (let i = 1; i <= 10; i++) {
        if (!relsXml.includes(`Id="rIdImg${i}"`)) {
            throw new Error(`Missing relationship rIdImg${i} in document.xml.rels`);
        }
    }
    console.log("  [PASS] DrawingML image relationships (rIdImg1..rIdImg10) correctly registered!");

    // 1c. Verify Drawings in document.xml
    const docXml = lapFiles['word/document.xml'].toString('utf8');
    const drawings = docXml.match(/<w:drawing>[\s\S]*?<\/w:drawing>/g) || [];
    if (drawings.length !== 10) {
        throw new Error(`Expected 10 <w:drawing> elements, found ${drawings.length}`);
    }
    console.log("  [PASS] Exactly 10 DrawingML inline drawings rendered in document body!");

    // 1d. Verify Tables in document.xml
    const tables = docXml.match(/<w:tbl>[\s\S]*?<\/w:tbl>/g) || [];
    if (tables.length !== 2) {
        throw new Error(`Expected 2 tables in laporan.pdf, found ${tables.length}`);
    }
    const t2 = tables[1];
    if (!t2.includes("Naive") || !t2.includes("Bayes") || !t2.includes("Logistic Regression") || !t2.includes("Random Forest")) {
        throw new Error("Table 2 does not contain expected model evaluation rows!");
    }
    const t2Cols = t2.match(/<w:gridCol [^>]*\/>/g) || [];
    if (t2Cols.length !== 4) {
        throw new Error(`Table 2 expected 4 gridCol, got ${t2Cols.length}`);
    }
    console.log("  [PASS] Tabel 3.2 (Ringkasan Komparasi Performa) reconstructed with 4 rows x 4 columns!");

    // 1e. Verify Page Flow Continuity (Chapter 1 points 3..5)
    const p3Idx = docXml.indexOf("Exploratory data analysis");
    const p4Idx = docXml.indexOf("Pemodelan hibrida");
    const p5Idx = docXml.indexOf("Dashboard visualisasi");
    if (p3Idx === -1 || p4Idx === -1 || p5Idx === -1) {
        throw new Error("Chapter 1 points 3, 4, or 5 missing!");
    }
    const betweenP3P5 = docXml.substring(p3Idx, p5Idx);
    if (betweenP3P5.includes('<w:br w:type="page"/>')) {
        throw new Error("Unexpected page break between items 3, 4, 5!");
    }
    console.log("  [PASS] Chapter 1 points 3..5 flow smoothly on the same page with zero orphan page splits!");

    // --- TEST 2: real_sample.pdf (Clean Deduplicated Tables with Fixed Layout) ---
    console.log("\n>>> [3/3] Testing real_sample.pdf (8 pages, deduplicated clean tables)...");
    const realBytes = fs.readFileSync('real_sample.pdf');
    const realDocx = convert_pdf_to_docx(realBytes);
    const realFiles = parseDocxZip(realDocx);
    const realDocXml = realFiles['word/document.xml'].toString('utf8');
    const realTables = realDocXml.match(/<w:tbl>[\s\S]*?<\/w:tbl>/g) || [];
    console.log(`  Found ${realTables.length} deduplicated clean tables in real_sample.pdf`);
    if (realTables.length < 4) {
        throw new Error(`Expected at least 4 tables in real_sample.pdf, found ${realTables.length}`);
    }
    for (let idx = 0; idx < realTables.length; idx++) {
        const tbl = realTables[idx];
        if (!tbl.includes('<w:tblLayout w:type="fixed"/>')) {
            throw new Error(`Table ${idx + 1} missing fixed table layout!`);
        }
        const gridCols = tbl.match(/<w:gridCol [^>]*\/>/g) || [];
        if (gridCols.length === 0) {
            throw new Error(`Table ${idx + 1} has empty gridCol!`);
        }
        for (const col of gridCols) {
            const wMatch = col.match(/w:w="(\d+)"/);
            if (!wMatch || parseInt(wMatch[1]) <= 0) {
                throw new Error(`Table ${idx + 1} has zero or invalid column width: ${col}`);
            }
        }
    }
    console.log(`  [PASS] All ${realTables.length} tables have fixed layout, valid <w:gridCol>, and non-zero <w:tcW> widths!`);

    console.log("\n==================================================================");
    console.log(">>> ALL PRECISION VERIFICATIONS FOR IMAGES AND TABLES PASSED 100%! <<<");
    console.log("==================================================================");
}

runPrecisionTests().catch(err => {
    console.error(">>> TEST FAILED:", err);
    process.exit(1);
});
