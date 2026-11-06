import fs from 'fs';
import zlib from 'zlib';

function parseZipEntries(zipBuffer) {
    const entries = {};
    let offset = 0;
    while (offset < zipBuffer.length - 4) {
        const sig = zipBuffer.readUInt32LE(offset);
        if (sig === 0x04034b50) {
            const method = zipBuffer.readUInt16LE(offset + 8);
            const compSize = zipBuffer.readUInt32LE(offset + 18);
            const uncompSize = zipBuffer.readUInt32LE(offset + 22);
            const nameLen = zipBuffer.readUInt16LE(offset + 26);
            const extraLen = zipBuffer.readUInt16LE(offset + 28);
            const fileName = zipBuffer.toString('utf8', offset + 30, offset + 30 + nameLen);
            const dataStart = offset + 30 + nameLen + extraLen;
            const compressedData = zipBuffer.slice(dataStart, dataStart + compSize);
            entries[fileName] = { method, compSize, uncompSize, data: compressedData };
            offset = dataStart + compSize;
        } else {
            break;
        }
    }
    return entries;
}

function decompressEntry(entry) {
    if (entry.method === 0) return entry.data.toString('utf8');
    if (entry.method === 8) return zlib.inflateRawSync(entry.data).toString('utf8');
    throw new Error('Unsupported method: ' + entry.method);
}

import init, { convert_pdf_to_docx } from '../pkg/pdftodocx.js';
const wasmBuffer = fs.readFileSync('./pkg/pdftodocx_bg.wasm');
await init(wasmBuffer);
const pdfBytes = fs.readFileSync('real_sample.pdf');
const docxBytes = convert_pdf_to_docx(pdfBytes);
fs.writeFileSync('real_sample_out.docx', docxBytes);

const entries = parseZipEntries(Buffer.from(docxBytes));
const xml = decompressEntry(entries['word/document.xml']);

console.log('DOCX Size:', docxBytes.length);

const tMatches = [...xml.matchAll(/<w:t[^>]*>(.*?)<\/w:t>/g)].map(m => m[1]);
const fullText = tMatches.join('');

const checks = [
    ['are everything you', 'areeverythingyou'],
    ['quality is the', 'qualityisthe'],
    ['features and you', 'featuresandyou'],
    ['inputs block the', 'inputsblockthe'],
    ['Keep it neutral. A', 'Keep it neutral.A'],
    ['not the screen. A', 'not the screen.A'],
    ['empty public repo', 'emptypublicrepo']
];

console.log('\n--- GLUED WORDS VS PROPER SPACES CHECK ---');
for (const [good, bad] of checks) {
    console.log(`"${good}" (good) => ${fullText.includes(good)} | "${bad}" (bad glued) => ${fullText.includes(bad)}`);
}

// Find all paragraphs with Heading styles
console.log('\n--- ALL HEADINGS IN DOCX ---');
const pRegex = /<w:p\b[\s\S]*?<\/w:p>/g;
let match;
while ((match = pRegex.exec(xml)) !== null) {
    const pXml = match[0];
    const styleMatch = pXml.match(/<w:pStyle w:val="(Heading\d)"\/>/);
    if (styleMatch) {
        const tTexts = [...pXml.matchAll(/<w:t[^>]*>(.*?)<\/w:t>/g)].map(m => m[1]).join('');
        console.log(`[${styleMatch[1]}] ${tTexts}`);
    }
}

// Check tab count
console.log('\n--- SECTION TITLES CHECK ---');
const pRegex2 = /<w:p\b[\s\S]*?<\/w:p>/g;
let match2;
while ((match2 = pRegex2.exec(xml)) !== null) {
    const pXml = match2[0];
    const text = [...pXml.matchAll(/<w:t[^>]*>(.*?)<\/w:t>/g)].map(m => m[1]).join('');
    if (text.includes('The role, in short') || text.includes('What we are really looking at') || text.includes('Our quality bar') || text.includes('Setup: you work')) {
        console.log('--- FOUND ---');
        console.log('Text:', text);
        const styleMatch = pXml.match(/<w:pStyle w:val="([^"]+)"\/>/);
        console.log('Style:', styleMatch ? styleMatch[1] : 'NONE');
        const rPrs = [...pXml.matchAll(/<w:rPr>([\s\S]*?)<\/w:rPr>/g)].map(m => m[1]);
        console.log('rPrs:', rPrs);
    }
}

const tabs = (xml.match(/<w:tab\/>/g) || []).length;
console.log('\nTotal <w:tab/> occurrences:', tabs);

// Check br count
const brs = (xml.match(/<w:br\/>/g) || []).length;
console.log('Total <w:br/> occurrences:', brs);

// Check tables
const tables = (xml.match(/<w:tbl>/g) || []).length;
console.log('Total <w:tbl> occurrences:', tables);
