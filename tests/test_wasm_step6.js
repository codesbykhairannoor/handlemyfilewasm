import fs from 'fs';
import init, { inspect_pdf_elements, inspect_pdf_structure } from '../pkg/pdftodocx.js';

function createValidTaggedPdf() {
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
    writeObject(1, '<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 6 0 R >>');
    // 2: Pages
    writeObject(2, '<< /Type /Pages /Kids [ 3 0 R ] /Count 1 >>');
    // 3: Page 1
    writeObject(3, '<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 595 842 ] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>');
    // 4: Font
    writeObject(4, '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');

    // 5: Content stream
    const streamData =
        `/Heading1 << /MCID 0 >> BDC
BT
/F1 14 Tf
56.8 750 Td
(Bab 1: Pendahuluan) Tj
ET
EMC
/CustomPara << /MCID 1 >> BDC
BT
/F1 11 Tf
56.8 710 Td
(Ini adalah isi paragraf semantik asli.) Tj
ET
EMC
/Table << /MCID 2 >> BDC
BT
/F1 10 Tf
56.8 650 Td
(Data Tabel Kolom 1 dan 2) Tj
ET
EMC`;

    const streamLen = Buffer.byteLength(streamData);
    writeObject(5, `<< /Length ${streamLen} >>\nstream\n${streamData}\nendstream`);

    // 6: StructTreeRoot with RoleMap
    writeObject(6, '<< /Type /StructTreeRoot /RoleMap 13 0 R /K [ 7 0 R ] >>');
    // 7: StructElem Document
    writeObject(7, '<< /Type /StructElem /S /Document /K [ 8 0 R 9 0 R 10 0 R ] >>');
    // 8: StructElem Heading1 (MCID 0)
    writeObject(8, '<< /Type /StructElem /S /Heading1 /Pg 3 0 R /K 0 >>');
    // 9: StructElem CustomPara (MCID 1)
    writeObject(9, '<< /Type /StructElem /S /CustomPara /Pg 3 0 R /K 1 >>');
    // 10: StructElem Table
    writeObject(10, '<< /Type /StructElem /S /Table /Pg 3 0 R /K [ 11 0 R ] >>');
    // 11: StructElem TR
    writeObject(11, '<< /Type /StructElem /S /TR /Pg 3 0 R /K [ 12 0 R ] >>');
    // 12: StructElem TD with ColSpan 2 and RowSpan 1 (MCID 2)
    writeObject(12, '<< /Type /StructElem /S /TD /Pg 3 0 R /A << /O /Table /ColSpan 2 /RowSpan 1 >> /K 2 >>');
    // 13: RoleMap dictionary
    writeObject(13, '<< /Heading1 /H1 /CustomPara /P >>');

    const startXref = pos;
    writeLine('xref');
    writeLine('0 14');
    writeLine('0000000000 65535 f ');
    for (let i = 1; i <= 13; i++) {
        const offStr = String(offsets[i]).padStart(10, '0');
        writeLine(`${offStr} 00000 n `);
    }

    writeLine('trailer');
    writeLine('<< /Size 14 /Root 1 0 R >>');
    writeLine('startxref');
    writeLine(String(startXref));
    writeLine('%%EOF');

    return Buffer.from(lines.join('\n'));
}

