import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('laporan.pdf');
    const pages = inspect_pdf_elements(pdfBytes);

    for (let pIdx = 0; pIdx < pages.length; pIdx++) {
        const p = pages[pIdx];
        console.log(`\n================ PAGE ${pIdx + 1} (w=${p.width_points}, h=${p.height_points}) ================`);
        console.log(`Paras: ${p.paragraphs.length}, Tables: ${p.tables.length}`);
        p.paragraphs.forEach((para, i) => {
            const txt = para.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            console.log(`  P${i} [y=${para.bbox.y_min.toFixed(1)}..${para.bbox.y_max.toFixed(1)}] (lines=${para.lines.length}, tag=${para.tag}, align=${para.alignment}): ${txt.slice(0, 90)}`);
        });
    }
}
run();
