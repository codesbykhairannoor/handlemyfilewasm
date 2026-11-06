import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('tests/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c - Copy.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    const p1 = pages[0];
    console.log('Page 1 header:', p1.header ? p1.header.lines.map(l => l.words.map(w => w.text).join(' ')).join(' | ') : 'None');
    console.log('Page 1 footer:', p1.footer ? p1.footer.lines.map(l => l.words.map(w => w.text).join(' ')).join(' | ') : 'None');
    console.log('Page 1 paras:');
    p1.paragraphs.forEach((p, i) => {
        const txt = p.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
        console.log('  [' + i + '] y=' + p.bbox.y_min.toFixed(1) + '..' + p.bbox.y_max.toFixed(1) + ': ' + txt);
    });
}
run();

