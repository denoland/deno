// Regression test for https://github.com/denoland/deno/issues/35876
// Destroying a `node:net` socket must not leak its `Socket`/`TCPWrap`: the
// native handle's active read (started by the implicit `read(0)` on connect)
// has to be stopped on close so the read-callback registry drops its strong
// `Global<this>` and the wrapper becomes collectable.
import net from "node:net";

const N = 200;
const gc = (globalThis as unknown as { gc: () => void }).gc;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const server = net.createServer(() => {});
await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
const { port } = server.address() as net.AddressInfo;

const refs: WeakRef<net.Socket>[] = [];
for (let i = 0; i < N; i++) {
  const s = net.connect(port, "127.0.0.1");
  await new Promise<void>((r) => {
    s.once("connect", () => r());
    s.once("error", () => r());
  });
  s.destroy();
  refs.push(new WeakRef(s));
  await sleep(2);
}

for (let i = 0; i < 5; i++) {
  gc();
  await sleep(20);
}

server.close();

const alive = refs.filter((r) => r.deref() !== undefined).length;
console.log(
  alive === 0
    ? "ok"
    : `LEAK: ${alive}/${N} destroyed sockets still reachable after GC`,
);
