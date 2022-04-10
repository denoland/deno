export function makeTable() {
  let c: number;
  const table = [];
  const m = 0xEDB88320;

  for (let n = 0; n < 256; n++) {
    c = n;
    for (let k = 0; k < 8; k++) {
      c = ((c & 1) ? (m ^ (c >>> 1)) : (c >>> 1));
    }
    table[n] = c;
  }

  return table;
}

// Create table on load. Just 255 signed longs. Not a problem.
const crcTable = makeTable();

export function crc32(crc: any, buf: any, len: any, pos: any) {
  let t = crcTable;
  let end = pos + len;
  let f = 0xFF;

  crc ^= -1;

  for (let i = pos; i < end; i++) {
    crc = (crc >>> 8) ^ t[(crc ^ buf[i]) & f];
  }

  return (crc ^ (-1)); // >>> 0;
}
