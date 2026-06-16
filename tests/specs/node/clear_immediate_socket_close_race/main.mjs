// Regression test for `clearImmediate` semantics across a TCP socket
// 'close' event. Mirrors the pattern that `npm:postgres` (postgres-js)
// uses for its write batching: a write enqueues `setImmediate(nextWrite)`
// and the connection's 'close' handler calls
// `clearImmediate(nextWriteTimer)` to cancel a still-pending write before
// nulling its `socket` reference.
//
// libuv (and therefore both Deno and Node.js) runs the check phase
// (setImmediate) BEFORE the close phase. Once an immediate has been
// dispatched it cannot be retroactively cancelled — that is identical to
// Node.js behavior. The contract is:
//   "clearImmediate(t) prevents t from firing IF t has not yet been picked
//    up by the check phase."
//
// See denoland/deno#34667 and porsager/postgres#1168 for the upstream
// guard that postgres-js adds in `nextWrite` to avoid crashing when user
// code (or a connection pool) writes after `socket = null`.

import { connect, createServer } from "node:net";
import { strictEqual } from "node:assert";

// Case 1: `clearImmediate` cancels a same-tick `setImmediate`.
{
  let fired = false;
  const t = setImmediate(() => {
    fired = true;
  });
  clearImmediate(t);
  await new Promise((r) => setTimeout(r, 20));
  strictEqual(fired, false, "sync clearImmediate must cancel");
}

// Case 2: `clearImmediate` cancels a `setImmediate` from a microtask.
{
  let fired = false;
  const t = setImmediate(() => {
    fired = true;
  });
  await Promise.resolve();
  clearImmediate(t);
  await new Promise((r) => setTimeout(r, 20));
  strictEqual(fired, false, "microtask clearImmediate must cancel");
}

// Case 3: a setImmediate queued before a same-tick `socket.destroy()`
// fires in the check phase BEFORE the close phase fires the 'close'
// event. This matches libuv / Node.js.
{
  const tmp = Deno.listen({ port: 0 });
  const port = tmp.addr.port;
  tmp.close();

  const server = createServer((sock) => {
    sock.on("data", () => {});
  });
  await new Promise((r) => server.listen(port, r));

  let socket = null;
  let immediateFired = false;
  let closeFired = false;
  let immediateRanBeforeClose = false;

  const done = new Promise((resolve) => {
    socket = connect(port, "127.0.0.1", () => {
      socket.on("data", () => {});
      socket.on("error", () => {});
      socket.on("close", () => {
        closeFired = true;
        resolve();
      });
      setImmediate(() => {
        immediateFired = true;
        if (!closeFired) immediateRanBeforeClose = true;
      });
      socket.destroy();
    });
  });

  await done;
  strictEqual(immediateFired, true, "queued immediate must fire");
  strictEqual(closeFired, true, "close event must fire");
  strictEqual(
    immediateRanBeforeClose,
    true,
    "check phase runs before close phase",
  );

  server.close();
}

// Case 4: a `socket && socket.write(...)` guard inside the queued
// callback (matching porsager/postgres#1168) prevents the crash that the
// pre-1168 postgres-js exhibits when a connection pool calls write()
// after the userland `socket` variable has been nulled.
{
  const tmp = Deno.listen({ port: 0 });
  const port = tmp.addr.port;
  tmp.close();

  const server = createServer((sock) => {
    sock.on("data", () => sock.destroy());
  });
  await new Promise((r) => server.listen(port, r));

  let socket = null;
  let nextWriteTimer = null;
  let chunk = null;
  let postCloseImmediateFired = false;

  const nextWrite = () => {
    // Upstream guard from porsager/postgres#1168. Without it the queued
    // callback dereferences a nulled `socket` and crashes.
    const x = socket ? socket.write(chunk) : false;
    if (!socket) postCloseImmediateFired = true;
    if (nextWriteTimer !== null) clearImmediate(nextWriteTimer);
    chunk = nextWriteTimer = null;
    return x;
  };
  const write = (x) => {
    chunk = chunk ? Buffer.concat([chunk, x]) : Buffer.from(x);
    if (nextWriteTimer === null) {
      nextWriteTimer = setImmediate(nextWrite);
    }
  };
  const closed = () => {
    clearImmediate(nextWriteTimer);
    socket = null;
  };

  await new Promise((resolve) => {
    socket = connect(port, "127.0.0.1", () => {
      socket.on("data", () => {});
      socket.on("error", () => {});
      socket.on("close", () => {
        closed();
        // Pool-reuse simulation: user code calls write() AFTER socket = null,
        // enqueueing a brand-new immediate that fires on the next tick.
        write(Buffer.from("post-close"));
        setTimeout(resolve, 50);
      });
      write(Buffer.from("hello"));
    });
  });

  strictEqual(
    postCloseImmediateFired,
    true,
    "the post-close immediate must have fired and been guarded",
  );

  server.close();
}

console.log("clearImmediate socket-close race regression OK");
