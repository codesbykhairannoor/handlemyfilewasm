import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function testWasmStep5() {
    console.log("===============================================================");
    console.log(">>> STEP 5 VERIFICATION: FONT DECODER, METRICS, & /ToUnicode CMAP");
    console.log("===============================================================");

    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    console.log(">>> [1/4] Modul WebAssembly Pure Rust Terinisialisasi.");

    const samplePdfBytes = fs.readFileSync('./sample.pdf');
    console.log(`>>> [2/4] sample.pdf Dibaca (${samplePdfBytes.length} bytes).`);

    console.log(">>> [3/4] Menjalankan inspect_pdf_elements dengan CMap & FontDecoder...");
    const pages = inspect_pdf_elements(samplePdfBytes);

    if (!pages || pages.length === 0) {
        throw new Error("Tidak ada halaman yang berhasil diekstrak!");
    }

    const page1 = pages[0];
    console.log(`>>> Halaman 1 Ukuran: ${page1.width_points} x ${page1.height_points} pt`);
    console.log(`>>> Total Glif: ${page1.glyphs.length}`);

    // Detail setiap glif
    console.log("\n--- DETAIL SETIAP GLIF YANG DIDEKODE DARI /ToUnicode ---");
    let fullExtractedText = "";
    for (let i = 0; i < page1.glyphs.length; i++) {
        const g = page1.glyphs[i];
        fullExtractedText += g.text;
        const w = (g.bbox.x_max - g.bbox.x_min).toFixed(2);
        const h = (g.bbox.y_max - g.bbox.y_min).toFixed(2);
        console.log(
            `  Glif #${String(i).padStart(2, '0')}: '${g.text}' ` +
            `| Box: [x=${g.bbox.x_min.toFixed(2)}, y=${g.bbox.y_min.toFixed(2)}, w=${w}, h=${h}] ` +
            `| Font: ${g.style.font_family} (Bold: ${g.style.is_bold}, Italic: ${g.style.is_italic}, Size: ${g.style.font_size} pt, Color: #${g.style.hex_color})`
        );
    }

    console.log("\n===============================================================");
    console.log(`>>> HASIL TEKS LENGKAP YANG DIEKSTRAK: "${fullExtractedText}"`);
    console.log("===============================================================");

    // Verifikasi Ketat
    console.log("\n>>> [4/4] Memvalidasi Syarat Kualitas Step 5:");

    // 1. Teks harus sama persis dengan "Dummy PDF file"
    if (fullExtractedText !== "Dummy PDF file") {
        throw new Error(`Teks tidak cocok! Diharapkan "Dummy PDF file", didapatkan "${fullExtractedText}"`);
    }
    console.log("  [PASS] 1. Anti Alien Text: Teks berhasil diterjemahkan 100% tepat menjadi \"Dummy PDF file\" via /ToUnicode!");

    // 2. Nama font harus bersih (bukan BAAAAA+Arial-BoldMT)
    const firstGlyph = page1.glyphs[0];
    if (firstGlyph.style.font_family !== "Arial") {
        throw new Error(`Nama font tidak dibersihkan dengan benar: ${firstGlyph.style.font_family}`);
    }
    console.log("  [PASS] 2. Sanitasi Font: Prefix subset acak dihilangkan (\"BAAAAA+Arial-BoldMT\" -> \"Arial\")");

    // 3. Status gaya tebal (is_bold) harus terdeteksi
    if (!firstGlyph.style.is_bold) {
        throw new Error("Gaya tebal (is_bold) tidak terdeteksi!");
    }
    console.log("  [PASS] 3. Deteksi Tipografi: Atribut is_bold = true berhasil dikenali");

    // 4. Koordinat X harus bertambah secara monoton
    for (let i = 1; i < page1.glyphs.length; i++) {
        const prev = page1.glyphs[i - 1];
        const curr = page1.glyphs[i];
        if (curr.bbox.x_min < prev.bbox.x_min) {
            throw new Error(`Koordinat horizontal regresi pada glif #${i} ('${curr.text}')! prev.x=${prev.bbox.x_min}, curr.x=${curr.bbox.x_min}`);
        }
    }
    console.log("  [PASS] 4. Monotonisitas Horisontal: Seluruh glif memiliki urutan spasial X positif tanpa tumpang tindih");

    console.log("\n>>> SEMUA VERIFIKASI STEP 5 LULUS 100% SEMPURNA! <<<");
}

testWasmStep5().catch(err => {
    console.error(">>> STEP 5 GAGAL:", err);
    process.exit(1);
});

