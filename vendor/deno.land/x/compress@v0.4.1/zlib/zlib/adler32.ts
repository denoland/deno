export default function adler32(adler: any, buf: any, len: any, pos: any) {
  let s1 = (adler & 0xffff) | 0;
  let s2 = ((adler >>> 16) & 0xffff) | 0;
  let n = 0;

  while (len !== 0) {
    // Set limit ~ twice less than 5552, to keep
    // s2 in 31-bits, because we force signed ints.
    // in other case %= will fail.
    n = len > 2000 ? 2000 : len;
    len -= n;

    do {
      s1 = (s1 + buf[pos++]) | 0;
      s2 = (s2 + s1) | 0;
    } while (--n);

    s1 %= 65521;
    s2 %= 65521;
  }

  return (s1 | (s2 << 16)) | 0;
}
