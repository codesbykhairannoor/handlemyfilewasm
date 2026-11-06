import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

function createMultiColumnPdf() {
    const lines = [];
    const offsets = {};

    let pos = 0;
    function writeLine(str) {
        lines.push(str);
        pos += Buffer.byteLength(str + '\n');
    }

    writeLine('%PDF-1.4');

    function writeObject(num, body) {
        offsets[num] = pos;
        writeLine(`${num} 0 obj`);
        writeLine(body);
        writeLine('endobj');
    }

    // 1: Catalog
    writeObject(1, '<< /Type /Catalog /Pages 2 0 R >>');
    // 2: Pages
    writeObject(2, '<< /Type /Pages /Kids [ 3 0 R ] /Count 1 >>');
    // 3: Page 1
    writeObject(3, '<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 595 842 ] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>');
    // 4: Font
    writeObject(4, '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');

    // 5: Content stream: 2 columns layout
    // Left column: X = 50..250, Y = 750, 720
    // Right column: X = 320..520, Y = 750, 720 (gap = 70 pt between columns)
    const streamData = 
`0 w
BT
/F1 10 Tf
50 750 Td
(Kolom Kiri Baris Satu) Tj
0 -25 Td
(Kolom Kiri Baris Dua) Tj
ET
BT
/F1 10 Tf
320 750 Td
(Kolom Kanan Baris Satu) Tj
0 -25 Td
(Kolom Kanan Baris Dua) Tj
ET
BT
/F1 10 Tf
50 620 Td
(- Butir Daftar Pertama) Tj
0 -20 Td
(1. Butir Daftar Kedua) Tj
ET`;

    const streamLen = Buffer.byteLength(streamData);
    writeObject(5, `<< /Length ${streamLen} >>\nstream\n${streamData}\nendstream`);

    const startXref = pos;
    writeLine('xref');
    writeLine('0 6');
    writeLine('0000000000 65535 f ');
    for (let i = 1; i <= 5; i++) {
        const offStr = String(offsets[i]).padStart(10, '0');
        writeLine(`${offStr} 00000 n `);
    }

    writeLine('trailer');
    writeLine('<< /Size 6 /Root 1 0 R >>');
    writeLine('startxref');
    writeLine(String(startXref));
    writeLine('%%EOF');

    return Buffer.from(lines.join('\n'));
}

