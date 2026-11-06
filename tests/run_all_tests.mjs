import { execSync } from 'child_process';
import fs from 'fs';

const testFiles = [
    'tests/test_wasm_step3.js',
    'tests/test_wasm_step4.js',
    'tests/test_wasm_step5.js',
    'tests/test_wasm_step6.js',
    'tests/test_wasm_step7.js',
    'tests/test_wasm_step8.js',
    'tests/test_wasm_step9.js',
    'tests/test_wasm_step10.js',
    'tests/test_margins_alignment_spacing.mjs',
    'tests/test_images_and_tables_precision.mjs'
];

for (const file of testFiles) {
    console.log(`\n================== RUNNING ${file} ==================`);
    try {
        const out = execSync(`node ${file}`, { stdio: 'inherit' });
        console.log(`>>> ${file} PASSED!`);
    } catch (e) {
        console.error(`>>> ${file} FAILED!`, e.message);
        process.exit(1);
    }
}
console.log('\n=============================================');
console.log('>>> ALL 10 WASM TEST SUITES PASSED 100%! <<<');
console.log('=============================================');

