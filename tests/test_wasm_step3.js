import fs from 'fs';
import init, { convert_pdf_to_docx } from '../pkg/pdftodocx.js';

async function testWasmStep3() {
    console.log(">>> Inisialisasi Modul WebAssembly Pure Rust...");
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });

    console.log(">>> Membaca sample.pdf...");
    const samplePdfBytes = fs.readFileSync('./sample.pdf');
    console.log(`>>> Ukuran PDF: ${samplePdfBytes.length} bytes`);

    console.log(">>> Menjalankan convert_pdf_to_docx via WebAssembly...");
    const resultDocx = convert_pdf_to_docx(samplePdfBytes);
    console.log(`>>> HASIL WASM: Berhasil diproses! Return bytes: ${resultDocx.length}`);
    console.log(">>> STEP 3 VERIFIKASI SELESAI!");
}

testWasmStep3().catch((err) => {
    console.error(">>> ERROR:", err);
    process.exit(1);
});