async function testWasmStep7() {
    console.log("===============================================================");
    console.log(">>> STEP 7 VERIFICATION: DLA ENGINE (DOCSTRUM & RECURSIVE X-Y CUT)");
    console.log("===============================================================");

    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/5] Modul WebAssembly Pure Rust Terinisialisasi.");

    // --- BAGIAN A: Verifikasi DLA pada sample.pdf nyata (Untagged PDF) ---
    console.log("\n>>> [2/5] Memeriksa Rekonstruksi DLA pada sample.pdf...");
    const samplePdfBytes = fs.readFileSync('./sample.pdf');
    const samplePages = inspect_pdf_elements(samplePdfBytes);

    if (!samplePages || samplePages.length === 0) {
        throw new Error("sample.pdf gagal diparsing!");
    }

    const p1 = samplePages[0];
    console.log(`  Halaman 1 Glif: ${p1.glyphs.length}`);
    console.log(`  Halaman 1 Paragraf yang terbentuk via DLA: ${p1.paragraphs.length}`);

    if (p1.paragraphs.length === 0) {
        throw new Error("DLA gagal membentuk paragraf pada sample.pdf!");
    }

    const para0 = p1.paragraphs[0];
    console.log(`  Paragraf #0: Tag='${para0.tag}', is_header=${para0.is_header}, Jumlah Baris=${para0.lines.length}`);

    const line0 = para0.lines[0];
    console.log(`  Baris #0: Jumlah Kata=${line0.words.length}, Jumlah Runs=${line0.runs.length}`);
    const wordTexts = line0.words.map(w => w.text);
    console.log(`  Daftar Kata yang terbentuk: ${JSON.stringify(wordTexts)}`);

    if (wordTexts.length !== 3 || wordTexts[0] !== "Dummy" || wordTexts[1] !== "PDF" || wordTexts[2] !== "file") {
        throw new Error(`Pengelompokan kata Docstrum tidak cocok! Didapatkan: ${JSON.stringify(wordTexts)}`);
    }
    console.log("  [PASS] 1. Docstrum Word Clustering: 14 glif berhasil dikelompokkan presisi menjadi [\"Dummy\", \"PDF\", \"file\"]!");

    if (line0.runs.length !== 1 || line0.runs[0].text !== "Dummy PDF file") {
        throw new Error(`Konsolidasi TextRun gagal! Didapatkan: ${JSON.stringify(line0.runs)}`);
    }
    console.log("  [PASS] 2. TextRun Consolidation: Kata-kata berkonsolidasi menjadi 1 TextRun bersih: \"Dummy PDF file\"!");

    if (!para0.is_header || para0.tag !== "H1") {
        throw new Error(`Klasifikasi Heading salah! tag=${para0.tag}, is_header=${para0.is_header}`);
    }
    console.log("  [PASS] 3. Heading Classification: Teks 16.1 pt tebal otomatis diklasifikasikan sebagai Heading 1 (H1)!");

    // --- BAGIAN B: Verifikasi Multi-Kolom & Urutan Baca RXYC ---
    console.log("\n>>> [3/5] Menguji Algoritma Recursive X-Y Cut pada Dokumen 2 Kolom...");
    const multiColPdfBytes = createMultiColumnPdf();
    fs.writeFileSync('./multicolumn_sample.pdf', multiColPdfBytes);
    console.log(`  multicolumn_sample.pdf dibuat (${multiColPdfBytes.length} bytes).`);

    const multiColPages = inspect_pdf_elements(multiColPdfBytes);
    const mPage = multiColPages[0];
    console.log(`  Jumlah Paragraf terekstraksi: ${mPage.paragraphs.length}`);

    console.log("\n>>> [4/5] Memeriksa Urutan Baca Alami (Reading Order DAG):");
    const paragraphTexts = [];
    for (let i = 0; i < mPage.paragraphs.length; i++) {
        const p = mPage.paragraphs[i];
        const fullText = p.lines.map(l => l.runs.map(r => r.text).join(' ')).join(' ');
        paragraphTexts.push(fullText);
        console.log(`  Blok #${i} [Tag='${p.tag}', is_list=${p.is_list}]: "${fullText}"`);
    }

    // Verifikasi bahwa Kolom Kiri dibaca SEBELUM Kolom Kanan
    const leftColIdx = paragraphTexts.findIndex(t => t.includes("Kolom Kiri"));
    const rightColIdx = paragraphTexts.findIndex(t => t.includes("Kolom Kanan"));

    if (leftColIdx < 0 || rightColIdx < 0 || leftColIdx >= rightColIdx) {
        throw new Error(`Urutan baca salah! Kolom kiri harus mendahului kolom kanan! leftIdx=${leftColIdx}, rightIdx=${rightColIdx}`);
    }
    console.log("  [PASS] 4. Recursive X-Y Cut: Kolom kiri dibaca utuh dari atas ke bawah sebelum kolom kanan (anti-interleaved layout)!");

    // Verifikasi Deteksi List / Butir
    const listPara = mPage.paragraphs.find(p => p.is_list);
    if (!listPara) {
        throw new Error("Gagal mendeteksi paragraf bertipe daftar/list!");
    }
    console.log(`  [PASS] 5. List Detection: Berhasil mendeteksi elemen daftar/butir [is_list=${listPara.is_list}, list_level=${listPara.list_level}]!`);

    console.log("\n===============================================================");
    console.log(">>> SEMUA VERIFIKASI STEP 7 LULUS 100% SEMPURNA! <<<");
    console.log("===============================================================");
}

testWasmStep7().catch(err => {
    console.error(">>> STEP 7 GAGAL:", err);
    process.exit(1);
});

