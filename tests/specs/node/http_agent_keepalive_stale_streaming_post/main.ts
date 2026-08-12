// Test that streaming POST requests (using pipeline) on stale keepAlive
// connections are transparently retried with the request body intact.
// Streaming writes bypass outputData and go directly to _bodyWriter,
// so they need separate handling in the retry logic.
import http from "node:http";
import net from "node:net";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

let idleSocketClosed: Promise<void>;
let resolveIdleSocketClosed: () => void;
idleSocketClosed = new Promise((r) => (resolveIdleSocketClosed = r));

const BODY = '{"test":true}';
const DELAYED_BODY = "chunk-one/chunk-two";
const DELAYED_BODY_FIRST_CHUNK = "chunk-one/";
const LARGE_BODY_FIRST_CHUNK = Buffer.alloc(1024 * 1024 + 1, "a");
const LARGE_BODY_SECOND_CHUNK = Buffer.from("b");
const LARGE_BODY_LENGTH = LARGE_BODY_FIRST_CHUNK.length +
  LARGE_BODY_SECOND_CHUNK.length;

let destroyedDirectWriteAttempt = false;
let destroyedLargeDirectWriteAttempt = false;

const server = net.createServer((socket) => {
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  const resetIdle = () => {
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(() => socket.destroy(), 50);
  };

  let buffer = "";
  socket.on("data", (data) => {
    buffer += data.toString();
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) return;

    const headers = buffer.substring(0, headerEnd);
    const requestLine = headers.split("\r\n", 1)[0];
    const path = requestLine.split(" ")[1];
    const clMatch = headers.match(/content-length:\s*(\d+)/i);
    const contentLength = clMatch ? parseInt(clMatch[1]) : 0;
    const bodyStart = headerEnd + 4;
    const body = buffer.substring(bodyStart, bodyStart + contentLength);

    if (
      path === "/direct-write-retry" &&
      !destroyedDirectWriteAttempt &&
      body.includes(DELAYED_BODY_FIRST_CHUNK) &&
      body.length < contentLength
    ) {
      destroyedDirectWriteAttempt = true;
      socket.destroy();
      return;
    }

    if (
      path === "/direct-write-too-large" &&
      !destroyedLargeDirectWriteAttempt &&
      body.length >= LARGE_BODY_FIRST_CHUNK.length &&
      body.length < contentLength
    ) {
      destroyedLargeDirectWriteAttempt = true;
      socket.destroy();
      return;
    }

    if (body.length < contentLength) return;

    const responseBody = JSON.stringify({ received: body });
    const response = [
      "HTTP/1.1 200 OK",
      `Content-Length: ${responseBody.length}`,
      "Connection: keep-alive",
      "",
      responseBody,
    ].join("\r\n");
    socket.write(response);
    buffer = "";
    resetIdle();
  });

  socket.on("close", () => {
    if (idleTimer) clearTimeout(idleTimer);
    resolveIdleSocketClosed();
  });
});

const agent = new http.Agent({
  keepAlive: true,
  maxSockets: 1,
});

async function waitForFreeSocket() {
  const deadline = Date.now() + 1000;
  while (Date.now() < deadline) {
    for (const sockets of Object.values(agent.freeSockets)) {
      if (sockets.length > 0) return;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error("timed out waiting for free socket");
}

function postStreaming(port: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("timeout")), 5000);
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        method: "POST",
        agent,
        headers: { "Content-Length": String(BODY.length) },
      },
      (res) => {
        let data = "";
        res.on("data", (c: Buffer) => (data += c));
        res.on("end", () => {
          clearTimeout(timeout);
          resolve(data);
        });
      },
    );
    req.on("error", (e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });

    const bodyStream = Readable.from(Buffer.from(BODY));
    pipeline(bodyStream, req).catch((e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });
  });
}

async function* delayedBodyChunks() {
  yield Buffer.from(DELAYED_BODY_FIRST_CHUNK);
  await new Promise((resolve) => setTimeout(resolve, 100));
  yield Buffer.from(DELAYED_BODY.slice(DELAYED_BODY_FIRST_CHUNK.length));
}

function postDelayedStreaming(port: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("timeout")), 5000);
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        path: "/direct-write-retry",
        method: "POST",
        agent,
        headers: { "Content-Length": String(DELAYED_BODY.length) },
      },
      (res) => {
        let data = "";
        res.on("data", (c: Buffer) => (data += c));
        res.on("end", () => {
          clearTimeout(timeout);
          resolve(data);
        });
      },
    );
    req.on("error", (e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });
    req.flushHeaders();

    const bodyStream = Readable.from(delayedBodyChunks());
    pipeline(bodyStream, req).catch((e: Error) => {
      clearTimeout(timeout);
      reject(e);
    });
  });
}

async function* largeBodyChunks() {
  yield LARGE_BODY_FIRST_CHUNK;
  await new Promise((resolve) => setTimeout(resolve, 100));
  yield LARGE_BODY_SECOND_CHUNK;
}

function postLargeDelayedStreaming(port: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("timeout")), 5000);
    const req = http.request(
      {
        hostname: "127.0.0.1",
        port,
        path: "/direct-write-too-large",
        method: "POST",
        agent,
        headers: { "Content-Length": String(LARGE_BODY_LENGTH) },
      },
      () => {
        clearTimeout(timeout);
        reject(new Error("unexpected response"));
      },
    );
    req.on("error", () => {
      clearTimeout(timeout);
      resolve();
    });
    req.flushHeaders();

    const bodyStream = Readable.from(largeBodyChunks());
    pipeline(bodyStream, req).catch(() => {});
  });
}

server.listen(0, async () => {
  const port = (server.address() as net.AddressInfo).port;
  try {
    // First request - establishes keepAlive connection
    const first = await postStreaming(port);
    console.log("Request 1: OK", first);

    // Wait until the server closes the idle socket
    await idleSocketClosed;
    await new Promise((r) => setTimeout(r, 50));

    // Reset the idle close promise for the next connection
    idleSocketClosed = new Promise((r) => (resolveIdleSocketClosed = r));

    // Second request on stale socket - should retry with body intact
    const second = await postStreaming(port);
    console.log("Request 2: OK", second);

    await waitForFreeSocket();

    // Third request writes body chunks directly to a reused socket. The server
    // drops that socket after the first chunk, so retry must replay both the
    // header write and the direct body write.
    const third = await postDelayedStreaming(port);
    console.log("Request 3: OK", third);

    await waitForFreeSocket();

    // Larger direct-write bodies exceed the bounded retry buffer. They should
    // not be retried after the reused socket is dropped.
    await postLargeDelayedStreaming(port);
    console.log("Request 4: OK not retried");

    console.log("PASS");
  } catch (e) {
    console.error("FAIL:", (e as Error).message);
    Deno.exit(1);
  } finally {
    server.close();
    agent.destroy();
  }
});