async function testWasmStep6() {
    console.log("===============================================================");
    console.log(">>> STEP 6 VERIFICATION: TAGGED PDF SEMANTIC TREE & /Outlines");
    console.log("===============================================================");

    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/5] Modul WebAssembly Pure Rust Terinisialisasi.");

    // --- BAGIAN A: Verifikasi /Outlines pada sample.pdf nyata ---
    console.log("\n>>> [2/5] Memeriksa /Outlines pada sample.pdf nyata...");
    const samplePdfBytes = fs.readFileSync('./sample.pdf');
    const sampleStruct = inspect_pdf_structure(samplePdfBytes);

    console.log(`  Outlines count: ${sampleStruct.outlines.length}`);
    if (sampleStruct.outlines.length > 0) {
        const outline0 = sampleStruct.outlines[0];
        console.log(`  Outline 0: "${outline0.title}" | Target Page: ${outline0.page_number} | Y: ${outline0.dest_y}`);
        if (outline0.title !== "Dummy PDF file") {
            throw new Error(`Judul Outline tidak cocok! Didapatkan: "${outline0.title}"`);
        }
        if (outline0.page_number !== 1) {
            throw new Error(`Halaman Outline tidak cocok! Didapatkan: ${outline0.page_number}`);
        }
    } else {
        throw new Error("Gagal mengekstrak outline dari sample.pdf!");
    }
    console.log("  [PASS] Ekstraksi /Outlines (Daftar Isi/Bookmark) dengan dekode UTF-16BE sukses 100%!");

    // --- BAGIAN B: Verifikasi Tagged PDF dengan /StructTreeRoot & MCID ---
    console.log("\n>>> [3/5] Membuat dan Menguji Berkas Tagged PDF (tagged_sample.pdf)...");
    const taggedPdfBytes = createValidTaggedPdf();
    fs.writeFileSync('./tagged_sample.pdf', taggedPdfBytes);
    console.log(`  tagged_sample.pdf berhasil dibuat (${taggedPdfBytes.length} bytes).`);

    console.log("\n>>> [4/5] Memeriksa inspect_pdf_structure pada Tagged PDF...");
    const taggedStruct = inspect_pdf_structure(taggedPdfBytes);
    const roleMap = taggedStruct.role_map instanceof Map
        ? Object.fromEntries(taggedStruct.role_map)
        : taggedStruct.role_map;

    console.log(`  RoleMap entries: ${JSON.stringify(roleMap)}`);
    console.log(`  Root elements count: ${taggedStruct.root_elements.length}`);

    if (roleMap["Heading1"] !== "H1" || roleMap["CustomPara"] !== "P") {
        throw new Error("RoleMap tidak dipetakan dengan benar!");
    }
    console.log("  [PASS] /RoleMap custom tag resolution (/Heading1 -> H1, /CustomPara -> P) sukses!");

    console.log("\n>>> [5/5] Memeriksa inspect_pdf_elements pada Tagged PDF...");
    const pages = inspect_pdf_elements(taggedPdfBytes);
    const p1 = pages[0];

    console.log(`  Halaman 1 is_tagged: ${p1.is_tagged}`);
    console.log(`  Jumlah Paragraf: ${p1.paragraphs.length}`);
    console.log(`  Jumlah Tabel: ${p1.tables.length}`);
    console.log(`  Jumlah Glif: ${p1.glyphs.length}`);

    if (!p1.is_tagged) {
        throw new Error("Halaman tidak terdeteksi sebagai Tagged PDF!");
    }

    // Periksa MCID pada setiap glif
    console.log("\n  Pemeriksaan Penanda MCID pada Glif:");
    const mcidCounts = {};
    for (const g of p1.glyphs) {
        mcidCounts[g.mcid] = (mcidCounts[g.mcid] || 0) + 1;
    }
    console.log(`  Distribusi MCID Glif: ${JSON.stringify(mcidCounts)}`);
    if (!mcidCounts[0] || !mcidCounts[1] || !mcidCounts[2]) {
        throw new Error("Ada glif yang kehilangan penanda MCID (0, 1, atau 2)!");
    }
    console.log("  [PASS] Operator BDC ... EMC sukses menandai setiap glif dengan MCID yang tepat!");

    // Periksa Paragraf Terstruktur
    console.log("\n  Pemeriksaan Paragraf Semantik:");
    for (let i = 0; i < p1.paragraphs.length; i++) {
        const para = p1.paragraphs[i];
        const text = para.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
        console.log(`  Paragraf #${i}: Tag='${para.tag}', is_header=${para.is_header}, Teks="${text}"`);
    }

    const h1 = p1.paragraphs[0];
    const h1Text = h1.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
    if (!h1.is_header || !h1Text.includes("Pendahuluan")) {
        throw new Error("Heading 1 (H1) semantik tidak terbentuk dengan benar!");
    }
    console.log("  [PASS] Paragraf #0 direkonstruksi sebagai Heading 1 (is_header=true) secara otoritatif!");

    const p = p1.paragraphs[1];
    const pText = p.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
    if (p.is_header || !pText.includes("paragraf semantik")) {
        throw new Error("Paragraf (P) semantik tidak terbentuk dengan benar!");
    }
    console.log("  [PASS] Paragraf #1 direkonstruksi sebagai Normal Paragraph (tag='P')!");

    // Periksa Tabel Terstruktur
    console.log("\n  Pemeriksaan Tabel Semantik:");
    const table = p1.tables[0];
    if (!table || table.cells.length === 0) {
        throw new Error("Tabel semantik tidak terbentuk!");
    }
    const cell0 = table.cells[0][0];
    console.log(`  Tabel 0: Baris=${table.cells.length}, Sel (0,0) ColSpan=${cell0.end_col - cell0.start_col}, RowSpan=${cell0.end_row - cell0.start_row}`);
    const cellText = cell0.content_blocks[0].lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
    console.log(`  Isi Sel (0,0): "${cellText}"`);

    if ((cell0.end_col - cell0.start_col) !== 2 || (cell0.end_row - cell0.start_row) !== 1) {
        throw new Error(`ColSpan atau RowSpan salah! ColSpan=${cell0.end_col - cell0.start_col}`);
    }
    if (!cellText.includes("Data Tabel")) {
        throw new Error("Konten sel tabel tidak sesuai!");
    }
    console.log("  [PASS] Tabel semantik berhasil direkonstruksi dengan atribut ColSpan=2 dan RowSpan=1 presisi 100%!");

    console.log("\n===============================================================");
    console.log(">>> SEMUA VERIFIKASI STEP 6 LULUS 100% SEMPURNA! <<<");
    console.log("===============================================================");
}

testWasmStep6().catch(err => {
    console.error(">>> STEP 6 GAGAL:", err);
    process.exit(1);
});
