import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);
const page1 = pages[0];
console.log('Page 1 glyph count:', page1.glyphs.length);

const relevant = page1.glyphs.filter(g => g.bbox.y_min >= 250 && g.bbox.y_min <= 450);
relevant.sort((a, b) => a.bbox.y_min - b.bbox.y_min);
let lastY = -1;
let line = '';
let minY = 999;
let maxY = 0;
let fontSize = 0;

for (const g of relevant) {
    if (Math.abs(g.bbox.y_min - lastY) > 3) {
        if (line) console.log('Y: ' + lastY.toFixed(1) + ' (h: ' + (maxY - minY).toFixed(1) + ', sz: ' + fontSize.toFixed(1) + ') -> ' + line);
        line = '';
        lastY = g.bbox.y_min;
        minY = g.bbox.y_min;
        maxY = g.bbox.y_max;
        fontSize = g.style.font_size;
    }
    line += g.text;
    minY = Math.min(minY, g.bbox.y_min);
    maxY = Math.max(maxY, g.bbox.y_max);
}
if (line) console.log('Y: ' + lastY.toFixed(1) + ' (h: ' + (maxY - minY).toFixed(1) + ', sz: ' + fontSize.toFixed(1) + ') -> ' + line);
