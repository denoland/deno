// Copyright 2018-2026 the Deno authors. MIT license.
// A cancelled request must not be resurrected by the stale-socket retry path.
import assert from "node:assert/strict";
import { once } from "node:events";
import { readFileSync } from "node:fs";
import http from "node:http";
import https from "node:https";

const [protocol, mode, connection] = process.argv.slice(2);
assert(["http", "https"].includes(protocol));
assert(
  ["destroy-error", "destroy", "abort", "signal", "timeout"].includes(mode),
);
assert(connection === undefined || connection === "fresh");
const keepAlive = connection !== "fresh";
const client = protocol === "https" ? https : http;
const tls = protocol === "https"
  ? {
    key: readFileSync(
      new URL("../../../testdata/tls/localhost.key", import.meta.url),
    ),
    cert: readFileSync(
      new URL("../../../testdata/tls/localhost.crt", import.meta.url),
    ),
    ca: readFileSync(
      new URL("../../../testdata/tls/RootCA.pem", import.meta.url),
    ),
  }
  : {};
const agent = new client.Agent({
  keepAlive,
  ca: tls.ca,
  servername: "localhost",
});
const received = [];
const receivedBody = Promise.withResolvers();
const server = client.createServer(tls, (req, res) => {
  let body = "";
  req.setEncoding("utf8");
  req.on("data", (chunk) => body += chunk);
  req.on("end", () => {
    received.push(body);
    if (received.length === 2) {
      receivedBody.resolve();
      // Hold the first response until the client cancels. A mistaken retry
      // receives a response so the test fails instead of hanging.
      return;
    }
    res.end("ok");
  });
});
const deadline = setTimeout(() => {
  console.error("cancelled request did not close");
  process.exit(1);
}, 20_000);

try {
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const options = {
    host: "127.0.0.1",
    port: server.address().port,
    method: "POST",
    agent,
  };
  const freed = keepAlive ? once(agent, "free") : Promise.resolve();
  await new Promise((resolve, reject) => {
    const req = client.request(options, (res) => {
      res.resume();
      res.once("end", resolve);
    });
    req.on("error", reject);
    req.end("warmup");
  });
  await freed;

  const controller = new AbortController();
  const reason = new Error("intentional cancellation");
  const errors = [];
  let responses = 0;
  const req = client.request(
    { ...options, signal: controller.signal },
    (res) => {
      responses++;
      res.resume();
    },
  );
  req.on("error", (error) => errors.push(error));
  const closed = new Promise((resolve) => req.once("close", resolve));
  req.end("cancel");

  // Prove the first POST reached the server and the intended socket was used.
  await receivedBody.promise;
  assert.equal(req.reusedSocket, keepAlive);
  switch (mode) {
    case "destroy-error":
      req.destroy(reason);
      break;
    case "destroy":
      req.destroy();
      break;
    case "abort":
      req.abort();
      break;
    case "signal":
      controller.abort(reason);
      break;
    case "timeout":
      req.setTimeout(1, () => req.destroy(reason));
      break;
  }
  await closed;

  assert.deepEqual(received, ["warmup", "cancel"]);
  assert.equal(responses, 0);
  assert.equal(req.destroyed, true);
  assert.equal(errors.length, 1);
  if (mode === "destroy-error" || mode === "timeout") {
    assert.equal(errors[0], reason);
  } else if (mode === "signal") {
    assert.equal(errors[0].name, "AbortError");
    assert.equal(errors[0].cause, reason);
  } else {
    assert.equal(errors[0].code, "ECONNRESET");
  }
  console.log("ok");
} finally {
  agent.destroy();
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  clearTimeout(deadline);
}
