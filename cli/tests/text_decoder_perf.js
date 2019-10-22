function r32() {
  return Math.floor(Math.random() * 0xffffffff);
}

function generateRandomUTF8(size) {
  const utf8 = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    const r = r32();
    const len = 1 + (r & 0x3);
    switch (len) {
      case 0:
        utf8[i] = r % 128;
        break;
      case 1:
        utf8[i] = 128 + (r % (2048 - 128));
        break;
      case 2:
        utf8[i] = 2048 + (r % (65536 - 2048));
        break;
      case 3:
        utf8[i] = 65536 + (r % (131072 - 65536));
        break;
    }
  }
  return utf8;
}

new TextDecoder().decode(generateRandomUTF8(20 * 1024 * 1024));
