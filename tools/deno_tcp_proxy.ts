// Used for benchmarking Deno's tcp proxy perfromance. See tools/http_benchmark.py
const addr = Deno.args[1] || "127.0.0.1:4500";
const originAddr = Deno.args[2] || "127.0.0.1:4501";

const listener = Deno.listen("tcp", addr);

async function handle(conn: Deno.Conn): Promise<void> {
  const origin = await Deno.dial("tcp", originAddr);
  const buffer = new Uint8Array(1024);
  const originBuffer = new Uint8Array(1024);
  try {
    while (true) {
      const r = await conn.read(buffer);

      const inbound = conn.read(buffer).then(() => origin.write(buffer));
      const outbound = origin.read(buffer).then(() => origin.write(buffer));

      const [r1, r2] = await Promise.all([inbound, outbound]);

      if (r1.eof || r2.eof) {
        break;
      }
    }
  } finally {
    conn.close();
    origin.close();
  }
}

async function main(): Promise<void> {
  console.log("Listening on", addr);
  while (true) {
    const conn = await listener.accept();
    handle(conn);
  }
}

main();
