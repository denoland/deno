import { once } from "node:events";
import assert from "node:assert/strict";
import * as http2 from "node:http2";

const server = http2.createServer();

let serverSessionClosed = false;
let resolveServerSessionClosed: (() => void) | undefined;
const serverSessionClosedPromise = new Promise<void>((resolve) => {
  resolveServerSessionClosed = resolve;
});
const serverSessionReady = new Promise<void>((resolve, reject) => {
  server.once("session", (session) => {
    try {
      assert.equal(session.connecting, false);
      assert.equal(typeof session.socket.remoteAddress, "string");
      assert.equal(typeof session.socket.localAddress, "string");
      session.socket.once("close", () => {
        serverSessionClosed = true;
        resolveServerSessionClosed?.();
      });
      resolve();
    } catch (error) {
      reject(error);
    }
  });
});

server.on("stream", (stream) => {
  stream.respond({
    [http2.constants.HTTP2_HEADER_STATUS]: http2.constants.HTTP_STATUS_OK,
  });
  stream.end("ok");
});

await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
const port = (server.address() as { port: number }).port;

const client = http2.connect(`http://127.0.0.1:${port}`);

try {
  await serverSessionReady;
  await once(client, "remoteSettings");

  await new Promise<void>((resolve, reject) => {
    const sent = client.ping((error, duration, payload) => {
      try {
        assert.equal(error, null);
        assert.equal(payload.length, 8);
        assert.equal(duration >= 0, true);
        resolve();
      } catch (assertionError) {
        reject(assertionError);
      }
    });
    assert.equal(sent, true);
  });

  const request = client.request({
    [http2.constants.HTTP2_HEADER_METHOD]: http2.constants.HTTP2_METHOD_GET,
    [http2.constants.HTTP2_HEADER_PATH]: "/",
  });

  request.setEncoding("utf8");
  const chunks: string[] = [];
  request.on("data", (chunk) => chunks.push(chunk));
  await once(request, "end");
  assert.equal(chunks.join(""), "ok");

  client.close();
  await once(client, "close");
  if (!serverSessionClosed) {
    await Promise.race([
      serverSessionClosedPromise,
      new Promise((_, reject) =>
        setTimeout(() => reject(new Error("server session did not close")), 1_000)
      ),
    ]);
  }
  assert.equal(serverSessionClosed, true);

  console.log("HTTP2_SESSION_OK");
} finally {
  server.close();
}
