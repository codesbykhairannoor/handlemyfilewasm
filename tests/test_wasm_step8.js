d: \Downloads\Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate - 40747646 - 1784 - 45dd - 9c2b - eb180c85c54c.pdfimport fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

function createTableAndTabsPdf() {
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

    // 5: Content stream:
    // (A) Bordered Table: 2x2 grid
    //     Horizontal lines: y = 600, 640, 680 (x: 50..250)
    //     Vertical lines: x = 50, 150, 250 (y: 600..680)
    //     Text inside cells:
    //     - Cell (0,0): (x: 60, y: 660) -> "Header A"
    //     - Cell (0,1): (x: 160, y: 660) -> "Header B"
    //     - Cell (1,0): (x: 60, y: 620) -> "Nilai 1"
    //     - Cell (1,1): (x: 160, y: 620) -> "Nilai 2"
    // (B) Colon Form:
    //     - Line 1: (x=50, y=500) "Nama Pasien" ... (x=200, y=500) ": Ahmad Dahlan"
    //     - Line 2: (x=50, y=475) "Nomor Rekam Medis" ... (x=200, y=475) ": RM-2026-9912"
    // (C) Dot Leader Table of Contents:
    //     - Line 3: (x=50, y=400) "Bab 1 Pendahuluan" ... (x=250, y=400) "....." ... (x=350, y=400) "15"
    const streamData =
        `0.5 w
50 600 m 250 600 l S
50 640 m 250 640 l S
50 680 m 250 680 l S
50 600 m 50 680 l S
150 600 m 150 680 l S
250 600 m 250 680 l S

BT
/F1 10 Tf
60 660 Td (Header A) Tj
100 0 Td (Header B) Tj
-100 -40 Td (Nilai 1) Tj
100 0 Td (Nilai 2) Tj
ET

BT
/F1 10 Tf
50 500 Td (Nama Pasien) Tj
150 0 Td (: Ahmad Dahlan) Tj
-150 -25 Td (Nomor Rekam Medis) Tj
150 0 Td (: RM-2026-9912) Tj
ET

BT
/F1 10 Tf
50 400 Td (Bab 1 Pendahuluan) Tj
150 0 Td (.....) Tj
60 0 Td (15) Tj
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

async function testWasmStep8() {
    console.log("===============================================================");
    console.log(">>> STEP 8 VERIFICATION: TABLE RECOGNITION (TSR) & TAB STOPS");
    console.log("===============================================================");

    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/5] Modul WebAssembly Pure Rust Terinisialisasi.");

    console.log("\n>>> [2/5] Membuat dan Menguji Berkas Tabel & Form (table_form_sample.pdf)...");
    const pdfBytes = createTableAndTabsPdf();
    fs.writeFileSync('./table_form_sample.pdf', pdfBytes);
    console.log(`  table_form_sample.pdf dibuat (${pdfBytes.length} bytes).`);

    console.log("\n>>> [3/5] Memeriksa Table Structure Recognition (TSR)...");
    const pages = inspect_pdf_elements(pdfBytes);
    const p1 = pages[0];

    console.log(`  Jumlah Jalur Vektor: ${p1.vector_paths.length}`);
    console.log(`  Jumlah Tabel Terdeteksi: ${p1.tables.length}`);
    console.log(`  Jumlah Paragraf Body di Luar Tabel: ${p1.paragraphs.length}`);

    if (p1.tables.length !== 1) {
        throw new Error(`Diharapkan 1 tabel terdeteksi, didapatkan: ${p1.tables.length}`);
    }

    const table = p1.tables[0];
    console.log(`  Dimensi Grid Tabel: ${table.cells.length} baris x ${table.cells[0].length} kolom`);
    console.log(`  Lebar Kolom Grid: ${JSON.stringify(table.grid_cols)}`);
    console.log(`  Tinggi Baris Grid: ${JSON.stringify(table.grid_rows)}`);

    if (table.cells.length !== 2 || table.cells[0].length !== 2) {
        throw new Error(`Grid sel tabel tidak sesuai! Didapatkan ${table.cells.length}x${table.cells[0].length}`);
    }

    // Detail isi sel tabel
    console.log("\n  Pemeriksaan Isi Setiap Sel Tabel:");
    for (let r = 0; r < table.cells.length; r++) {
        for (let c = 0; c < table.cells[r].length; c++) {
            const cell = table.cells[r][c];
            const content = cell.content_blocks.map(cb => cb.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ')).join(' | ');
            console.log(`    Sel (${r},${c}): Box=[x=${cell.bbox.x_min}, y=${cell.bbox.y_min}, w=${cell.bbox.x_max - cell.bbox.x_min}, h=${cell.bbox.y_max - cell.bbox.y_min}] -> "${content}"`);
        }
    }

    const c00Text = table.cells[0][0].content_blocks[0].lines[0].words.map(w => w.text).join(' ');
    const c01Text = table.cells[0][1].content_blocks[0].lines[0].words.map(w => w.text).join(' ');
    const c10Text = table.cells[1][0].content_blocks[0].lines[0].words.map(w => w.text).join(' ');
    const c11Text = table.cells[1][1].content_blocks[0].lines[0].words.map(w => w.text).join(' ');

    if (!c00Text.includes("Header A") || !c01Text.includes("Header B") || !c10Text.includes("Nilai 1") || !c11Text.includes("Nilai 2")) {
        throw new Error(`Teks di dalam sel tabel salah! c00="${c00Text}", c01="${c01Text}", c10="${c10Text}", c11="${c11Text}"`);
    }
    console.log("  [PASS] 1. Table Structure Recognition (TSR): Matriks 2x2 direkonstruksi dengan pemetaan teks sel 100% presisi!");

    // --- BAGIAN B: Verifikasi Custom Tab Stops pada Formulir Titik Dua & Dot Leader ---
    console.log("\n>>> [4/5] Memeriksa Custom Tab Stops (<w:tabs>) pada Formulir Titik Dua...");
    const colonPara = p1.paragraphs.find(p => p.lines.some(l => l.tab_stops.length > 0 && !l.tab_stops[0].is_leader_dot));
    if (!colonPara) {
        throw new Error("Gagal mendeteksi paragraf formulir bertitik dua dengan Custom Tab Stop!");
    }

    console.log(`  Paragraf Formulir Titik Dua: ${colonPara.lines.length} baris`);
    for (let i = 0; i < colonPara.lines.length; i++) {
        const l = colonPara.lines[i];
        console.log(`    Baris #${i} TabStops: ${JSON.stringify(l.tab_stops)}`);
        console.log(`    Baris #${i} Runs: ${JSON.stringify(l.runs.map(r => ({ text: r.text, is_tab: r.is_tab })))}`);

        const hasTabRun = l.runs.some(r => r.is_tab);
        if (!hasTabRun) {
            throw new Error(`Baris #${i} tidak memiliki TextRun tab!`);
        }
    }
    console.log("  [PASS] 2. Colon Alignment Tabs: Posisi titik dua ':' otomatis dikonversi menjadi Custom Tab Stop dan TextRun <w:tab/>!");

    console.log("\n>>> [5/5] Memeriksa Dot Leader Tabs pada Daftar Isi...");
    for (let i = 0; i < p1.paragraphs.length; i++) {
        const p = p1.paragraphs[i];
        for (let j = 0; j < p.lines.length; j++) {
            const l = p.lines[j];
            console.log(`    P[${i}].L[${j}] words: ${JSON.stringify(l.words.map(w => w.text))}, tab_stops: ${JSON.stringify(l.tab_stops)}`);
        }
    }

    const dotLeaderPara = p1.paragraphs.find(p => p.lines.some(l => l.tab_stops.some(ts => ts.is_leader_dot)));
    if (!dotLeaderPara) {
        throw new Error("Gagal mendeteksi dot leader tab pada baris daftar isi!");
    }

    const dotLine = dotLeaderPara.lines[0];
    const dotTab = dotLine.tab_stops.find(ts => ts.is_leader_dot);
    console.log(`  Dot Leader Tab: Posisi X=${dotTab.x_pos_points} pt, Align='${dotTab.align_type}', Leader='dot'`);
    console.log(`  Dot Line Runs: ${JSON.stringify(dotLine.runs.map(r => ({ text: r.text, is_tab: r.is_tab, is_leader: r.is_leader_dot })))}`);

    const hasDotTabRun = dotLine.runs.some(r => r.is_tab && r.is_leader_dot);
    if (!hasDotTabRun) {
        throw new Error("Dot leader run tidak terbentuk!");
    }
    console.log("  [PASS] 3. Dot Leader Tabs: Karakter '.....' otomatis diubah menjadi tab stop rata kanan dengan pemandu titik (leader dot)!");

    console.log("\n===============================================================");
    console.log(">>> SEMUA VERIFIKASI STEP 8 LULUS 100% SEMPURNA! <<<");
    console.log("===============================================================");
}

testWasmStep8().catch(err => {
    console.error(">>> STEP 8 GAGAL:", err);
    process.exit(1);
});
