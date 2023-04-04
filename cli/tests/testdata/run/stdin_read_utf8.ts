const buf = new Uint8Array(4);
const n = Deno.stdin.readSync(buf);
console.log(buf.slice(0, n));
