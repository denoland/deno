import * as zlib from 'node:zlib';
import * as fs from 'node:fs';

const input = fs.createReadStream('input.txt');
const output = fs.createWriteStream('input.txt.deflated');

const deflate = zlib.createDeflate();

input.pipe(deflate).pipe(output);

output.on('finish', () => {
  console.log('File has been compressed using deflate!');
});
