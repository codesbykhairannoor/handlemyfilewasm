import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);

for (let pIdx = 0; pIdx < pages.length; pIdx++) {
    const page = pages[pIdx];
    if (page.tables.length > 0) {
        console.log(`\n=== Page ${pIdx + 1} Tables: ${page.tables.length} ===`);
        for (let tIdx = 0; tIdx < page.tables.length; tIdx++) {
            const t = page.tables[tIdx];
            console.log(`Table ${tIdx}: rows=${t.rows}, cols=${t.cols}, bbox=[${t.bbox.x_min.toFixed(1)}, ${t.bbox.y_min.toFixed(1)}, ${t.bbox.x_max.toFixed(1)}, ${t.bbox.y_max.toFixed(1)}]`);
            for (let r = 0; r < t.cells.length; r++) {
                const row = t.cells[r];
                const rowText = row.map(c => c.content_blocks.map(b => b.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ')).join(' ')).join(' | ');
                console.log(`  Row ${r}: ${rowText}`);
            }
        }
    }
}

