import * as zlib from 'node:zlib';
import { Buffer } from 'node:buffer';
import * as fs from 'node:fs';

const input = fs.createReadStream('input.txt');
const output = fs.createWriteStream('input.txt.deflated');

const deflate = zlib.createDeflate();

input.pipe(deflate).pipe(output);

output.on('finish', () => {
  console.log('File has been compressed using deflate!');
});


// gunzip and gzip roundtrip
const input2 = 'Hello World!';
const expected = Buffer.from(input2);
const compressed = zlib.gzipSync(input2);
const decompressed = zlib.gunzipSync(compressed);

console.log(expected);
console.log(decompressed);