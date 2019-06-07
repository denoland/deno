// Used for benchmarking Deno's tcp proxy perfromance. See tools/http_benchmark.py
const addr = Deno.args[1] || "127.0.0.1:4500";
const originAddr = Deno.args[2] || "127.0.0.1:4501";

const listener = Deno.listen("tcp", addr);

async function handle(conn: Deno.Conn): Promise<void> {
  const origin = await Deno.dial("tcp", originAddr);
  try {
    await Promise.all([Deno.copy(conn, origin), Deno.copy(origin, conn)]);
  } catch (err) {
    if (err.message !== "read error" && err.message !== "write error") {
      throw err;
    }
  } finally {
    conn.close();
    origin.close();
  }
}

async function main(): Promise<void> {
  console.log(`Proxy listening on http://${addr}/`);
  while (true) {
    const conn = await listener.accept();
    handle(conn);
  }
}

main();
