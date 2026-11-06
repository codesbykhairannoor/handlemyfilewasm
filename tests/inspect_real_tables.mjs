import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('real_sample.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    const p = pages[1]; // Page 2
    console.log(`Page 2 tables count: ${p.tables.length}`);
    p.tables.forEach((t, i) => {
        console.log(`Table ${i}: bbox=[${t.bbox.x_min.toFixed(1)}, ${t.bbox.y_min.toFixed(1)} - ${t.bbox.x_max.toFixed(1)}, ${t.bbox.y_max.toFixed(1)}]`);
        console.log(`  cols: ${t.grid_cols.length}, rows: ${t.grid_rows.length}, cells rows: ${t.cells.length}`);
        t.cells.forEach((row, ri) => {
            row.forEach((c, ci) => {
                const txt = c.content_blocks.map(b => b.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ')).join(' | ');
                console.log(`    Cell [${ri},${ci}]: "${txt.slice(0, 60)}"`);
            });
        });
    });
}
run();

