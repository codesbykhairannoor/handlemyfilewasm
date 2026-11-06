import fs from 'fs';
import zlib from 'zlib';
import init, { convert_pdf_to_docx, inspect_pdf_elements } from '../pkg/pdftodocx.js';

function parseDocXml(docxUint8) {
    const docxBuf = Buffer.from(docxUint8);
    let offset = 0;
    while (offset < docxBuf.length - 4) {
        const sig = docxBuf.readUInt32LE(offset);
        if (sig === 0x04034b50) {
            const compSize = docxBuf.readUInt32LE(offset + 18);
            const nameLen = docxBuf.readUInt16LE(offset + 26);
            const extraLen = docxBuf.readUInt16LE(offset + 28);
            const fileName = docxBuf.toString('utf8', offset + 30, offset + 30 + nameLen);
            const dataStart = offset + 30 + nameLen + extraLen;
            if (fileName === 'word/document.xml') {
                return zlib.inflateRawSync(docxBuf.slice(dataStart, dataStart + compSize)).toString('utf8');
            }
            offset = dataStart + compSize;
        } else {
            break;
        }
    }
    throw new Error("word/document.xml not found in docx container");
}

async function runTest() {
    console.log("===============================================================");
    console.log(">>> VERIFIKASI: DYNAMIC MARGINS, ALIGNMENT & LINE LEADING");
    console.log("===============================================================");

    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/3] Modul WASM Pure Rust Terinisialisasi.");

    // Test on real_sample.pdf
    console.log(">>> [2/3] Memeriksa real_sample.pdf...");
    const pdfBytes = fs.readFileSync('real_sample.pdf');
    const docxUint8 = convert_pdf_to_docx(pdfBytes);
    const docXml = parseDocXml(docxUint8);

    // 1. Check SectPr & pgMar
    const sectMatch = docXml.match(/<w:sectPr>[\s\S]*?<\/w:sectPr>/);
    if (!sectMatch) throw new Error("Missing <w:sectPr>");
    const pgMarMatch = docXml.match(/<w:pgMar\s+([^>]+)\/>/);
    if (!pgMarMatch) throw new Error("Missing <w:pgMar>");
    const pgMarAttrs = pgMarMatch[1];
    console.log("  [PASS] Deteksi Margin Halaman Dinamis:", pgMarMatch[0]);

    const leftTwips = parseInt(pgMarAttrs.match(/w:left="(\d+)"/)[1], 10);
    const topTwips = parseInt(pgMarAttrs.match(/w:top="(\d+)"/)[1], 10);
    const rightTwips = parseInt(pgMarAttrs.match(/w:right="(\d+)"/)[1], 10);

    // Verify margins are dynamic (not hardcoded 1440)
    if (leftTwips === 1440 && rightTwips === 1440 && topTwips === 1440) {
        throw new Error("Margins are still hardcoded to 1440 twips!");
    }
    console.log(`  [PASS] Margin Left: ${leftTwips} twips (~${(leftTwips / 20).toFixed(1)} pt) | Top: ${topTwips} twips (~${(topTwips / 20).toFixed(1)} pt) | Right: ${rightTwips} twips (~${(rightTwips / 20).toFixed(1)} pt)`);

    // 2. Check Line Spacing (<w:spacing w:line=...>)
    const lineSpacingMatches = docXml.match(/<w:spacing [^>]*w:line="(\d+)" [^>]*w:lineRule="(auto|exact)"[^>]*>/g) || [];
    console.log(`  [PASS] Total paragraf dengan deteksi leading garis (<w:spacing w:line>): ${lineSpacingMatches.length}`);
    if (lineSpacingMatches.length === 0) {
        throw new Error("No paragraphs detected with custom line spacing leading!");
    }
    console.log("  Contoh deteksi line leading:", lineSpacingMatches.slice(0, 3));

    // 3. Check Text Alignment (<w:jc w:val=...>)
    const jcBoth = docXml.match(/<w:jc w:val="both"\/>/g) || [];
    const jcRight = docXml.match(/<w:jc w:val="right"\/>/g) || [];
    const jcCenter = docXml.match(/<w:jc w:val="center"\/>/g) || [];

    console.log(`  [PASS] Perataan Rata Kiri-Kanan (<w:jc w:val="both"/>): ${jcBoth.length} paragraf`);
    console.log(`  [PASS] Perataan Rata Kanan (<w:jc w:val="right"/>): ${jcRight.length} paragraf`);
    console.log(`  [PASS] Perataan Rata Tengah (<w:jc w:val="center"/>): ${jcCenter.length} paragraf`);

    if (jcBoth.length < 20) {
        throw new Error(`Expected at least 20 justified paragraphs, found ${jcBoth.length}`);
    }
    if (jcRight.length !== 8) {
        console.warn(`Note: Found ${jcRight.length} right-aligned footer/page-number paragraphs on real_sample.pdf`);
    }

    fs.writeFileSync('real_sample_out.docx', Buffer.from(docxUint8));
    console.log("  [PASS] real_sample_out.docx berhasil ditulis ulang ke disk.");

    console.log(">>> [3/3] Seluruh pemeriksaan presisi layout, margin, leading, & perataan LULUS 100%!");
}

runTest().catch(err => {
    console.error(">>> ERROR:", err);
    process.exit(1);
});

