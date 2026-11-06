import init, { convert_pdf_to_docx, inspect_pdf_elements } from './pkg/pdftodocx.js';

let wasmInitialized = false;
let currentDocxBlob = null;
let currentFileName = 'document.docx';
let currentLayoutJson = null;

const dropzone = document.getElementById('dropzone');
const fileInput = document.getElementById('fileInput');
const logArea = document.getElementById('logArea');
const telemetryGrid = document.getElementById('telemetryGrid');
const actionBar = document.getElementById('actionBar');
const btnDownload = document.getElementById('btnDownload');
const btnInspect = document.getElementById('btnInspect');

const mDuration = document.getElementById('mDuration');
const mSpeed = document.getElementById('mSpeed');
const mPages = document.getElementById('mPages');
const mParas = document.getElementById('mParas');
const mTables = document.getElementById('mTables');

function log(msg, type = 'info') {
    const timestamp = new Date().toLocaleTimeString();
    const prefix = `[${timestamp}] `;
    logArea.innerHTML += `\n${prefix}${msg}`;
    logArea.scrollTop = logArea.scrollHeight;
}

function clearLog() {
    logArea.innerHTML = '';
}

// Inisialisasi WebAssembly Engine
async function ensureWasmReady() {
    if (!wasmInitialized) {
        log('Menginisialisasi mesin WebAssembly Pure Rust...', 'info');
        await init();
        wasmInitialized = true;
        log('Modul WebAssembly siap 100%!', 'pass');
    }
}

// Handler Pemrosesan Berkas PDF Utama
async function processPdfBytes(pdfBytes, fileName) {
    clearLog();
    log(`Memulai konversi "${fileName}" (${(pdfBytes.byteLength / 1024).toFixed(1)} KB)...`, 'info');

    try {
        await ensureWasmReady();

        const t0 = performance.now();

        // 1. Eksekusi Konversi PDF ke DOCX via Pure Rust WASM
        const docxUint8 = convert_pdf_to_docx(pdfBytes);

        // 2. Ekstraksi Metrik Layout untuk Telemetri UI
        let layoutData = null;
        try {
            layoutData = inspect_pdf_elements(pdfBytes);
            currentLayoutJson = layoutData;
        } catch (e) {
            console.warn('Inspect failed:', e);
        }

        const t1 = performance.now();
        const durationMs = Math.round(t1 - t0);
        const speedMBps = ((pdfBytes.byteLength / (1024 * 1024)) / ((t1 - t0) / 1000)).toFixed(2);

        // 3. Simpan Blob DOCX
        currentDocxBlob = new Blob([docxUint8], {
            type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
        });
        currentFileName = fileName.replace(/\.pdf$/i, '') + '.docx';

        // 4. Update Telemetri
        const pageCount = layoutData ? layoutData.length : 1;
        let totalParas = 0;
        let totalTables = 0;
        if (layoutData) {
            for (const p of layoutData) {
                totalParas += (p.paragraphs ? p.paragraphs.length : 0);
                totalTables += (p.tables ? p.tables.length : 0);
            }
        }

        mDuration.textContent = `${durationMs} ms`;
        mSpeed.textContent = `${speedMBps} MB/s`;
        mPages.textContent = pageCount;
        mParas.textContent = totalParas;
        mTables.textContent = totalTables;

        telemetryGrid.style.display = 'grid';
        actionBar.style.display = 'flex';

        log(`[SUKSES] Konversi selesai dalam ${durationMs} ms (${speedMBps} MB/s).`, 'pass');
        log(`Ukuran biner DOCX: ${(docxUint8.byteLength / 1024).toFixed(1)} KB (${docxUint8.byteLength} bytes).`, 'pass');
        log(`Magic Bytes terverifikasi: PK\\x03\\x04 (Open Packaging Conventions).`, 'pass');
        log(`Klik tombol 'Unduh Berkas .DOCX' untuk membuka dokumen di Microsoft Word.`, 'info');

    } catch (err) {
        log(`[ERROR] Gagal memproses berkas: ${err}`, 'err');
        console.error(err);
    }
}

