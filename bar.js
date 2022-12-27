console.log("before connected");
const conn = await Deno.connect({ port: 3500 });
console.log("connected");
conn.unref();
console.log("unrefed in program");

setTimeout(() => {
  console.log("timeout");
  const m = Deno.metrics();
  const ops = m.ops;

  for (const opName of Object.keys(ops)) {
    const op = ops[opName];
    if (op.opsDispatchedAsync != op.opsCompletedAsync) {
      console.log(opName, op);
    }
  }
}, 1000);

// while (true) {

// }

const buf = new Uint8Array(10);
const nread = await conn.read(buf); // The program exits here
console.log("read", nread, buf);

// console.log(m);

// console.log("after read");
// throw new Error(); // The program doesn't reach here
