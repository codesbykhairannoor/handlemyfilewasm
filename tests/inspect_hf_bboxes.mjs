import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('tests/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c - Copy.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    pages.forEach((p, i) => {
        console.log(`Page ${i + 1}:`);
        if (p.header) {
            console.log(`  Header bbox: y=[${p.header.bbox.y_min.toFixed(1)}..${p.header.bbox.y_max.toFixed(1)}], text: ${p.header.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ')}`);
        } else {
            console.log(`  Header: NONE`);
        }
        if (p.footer) {
            console.log(`  Footer bbox: y=[${p.footer.bbox.y_min.toFixed(1)}..${p.footer.bbox.y_max.toFixed(1)}], text: ${p.footer.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ')}`);
        } else {
            console.log(`  Footer: NONE`);
        }
        const lastPara = p.paragraphs[p.paragraphs.length - 1];
        if (lastPara) {
            console.log(`  Last Para bbox: y=[${lastPara.bbox.y_min.toFixed(1)}..${lastPara.bbox.y_max.toFixed(1)}]`);
        }
    });
}
run();

