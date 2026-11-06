import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('tests/Brief__Case_Study_Job_Vacancy_Detail__fullstacksdetcandidate-40747646-1784-45dd-9c2b-eb180c85c54c - Copy.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    pages.forEach((p, i) => {
        if (p.footer) {
            const runCount = p.footer.lines.reduce((acc, l) => acc + (l.runs ? l.runs.length : 0), 0);
            const text = p.footer.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            console.log(`Page ${i + 1} footer: runs=${runCount}, text="${text}"`);
        }
    });
}
run();

