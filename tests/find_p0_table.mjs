import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);

// Check Page 2 and Page 3 glyphs and vector paths
for (const pNum of [2, 3]) {
    const page = pages[pNum - 1];
    console.log(`\n=== PAGE ${pNum} ===`);
    const p0 = page.glyphs.find(g => g.text === 'P' && page.glyphs.some(g2 => g2.text === '0' && Math.abs(g2.bbox.y_min - g.bbox.y_min) < 3));
    if (p0) {
        console.log(`P0 found at Y: ${p0.bbox.y_min.toFixed(1)}, X: ${p0.bbox.x_min.toFixed(1)}`);
        // Find vector paths near this Y
        const nearVp = page.vector_paths.filter(vp => {
            const minY = Math.min(vp.y1, vp.y2);
            const maxY = Math.max(vp.y1, vp.y2);
            return maxY >= p0.bbox.y_min - 50 && minY <= p0.bbox.y_min + 150;
        });
        console.log(`Vector paths near P0 (within 100pt): ${nearVp.length}`);
        for (const vp of nearVp) {
            console.log(`  (${vp.x1.toFixed(1)}, ${vp.y1.toFixed(1)}) -> (${vp.x2.toFixed(1)}, ${vp.y2.toFixed(1)})`);
        }
    } else {
        console.log('P0 not on page ' + pNum);
    }
}

