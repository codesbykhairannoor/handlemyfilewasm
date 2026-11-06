import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function check() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('laporan.pdf');
    const pages = inspect_pdf_elements(pdfBytes);

    for (let i = 0; i < pages.length - 1; i++) {
        const curP = pages[i].paragraphs;
        const nextP = pages[i + 1].paragraphs;
        if (curP.length > 0 && nextP.length > 0) {
            const last = curP[curP.length - 1];
            const first = nextP[0];
            const lastText = last.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            const firstText = first.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            const words = lastText.trim().split(/\s+/);
            const lastWord = words[words.length - 1] || '';
            const firstWord = firstText.trim().split(/\s+/)[0] || '';
            const endsWithPeriod = lastText.trim().endsWith('.') || lastText.trim().endsWith(':');
            const startsWithLower = /^[a-z]/.test(firstWord);

            console.log(`Boundary ${i} -> ${i + 1}:`);
            console.log(`   Page ${i} ends at y=${last.bbox.y_max.toFixed(1)} with: "...${lastWord}" (period: ${endsWithPeriod})`);
            console.log(`   Page ${i + 1} starts at y=${first.bbox.y_min.toFixed(1)} with: "${firstWord}..." (lower: ${startsWithLower})`);
            if (!endsWithPeriod && startsWithLower) {
                console.log(`   >>> CONTINUATION DETECTED! (Page ${i} -> Page ${i + 1})`);
            }
        }
    }
}

check();

