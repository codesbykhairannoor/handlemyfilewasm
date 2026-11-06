import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);

console.log('Total pages:', pages.length);

for (let pIdx = 0; pIdx < pages.length; pIdx++) {
    const page = pages[pIdx];
    const glyphs = page.glyphs;
    glyphs.sort((a,b) => a.bbox.y_min - b.bbox.y_min);
    
    let lastY = -1;
    let line = '';
    let maxSz = 0;
    let isBold = false;
    
    console.log(`\n=== PAGE ${pIdx + 1} (glyphs: ${glyphs.length}) ===`);
    for (const g of glyphs) {
        if (Math.abs(g.bbox.y_min - lastY) > 3) {
            if (line.trim().length > 0 && (maxSz >= 13.0 || isBold)) {
                console.log(`P${pIdx+1} Y:${lastY.toFixed(1)} [sz:${maxSz.toFixed(1)}${isBold ? ' BOLD' : ''}] ${line.trim()}`);
            }
            line = '';
            lastY = g.bbox.y_min;
            maxSz = g.style.font_size;
            isBold = g.style.is_bold;
        }
        line += g.text;
        maxSz = Math.max(maxSz, g.style.font_size);
        if (g.style.is_bold) isBold = true;
    }
    if (line.trim().length > 0 && (maxSz >= 13.0 || isBold)) {
        console.log(`P${pIdx+1} Y:${lastY.toFixed(1)} [sz:${maxSz.toFixed(1)}${isBold ? ' BOLD' : ''}] ${line.trim()}`);
    }
}

