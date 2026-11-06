import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);
const page3 = pages[2];
const pageW = 595.3;
const pageH = 841.9;

const hLines3 = [];
const vLines3 = [];
for (const p of page3.vector_paths) {
    if (p.y1 < -5 || p.y1 > pageH + 5 || p.y2 < -5 || p.y2 > pageH + 5) continue;
    if (p.x1 < -5 || p.x1 > pageW + 5 || p.x2 < -5 || p.x2 > pageW + 5) continue;
    const dx = Math.abs(p.x2 - p.x1);
    const dy = Math.abs(p.y2 - p.y1);
    if (dx >= pageW * 0.85 && (p.y1 <= 50 || p.y1 >= pageH - 50)) continue;
    if (dy >= pageH * 0.85 && (p.x1 <= 50 || p.x1 >= pageW - 50)) continue;

    if (dy < 1.0 && dx >= 12.0 && p.y1 >= 40 && p.y1 <= 270) {
        hLines3.push({ y: (p.y1 + p.y2) / 2, x1: Math.min(p.x1, p.x2), x2: Math.max(p.x1, p.x2) });
    } else if (dx < 1.0 && dy >= 12.0 && Math.min(p.y1, p.y2) >= 40 && Math.max(p.y1, p.y2) <= 270) {
        vLines3.push({ x: (p.x1 + p.x2) / 2, y1: Math.min(p.y1, p.y2), y2: Math.max(p.y1, p.y2) });
    }
}

function cluster(arr, tol) {
    if (arr.length === 0) return [];
    arr.sort((a, b) => a - b);
    const res = [arr[0]];
    for (let i = 1; i < arr.length; i++) {
        if (arr[i] - res[res.length - 1] > tol) {
            res.push(arr[i]);
        }
    }
    return res;
}

console.log(`Page 3 table lines: H=${hLines3.length}, V=${vLines3.length}`);
console.log('Page 3 Unique Ys:', cluster(hLines3.map(h => h.y), 3.0));
console.log('Page 3 Unique Xs:', cluster(vLines3.map(v => v.x), 3.0));
