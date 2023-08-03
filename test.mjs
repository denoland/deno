import { Buffer } from 'node:buffer';
import { createDeflate } from 'node:zlib';

createDeflate({
  dictionary: Buffer.alloc(0)
});
console.log('All good!');
