// Regression test: TLSWrap readStop must properly stop delivering decrypted
// data to JS. Previously, do_emit_read had a fallback that looked up "onread"
// on the JS handle object, bypassing readStop and breaking backpressure.
// This caused data to be delivered out of order and EOF to be missed when
// the readable stream's internal buffer was full.

import tls from "node:tls";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

// Use a small highWaterMark to trigger backpressure quickly
const HWM = 1024;
// Send enough data to exceed the HWM multiple times
const TOTAL = HWM * 64;

const server = tls.createServer({ key, cert }, (socket) => {
  // Write a large chunk that will trigger backpressure on the client
  socket.end(Buffer.alloc(TOTAL, 0x41));
});

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address() as { port: number };

  const client = tls.connect(
    {
      host: "127.0.0.1",
      port,
      rejectUnauthorized: false,
      highWaterMark: HWM,
    },
    () => {
      let received = 0;
      let ended = false;

      client.on("data", (chunk: Buffer) => {
        received += chunk.length;
      });

      client.on("end", () => {
        ended = true;
        clearTimeout(timer);
        console.log(
          `received=${received} expected=${TOTAL} match=${received === TOTAL}`,
        );
        console.log(`ended=${ended}`);
        if (received === TOTAL && ended) {
          console.log("ok");
        } else {
          console.log("FAIL: data mismatch or end not received");
        }
        client.destroy();
        server.close();
      });
    },
  );

  // Safety timeout
  const timer = setTimeout(() => {
    console.log(
      "FAIL: timed out (readStop likely blocked data/EOF delivery)",
    );
    client.destroy();
    server.close();
    Deno.exit(1);
  }, 10000);
  // Unref so it doesn't keep the process alive if test passes
  timer.unref?.();
});
