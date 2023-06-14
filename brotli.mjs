import { createBrotliCompress, createBrotliDecompress, brotliCompressSync, brotliDecompressSync } from 'node:zlib';
import { Buffer } from 'node:buffer';
import  {createWriteStream, createReadStream} from 'node:fs';

// Compress file hello.txt to hello.txt.br
const compress = createBrotliCompress();
const input = createReadStream('README.md');
const output = createWriteStream('hello.txt.br');

const stream = input.pipe(compress).pipe(output);

stream.on('finish', () => {
  console.log('Done compressing 😎');

const decompress = createBrotliDecompress();
const input2 = createReadStream('hello.txt.br');
const output2 = createWriteStream('hello.txt');

const stream2 = input2.pipe(decompress).pipe(output2);

stream2.on('finish', () => {
  console.log('Done decompressing 😎');
});
});

const buf = Buffer.from('hello world');
const compressed = brotliCompressSync(buf);
const decompressed = brotliDecompressSync(compressed);
console.log(decompressed.toString());

