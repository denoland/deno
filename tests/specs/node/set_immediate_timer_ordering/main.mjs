// Node.js guarantees: inside an I/O callback, setImmediate fires before
// setTimeout(fn, 0). This is because setImmediate runs in libuv's check phase
// (after I/O poll), while timers run in the timers phase of the NEXT iteration.
// https://nodejs.org/en/learn/asynchronous-work/event-loop-timers-and-nexttick#setimmediate-vs-settimeout

import { connect, createServer } from "node:net";
import { strictEqual } from "node:assert";

// Get a free port via Deno, then release it for the Node net server.
const tmp = Deno.listen({ port: 0 });
const PORT = tmp.addr.port;
tmp.close();

const order = [];

const server = createServer((socket) => {
  socket.on("data", () => {
    setTimeout(() => {
      order.push("setTimeout");
      check();
    }, 0);

    setImmediate(() => {
      order.push("setImmediate");
      check();
    });

    socket.end();
  });
});

server.listen(PORT, "127.0.0.1", () => {
  const client = connect(PORT, "127.0.0.1", () => {
    client.write("hello");
  });
  client.on("data", () => {});
  client.on("error", () => {});
  client.on("close", () => {
    server.close();
  });
});

function check() {
  if (order.length === 2) {
    strictEqual(order[0], "setImmediate");
    strictEqual(order[1], "setTimeout");
    console.log("setImmediate fired before setTimeout inside I/O callback");
  }
}
