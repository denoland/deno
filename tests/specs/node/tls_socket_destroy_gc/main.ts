// Regression test for https://github.com/denoland/deno/issues/35876 (TLS case).
// Destroying a `node:tls` socket must not leak its `Socket`/`TLSWrap` and the
// underlying `TCPWrap`. The active read on the underlying TCP handle (started
// by the implicit `read(0)` on connect) has to be stopped on close so the
// read-callback registry drops its strong `Global<this>` and the wrappers
// become collectable. node-postgres-over-TLS churns pooled connections through
// exactly this path.
import tls from "node:tls";
import { readFileSync } from "node:fs";

const N = 200;
const gc = (globalThis as unknown as { gc: () => void }).gc;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const server = tls.createServer({ key, cert }, () => {});
await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
const { port } = server.address() as { port: number };

const refs: WeakRef<tls.TLSSocket>[] = [];
for (let i = 0; i < N; i++) {
  const s = tls.connect({ host: "127.0.0.1", port, rejectUnauthorized: false });
  await new Promise<void>((r) => {
    s.once("secureConnect", () => r());
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
    : `LEAK: ${alive}/${N} destroyed TLS sockets still reachable after GC`,
);
