import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);

for (let pIdx = 0; pIdx < pages.length; pIdx++) {
    const page = pages[pIdx];
    console.log(`\nPage ${pIdx + 1}: vector paths = ${page.vector_paths.length}, tables = ${page.tables.length}`);
    if (page.vector_paths.length > 0) {
        for (let i = 0; i < Math.min(10, page.vector_paths.length); i++) {
            const vp = page.vector_paths[i];
            console.log(`  Path ${i}: (${vp.x1.toFixed(1)}, ${vp.y1.toFixed(1)}) -> (${vp.x2.toFixed(1)}, ${vp.y2.toFixed(1)})`);
        }
    }
}
