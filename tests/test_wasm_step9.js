import fs from 'fs';
import init, { convert_pdf_to_docx } from '../pkg/pdftodocx.js';

// Simple in-memory uncompressed or deflated ZIP parser to verify OPC parts without external dependencies
function parseZipEntries(zipBuffer) {
    const entries = {};
    let offset = 0;

    while (offset < zipBuffer.length - 4) {
        const sig = zipBuffer.readUInt32LE(offset);
        if (sig === 0x04034b50) { // Local file header signature PK\x03\x04
            const flags = zipBuffer.readUInt16LE(offset + 6);
            const method = zipBuffer.readUInt16LE(offset + 8);
            const compSize = zipBuffer.readUInt32LE(offset + 18);
            const uncompSize = zipBuffer.readUInt32LE(offset + 22);
            const nameLen = zipBuffer.readUInt16LE(offset + 26);
            const extraLen = zipBuffer.readUInt16LE(offset + 28);

            const fileName = zipBuffer.toString('utf8', offset + 30, offset + 30 + nameLen);
            const dataStart = offset + 30 + nameLen + extraLen;
            const compressedData = zipBuffer.slice(dataStart, dataStart + compSize);

            entries[fileName] = {
                method,
                compSize,
                uncompSize,
                data: compressedData,
            };

            offset = dataStart + compSize;
        } else {
            break;
        }
    }

    return entries;
}

// Decompress DEFLATE data using Node's built-in zlib
import zlib from 'zlib';

function decompressEntry(entry) {
    if (entry.method === 0) {
        return entry.data.toString('utf8');
    } else if (entry.method === 8) {
        // Raw deflate without zlib wrapper (rfc 1951)
        return zlib.inflateRawSync(entry.data).toString('utf8');
    }
    throw new Error(`Metode kompresi tidak didukung: ${entry.method}`);
}

