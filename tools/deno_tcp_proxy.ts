// Used for benchmarking Deno's tcp proxy perfromance. See tools/http_benchmark.py
const addr = Deno.args[1] || "127.0.0.1:4500";
const originAddr = Deno.args[2] || "127.0.0.1:4501";

const listener = Deno.listen("tcp", addr);

async function handle(conn: Deno.Conn): Promise<void> {
  const origin = await Deno.dial("tcp", originAddr);
  const buffer = new Uint8Array(1024);
  const originBuffer = new Uint8Array(1024);

  let connPromise: Promise<{ eof: boolean }> | null;
  let originPromise: Promise<{ eof: boolean }> | null;

  let connEof = false;
  let originEof = false;

  try {
    while (true) {
      const ops = [];
      if (!connPromise && !connEof) {
        connPromise = proxyRead(conn, origin, buffer);
        connPromise.then(eof => {
          if (eof) connEof = true;
          connPromise = null;
        });
        ops.push(connPromise);
      }

      if (!originPromise && !originEof) {
        originPromise = proxyRead(origin, conn, buffer);
        originPromise.then(eof => {
          if (eof) connEof = true;
          originPromise = null;
        });
        ops.push(originPromise);
      }

      if (connPromise === null && originPromise === null) {
        break;
      }
      const r = await Promise.race(ops);

      if (connEof && originEof) {
        break;
      }
    }
  } finally {
    conn.close();
    origin.close();
  }
}

async function proxyRead(
  source: any,
  dest: any,
  buffer: Uint8Array
): Promise<{ eof: boolean }> {
  try {
    const r = await source.read(buffer);
    try {
      await dest.write(buffer);
    } catch (err) {
      console.error("Error writing:", dest, err);
      return { eof: true };
    }
    return r;
  } catch (err) {
    console.error("Error reading:", source);
    return { eof: true };
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