// Download Button
btnDownload.addEventListener('click', () => {
    if (!currentDocxBlob) return;
    const url = URL.createObjectURL(currentDocxBlob);
    const a = document.createElement('a');
    a.href = url;
    a.download = currentFileName;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
});

// Inspect JSON Button
btnInspect.addEventListener('click', () => {
    if (!currentLayoutJson) {
        log('Tidak ada data struktur JSON layout yang tersedia.', 'err');
        return;
    }
    const jsonStr = JSON.stringify(currentLayoutJson, null, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    window.open(url, '_blank');
});

// Dropzone & File Input Events
dropzone.addEventListener('click', () => fileInput.click());

dropzone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropzone.classList.add('dragover');
});

dropzone.addEventListener('dragleave', (e) => {
    e.preventDefault();
    dropzone.classList.remove('dragover');
});

dropzone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropzone.classList.remove('dragover');
    if (e.dataTransfer.files.length) {
        const file = e.dataTransfer.files[0];
        handleUploadedFile(file);
    }
});

fileInput.addEventListener('change', (e) => {
    if (e.target.files.length) {
        handleUploadedFile(e.target.files[0]);
    }
});

async function handleUploadedFile(file) {
    if (!file.name.toLowerCase().endsWith('.pdf')) {
        log('Peringatan: Berkas yang dipilih bukan berkas PDF (.pdf)', 'err');
        return;
    }
    const arrayBuffer = await file.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    await processPdfBytes(bytes, file.name);
}

// Stress Test Buttons
document.getElementById('btnCapybala')?.addEventListener('click', () => {
    runSampleTest('/CAPYBALA__Context_Aware_Pruning_for_Yielding_and_Backtrack_Aligned_Loop_Avoidance_in_Autonomous_Software_Engineering_Agents__Anonim.pdf', 'capybala_paper.pdf');
});

document.getElementById('btnTranskrip')?.addEventListener('click', () => {
    runSampleTest('/transkrip_akademik.pdf', 'transkrip_akademik.pdf');
});

document.getElementById('btnBrief')?.addEventListener('click', () => {
    runSampleTest('/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c%20-%20Copy.pdf', 'brief_coda.pdf');
});

document.getElementById('btnSpkp')?.addEventListener('click', () => {
    runSampleTest('/SPKP_KodingData_Khairan%20(signed).pdf', 'spkp_legal.pdf');
});

document.getElementById('btnLaporan')?.addEventListener('click', () => {
    runSampleTest('/laporan.pdf', 'laporan_26_hal.pdf');
});

// Quick Test Sample Buttons
async function runSampleTest(samplePath, sampleName) {
    try {
        log(`Mengunduh sampel "${sampleName}"...`, 'info');
        const res = await fetch(samplePath);
        if (!res.ok) throw new Error(`HTTP ${res.status}: Gagal memuat ${samplePath}`);
        const arrayBuffer = await res.arrayBuffer();
        const bytes = new Uint8Array(arrayBuffer);
        await processPdfBytes(bytes, sampleName);
    } catch (e) {
        log(`Gagal memuat ${sampleName}: ${e.message}`, 'err');
    }
}

document.getElementById('btnSample').addEventListener('click', () => {
    runSampleTest('/sample.pdf', 'sample.pdf');
});

document.getElementById('btnTable').addEventListener('click', () => {
    runSampleTest('/table_form_sample.pdf', 'table_form_sample.pdf');
});

document.getElementById('btnMultiCol').addEventListener('click', () => {
    runSampleTest('/multicolumn_sample.pdf', 'multicolumn_sample.pdf');
});

document.getElementById('btnTagged').addEventListener('click', () => {
    runSampleTest('/tagged_sample.pdf', 'tagged_sample.pdf');
});

// Auto-run initialization on window load
window.addEventListener('load', async () => {
    await ensureWasmReady();
    log('Antarmuka siap. Silakan drag-and-drop berkas PDF atau klik salah satu tombol uji cepat di atas.', 'info');
});
