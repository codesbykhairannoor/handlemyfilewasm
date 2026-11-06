import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);
const page1 = pages[0];

for (const vp of page1.vector_paths) {
    if (Math.min(vp.y1, vp.y2) <= 465) {
        console.log(`(${vp.x1.toFixed(1)}, ${vp.y1.toFixed(1)}) -> (${vp.x2.toFixed(1)}, ${vp.y2.toFixed(1)})`);
    }
}

