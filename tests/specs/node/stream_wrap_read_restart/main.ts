// Regression test: read callback teardown and restart behavior.
// After the stream infrastructure refactor, read callbacks are tracked via a
// generational registry (ReadCallbackRegistry). This test verifies that
// pausing and resuming a stream (which triggers readStop/readStart internally)
// correctly tears down and reinstalls the callback state.

import * as net from "node:net";

const CHUNKS = ["chunk1", "chunk2", "chunk3"];
const received: string[] = [];

const server = net.createServer((socket) => {
  let i = 0;
  const sendNext = () => {
    if (i < CHUNKS.length) {
      socket.write(CHUNKS[i++], () => {
        setTimeout(sendNext, 50);
      });
    } else {
      socket.end();
    }
  };
  sendNext();
});

server.listen(0, () => {
  const { port } = server.address() as net.AddressInfo;

  const client = net.connect(port, () => {
    let chunkCount = 0;

    client.on("data", (data: Buffer) => {
      received.push(data.toString());
      chunkCount++;

      if (chunkCount === 1) {
        // Pause the stream (triggers readStop internally)
        client.pause();

        // Resume after a delay (triggers readStart with fresh callback state)
        setTimeout(() => {
          client.resume();
        }, 100);
      }
    });

    client.on("end", () => {
      client.end();
    });

    client.on("close", () => {
      const expected = CHUNKS.join(",");
      const actual = received.join(",");
      if (actual !== expected) {
        console.log(`FAIL: expected [${expected}], got [${actual}]`);
        process.exit(1);
      }
      console.log("PASS: all chunks received after pause/resume cycle");
      server.close();
    });
  });
});
