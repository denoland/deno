// Regression test: EOF on a net.Socket must emit events in the correct order
// and the process must exit cleanly. After the stream infrastructure refactor,
// the explicit readStop call was removed from onStreamRead's EOF path (matching
// Node.js behavior -- the native layer handles stopping the stream on EOF).
// This test verifies: data -> end -> close ordering and clean process exit.

import * as net from "node:net";

const server = net.createServer((socket) => {
  socket.write("hello");
  socket.end();
});

server.listen(0, () => {
  const { port } = server.address() as net.AddressInfo;
  const events: string[] = [];

  const client = net.connect(port, () => {
    events.push("connect");
  });

  client.on("data", () => {
    events.push("data");
  });

  client.on("end", () => {
    events.push("end");
    client.end();
  });

  client.on("close", () => {
    events.push("close");
    const expected = "connect,data,end,close";
    const actual = events.join(",");
    if (actual !== expected) {
      console.log(`FAIL: expected [${expected}], got [${actual}]`);
      process.exit(1);
    }
    console.log("PASS: EOF event ordering is correct");
    server.close();
  });
});
