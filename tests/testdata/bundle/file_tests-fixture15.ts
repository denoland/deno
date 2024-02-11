export function getIndex(c: string): number {
  return "\x00\r\n\x85\u2028\u2029".indexOf(c);
}
