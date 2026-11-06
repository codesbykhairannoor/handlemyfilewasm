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

const pageBlocks = docXml.split('<w:br w:type="page"/>');
console.log('=== Section 4 (BAB II) ===');
const paras = [...pageBlocks[4].matchAll(/<w:p(?:[\s\S]*?)<\/w:p>/g)];
paras.forEach((p, i) => {
    const txt = [...p[0].matchAll(/<w:t[^>]*>([^<]*)<\/w:t>/g)].map(m => m[1]).join('');
    const ind = p[0].match(/<w:ind [^>]+>/)?.[0] || '';
    console.log(`  P${i} ${ind} : ${txt.slice(0, 80)}`);
});

