import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('tests/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c - Copy.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    pages.forEach((p, i) => {
        let maxY = 0;
        p.paragraphs.forEach(para => {
            if (para.bbox.y_max > maxY) maxY = para.bbox.y_max;
        });
        p.tables.forEach(tbl => {
            if (tbl.bbox.y_max > maxY) maxY = tbl.bbox.y_max;
        });
        console.log(`Page ${i + 1}: maxY = ${maxY.toFixed(1)} / ${p.height_points.toFixed(1)} (${(maxY / p.height_points * 100).toFixed(1)}%)`);
    });
}
run();

