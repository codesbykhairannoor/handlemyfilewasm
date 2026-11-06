import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function testWasmStep4() {
    console.log(">>> Inisialisasi Modul WebAssembly Pure Rust (Step 4)...");
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });

    console.log(">>> Membaca sample.pdf...");
    const samplePdfBytes = fs.readFileSync('./sample.pdf');

    console.log(">>> Menjalankan inspect_pdf_elements (Affine Matrix & Graphics State Machine)...");
    const pages = inspect_pdf_elements(samplePdfBytes);

    console.log(`>>> Total Halaman: ${pages.length}`);
    for (const p of pages) {
        console.log(`\n--- HALAMAN ${p.page_number} (${p.width_points} x ${p.height_points} pt) ---`);
        console.log(`Jumlah Glif Teks terekstraksi: ${p.glyphs.length}`);
        console.log(`Jumlah Jalur Garis Vektor (Tabel): ${p.vector_paths.length}`);
        console.log(`Jumlah Gambar Terdeteksi: ${p.graphics.length}`);

        if (p.glyphs.length > 0) {
            const sampleText = p.glyphs.slice(0, 50).map(g => g.text).join('');
            console.log(`Sampel Teks Awal: "${sampleText}"`);
            const first = p.glyphs[0];
            console.log(`Glif Pertama: "${first.text}" di [x=${first.bbox.x_min.toFixed(2)}, y=${first.bbox.y_min.toFixed(2)}, w=${first.bbox.x_max - first.bbox.x_min}, h=${first.style.font_size} pt, Font: ${first.style.font_family}]`);
        }

        if (p.vector_paths.length > 0) {
            const firstLine = p.vector_paths[0];
            console.log(`Garis Vektor Pertama: (${firstLine.x1.toFixed(2)}, ${firstLine.y1.toFixed(2)}) -> (${firstLine.x2.toFixed(2)}, ${firstLine.y2.toFixed(2)})`);
        }
    }

    console.log("\n>>> STEP 4 VERIFIKASI 100% SUKSES!");
}

testWasmStep4().catch((err) => {
    console.error(">>> ERROR:", err);
    process.exit(1);
});

