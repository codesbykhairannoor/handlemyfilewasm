import fs from 'fs';
import zlib from 'zlib';

const docxBuf = fs.readFileSync('laporan_test.docx');
let offset = 0;
let docXml = '';
while (offset < docxBuf.length - 4) {
    const sig = docxBuf.readUInt32LE(offset);
    if (sig === 0x04034b50) {
        const compSize = docxBuf.readUInt32LE(offset + 18);
        const nameLen = docxBuf.readUInt16LE(offset + 26);
        const extraLen = docxBuf.readUInt16LE(offset + 28);
        const fileName = docxBuf.toString('utf8', offset + 30, offset + 30 + nameLen);
        const dataStart = offset + 30 + nameLen + extraLen;
        if (fileName === 'word/document.xml') {
            docXml = zlib.inflateRawSync(docxBuf.slice(dataStart, dataStart + compSize)).toString('utf8');
            break;
        }
        offset = dataStart + compSize;
    } else {
        break;
    }
}

// Split by page breaks
const pageBlocks = docXml.split('<w:br w:type="page"/>');
console.log('Total page blocks in docXml:', pageBlocks.length);
pageBlocks.forEach((pb, idx) => {
    const pMatches = [...pb.matchAll(/<w:p(?:[\s\S]*?)<\/w:p>/g)];
    console.log(`\n--- DOCX Section/Page ${idx} (paras=${pMatches.length}) ---`);
    pMatches.forEach((pm, pidx) => {
        const tMatches = [...pm[0].matchAll(/<w:t[^>]*>([^<]*)<\/w:t>/g)].map(m => m[1]).join('');
        const spMatch = pm[0].match(/<w:spacing [^>]+>/);
        const sp = spMatch ? spMatch[0] : '';
        if (tMatches.trim().length > 0) {
            console.log(`  P${pidx} ${sp} : ${tMatches.slice(0, 70)}`);
        }
    });
});

