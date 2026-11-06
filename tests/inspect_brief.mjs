import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfPath = 'tests/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c - Copy.pdf';
    const pdfBytes = fs.readFileSync(pdfPath);
    const pages = inspect_pdf_elements(pdfBytes);

    for (let pIdx = 0; pIdx < pages.length; pIdx++) {
        const p = pages[pIdx];
        console.log(`\n=== PAGE ${pIdx + 1} (w=${p.width_points.toFixed(1)}, h=${p.height_points.toFixed(1)}) ===`);
        console.log('Header:', p.header ? JSON.stringify(p.header.lines.map(l => l.words.map(w => w.text).join(' '))) : 'NONE');
        console.log('Footer:', p.footer ? JSON.stringify(p.footer.lines.map(l => l.words.map(w => w.text).join(' '))) : 'NONE');
        console.log(`Paras count: ${p.paragraphs.length}, Tables: ${p.tables.length}`);
        p.paragraphs.forEach((para, i) => {
            const txt = para.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            if (i < 3 || i >= p.paragraphs.length - 3) {
                console.log(`  P${i} [y=${para.bbox.y_min.toFixed(1)}..${para.bbox.y_max.toFixed(1)}]: ${txt.slice(0, 80)}`);
            }
        });
    }
}
run();

