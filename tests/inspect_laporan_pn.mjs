import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

async function run() {
    const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
    await init({ module_or_path: wasmBuffer });
    const pdfBytes = fs.readFileSync('laporan.pdf');
    const pages = inspect_pdf_elements(pdfBytes);
    pages.slice(0, 6).forEach((p, i) => {
        console.log('Page ' + (i + 1) + ' (h=' + p.height_points + '):');
        console.log('  Header:', p.header ? p.header.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ') : 'NONE');
        console.log('  Footer:', p.footer ? p.footer.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ') : 'NONE');
        p.paragraphs.forEach((para, pi) => {
            const txt = para.lines.map(l => l.words.map(w => w.text).join(' ')).join(' ');
            if (txt.trim().match(/^[0-9]+$/)) {
                console.log('  Para ' + pi + ' PAGE NUM: text=' + txt + ', y=[' + para.bbox.y_min.toFixed(1) + '..' + para.bbox.y_max.toFixed(1) + ']');
            }
        });
    });
}
run();