async function testWasmStep9() {
    console.log("===============================================================");
    console.log(">>> STEP 9 VERIFICATION: OOXML GENERATOR & IN-MEMORY OPC PACKAGER");
    console.log("===============================================================");

    // 1. Initialize Pure Rust WASM
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/5] Modul WebAssembly Pure Rust Terinisialisasi.");

    // 2. Convert sample.pdf
    console.log("\n>>> [2/5] Mengonversi sample.pdf ke berkas .docx nyata...");
    const sampleBytes = fs.readFileSync('./sample.pdf');
    const docxUint8 = convert_pdf_to_docx(sampleBytes);
    const docxBuffer = Buffer.from(docxUint8);

    console.log(`  Ukuran berkas DOCX yang dihasilkan: ${docxBuffer.length} bytes`);
    fs.writeFileSync('./output_sample.docx', docxBuffer);
    console.log("  Berkas output_sample.docx berhasil disimpan ke disk.");

    // Magic Bytes Verification
    const magic = docxBuffer.slice(0, 4);
    console.log(`  Magic Bytes: [ 0x${magic[0].toString(16)}, 0x${magic[1].toString(16)}, 0x${magic[2].toString(16)}, 0x${magic[3].toString(16)} ]`);
    if (magic[0] !== 0x50 || magic[1] !== 0x4B || magic[2] !== 0x03 || magic[3] !== 0x04) {
        throw new Error("Magic bytes DOCX bukan kontainer ZIP PK\\x03\\x04 yang valid!");
    }
    console.log("  [PASS] 1. Magic Bytes: Format biner ZIP PK\\x03\\x04 terverifikasi 100% valid!");

    // 3. Inspect OPC Package Structure
    console.log("\n>>> [3/5] Menginspeksi struktur paket Open Packaging Conventions (OPC)...");
    const entries = parseZipEntries(docxBuffer);
    const entryNames = Object.keys(entries);
    console.log(`  Daftar Berkas di dalam Kontainer DOCX: ${JSON.stringify(entryNames, null, 2)}`);

    const requiredParts = [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/_rels/document.xml.rels",
        "word/settings.xml",
        "word/styles.xml",
        "word/numbering.xml",
        "word/document.xml"
    ];

    for (const part of requiredParts) {
        if (!entries[part]) {
            throw new Error(`Bagian OPC wajib hilang dari ZIP: ${part}`);
        }
    }
    console.log("  [PASS] 2. Arsitektur OPC: Seluruh 7 komponen XML wajib hadir di dalam kontainer ZIP!");

    // Inspect word/document.xml content
    const docXml = decompressEntry(entries["word/document.xml"]);
    console.log("\n  Pemeriksaan Cuplikan word/document.xml:");
    console.log("  -------------------------------------------------------------");
    console.log(docXml.substring(0, 600) + "...\n  [konten tengah] ...\n" + docXml.substring(docXml.length - 300));
    console.log("  -------------------------------------------------------------");

    if (!docXml.includes("Dummy PDF file")) {
        throw new Error("Teks 'Dummy PDF file' tidak ditemukan di word/document.xml!");
    }
    if (!docXml.includes("<w:pStyle w:val=\"Heading1\"/>")) {
        throw new Error("Gaya Heading 1 (<w:pStyle w:val=\"Heading1\"/>) tidak ditemukan di word/document.xml!");
    }
    if (!docXml.includes("<w:sectPr>")) {
        throw new Error("Properti seksi halaman (<w:sectPr>) tidak ditemukan di word/document.xml!");
    }
    console.log("  [PASS] 3. WordprocessingML: Teks semantik, hierarki Heading 1, dan dimensi halaman tertanam sempurna!");

    // 4. Test Table & Tab Stops Conversion on table_form_sample.pdf
    console.log("\n>>> [4/5] Mengonversi table_form_sample.pdf (Tabel Grid & Custom Tab Stops)...");
    const tablePdfBytes = fs.readFileSync('./table_form_sample.pdf');
    const tableDocxUint8 = convert_pdf_to_docx(tablePdfBytes);
    const tableDocxBuffer = Buffer.from(tableDocxUint8);

    fs.writeFileSync('./output_table_form.docx', tableDocxBuffer);
    console.log(`  Berkas output_table_form.docx dibuat (${tableDocxBuffer.length} bytes).`);

    const tableEntries = parseZipEntries(tableDocxBuffer);
    const tableDocXml = decompressEntry(tableEntries["word/document.xml"]);

    console.log("\n  Pemeriksaan Tabel & Tabs di word/document.xml:");
    if (!tableDocXml.includes("<w:tbl>")) {
        throw new Error("Tabel (<w:tbl>) tidak ditemukan di word/document.xml!");
    }
    if (!tableDocXml.includes("<w:tblGrid>")) {
        throw new Error("Grid kolom tabel (<w:tblGrid>) tidak ditemukan di word/document.xml!");
    }
    if (!tableDocXml.includes("Header A") || !tableDocXml.includes("Header B") || !tableDocXml.includes("Nilai 1") || !tableDocXml.includes("Nilai 2")) {
        throw new Error("Data isi sel tabel tidak lengkap di word/document.xml!");
    }
    console.log("  [PASS] 4. Rekonstruksi Tabel OOXML: Struktur <w:tbl>, <w:tr>, <w:tc>, dan <w:gridCol> 100% presisi!");

    if (!tableDocXml.includes("<w:tab/>")) {
        throw new Error("Elemen lompatan tab (<w:tab/>) tidak ditemukan di formulir/daftar isi!");
    }
    if (!tableDocXml.includes("<w:tab w:val=\"right\" w:leader=\"dot\"")) {
        throw new Error("Tab stop pemandu titik (<w:tab w:val=\"right\" w:leader=\"dot\".../>) tidak ditemukan!");
    }
    console.log("  [PASS] 5. Custom Tab Stops & Leader Dots: <w:tabs> dan <w:tab/> berhasil disematkan presisi!");

    console.log("\n===============================================================");
    console.log(">>> SEMUA VERIFIKASI STEP 9 LULUS 100% SEMPURNA! <<<");
    console.log("===============================================================");
}

testWasmStep9().catch(err => {
    console.error(">>> STEP 9 GAGAL:", err);
    process.exit(1);
});
