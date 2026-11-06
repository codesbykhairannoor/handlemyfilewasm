import fs from 'fs';
import init, { inspect_pdf_elements } from '../pkg/pdftodocx.js';

const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);

const pdfBytes = fs.readFileSync('real_sample.pdf');
const pages = inspect_pdf_elements(pdfBytes);

for (let pIdx = 0; pIdx < pages.length; pIdx++) {
    const page = pages[pIdx];
    const vps = page.vector_paths;
    const pageW = page.width_points || 595.3;
    const pageH = page.height_points || 841.9;

    // Separate into H and V lines, filtering out lines outside page or exactly on full page border
    const hLines = [];
    const vLines = [];
    for (const p of vps) {
        // Discard lines outside page bounds
        if (p.y1 < -5 || p.y1 > pageH + 5 || p.y2 < -5 || p.y2 > pageH + 5) continue;
        if (p.x1 < -5 || p.x1 > pageW + 5 || p.x2 < -5 || p.x2 > pageW + 5) continue;

        const dx = Math.abs(p.x2 - p.x1);
        const dy = Math.abs(p.y2 - p.y1);

        // Filter out full-page border lines (width >= 0.85 page width or height >= 0.85 page height)
        if (dx >= pageW * 0.85 && (p.y1 <= 50 || p.y1 >= pageH - 50)) continue;
        if (dy >= pageH * 0.85 && (p.x1 <= 50 || p.x1 >= pageW - 50)) continue;

        if (dy < 1.0 && dx >= 12.0) {
            hLines.push({ y: (p.y1 + p.y2) / 2, x1: Math.min(p.x1, p.x2), x2: Math.max(p.x1, p.x2) });
        } else if (dx < 1.0 && dy >= 12.0) {
            vLines.push({ x: (p.x1 + p.x2) / 2, y1: Math.min(p.y1, p.y2), y2: Math.max(p.y1, p.y2) });
        }
    }

    // Connected components of intersecting lines
    // Adjacency: H line intersects V line if v.x in [h.x1-3, h.x2+3] and h.y in [v.y1-3, v.y2+3]
    const adjH = hLines.map(() => []);
    const adjV = vLines.map(() => []);

    for (let i = 0; i < hLines.length; i++) {
        for (let j = 0; j < vLines.length; j++) {
            const h = hLines[i];
            const v = vLines[j];
            if (v.x >= h.x1 - 3 && v.x <= h.x2 + 3 && h.y >= v.y1 - 3 && h.y <= v.y2 + 3) {
                adjH[i].push(j);
                adjV[j].push(i);
            }
        }
    }

    const visitedH = new Set();
    const visitedV = new Set();
    const components = [];

    for (let i = 0; i < hLines.length; i++) {
        if (visitedH.has(i)) continue;
        const compH = [];
        const compV = [];
        const queueH = [i];
        visitedH.add(i);

        while (queueH.length > 0) {
            const curH = queueH.pop();
            compH.push(curH);
            for (const vIdx of adjH[curH]) {
                if (!visitedV.has(vIdx)) {
                    visitedV.add(vIdx);
                    compV.push(vIdx);
                    for (const nextH of adjV[vIdx]) {
                        if (!visitedH.has(nextH)) {
                            visitedH.add(nextH);
                            queueH.push(nextH);
                        }
                    }
                }
            }
        }

        if (compH.length >= 2 && compV.length >= 2) {
            components.push({ h: compH.map(idx => hLines[idx]), v: compV.map(idx => vLines[idx]) });
        }
    }

    console.log(`\nPage ${pIdx + 1}: Found ${components.length} connected line components`);
    for (let cIdx = 0; cIdx < components.length; cIdx++) {
        const comp = components[cIdx];
        const minX = Math.min(...comp.h.map(l => l.x1), ...comp.v.map(l => l.x));
        const maxX = Math.max(...comp.h.map(l => l.x2), ...comp.v.map(l => l.x));
        const minY = Math.min(...comp.h.map(l => l.y), ...comp.v.map(l => l.y1));
        const maxY = Math.max(...comp.h.map(l => l.y), ...comp.v.map(l => l.y2));
        const w = maxX - minX;
        const h = maxY - minY;
        console.log(`  Comp ${cIdx}: bbox=[${minX.toFixed(1)}, ${minY.toFixed(1)}, ${maxX.toFixed(1)}, ${maxY.toFixed(1)}], w=${w.toFixed(1)}, h=${h.toFixed(1)}, H_lines=${comp.h.length}, V_lines=${comp.v.length}`);
    }
}
