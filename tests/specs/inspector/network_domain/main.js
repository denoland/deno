import inspector from "node:inspector/promises";
import { strict as assert } from "node:assert";

const session = new inspector.Session();
session.connect();

// Test 1: Network events are NOT received before Network.enable
let unexpectedCalled = false;
session.on("Network.requestWillBeSent", () => {
  unexpectedCalled = true;
});
inspector.Network.requestWillBeSent({
  requestId: "before-enable",
  request: { url: "http://example.com", method: "GET", headers: {} },
  timestamp: 1,
  wallTime: 1,
});
// Give event loop a chance to deliver
await new Promise((r) => setTimeout(r, 10));
assert.equal(
  unexpectedCalled,
  false,
  "should not receive events before Network.enable",
);
session.removeAllListeners("Network.requestWillBeSent");
console.log("PASS: no events before enable");

// Test 2: Network.enable then receive events
await session.post("Network.enable");
const gotEvent = new Promise((resolve) => {
  session.once("Network.requestWillBeSent", ({ params }) => {
    resolve(params);
  });
});
inspector.Network.requestWillBeSent({
  requestId: "req-1",
  request: { url: "https://deno.land", method: "POST", headers: {} },
  timestamp: 2000,
  wallTime: 2000,
});
const params = await gotEvent;
assert.equal(params.requestId, "req-1");
assert.equal(params.request.url, "https://deno.land");
assert.equal(params.request.method, "POST");
// hasPostData should be auto-added for requestWillBeSent
assert.equal(params.request.hasPostData, false);
// initiator should be auto-captured
assert.ok(params.initiator, "initiator should be present");
assert.ok(
  params.initiator.type === "script" || params.initiator.type === "other",
  "initiator.type should be script or other",
);
console.log("PASS: requestWillBeSent with augmentation");

// Test 3: responseReceived (no augmentation)
const gotResponse = new Promise((resolve) => {
  session.once("Network.responseReceived", ({ params }) => {
    resolve(params);
  });
});
inspector.Network.responseReceived({
  requestId: "req-1",
  timestamp: 2001,
  type: "Document",
  response: {
    url: "https://deno.land",
    status: 200,
    statusText: "OK",
    headers: {},
    mimeType: "text/html",
    charset: "utf-8",
  },
});
const respParams = await gotResponse;
assert.equal(respParams.requestId, "req-1");
assert.equal(respParams.response.status, 200);
// Should NOT have initiator (only requestWillBeSent and webSocketCreated get it)
assert.equal(respParams.initiator, undefined);
console.log("PASS: responseReceived without augmentation");

// Test 4: loadingFinished
const gotFinished = new Promise((resolve) => {
  session.once("Network.loadingFinished", ({ params }) => {
    resolve(params);
  });
});
inspector.Network.loadingFinished({
  requestId: "req-1",
  timestamp: 2002,
});
const finParams = await gotFinished;
assert.equal(finParams.requestId, "req-1");
console.log("PASS: loadingFinished");

// Test 5: webSocketCreated (gets initiator)
const gotWs = new Promise((resolve) => {
  session.once("Network.webSocketCreated", ({ params }) => {
    resolve(params);
  });
});
inspector.Network.webSocketCreated({
  requestId: "ws-1",
  url: "ws://localhost:8080",
});
const wsParams = await gotWs;
assert.equal(wsParams.requestId, "ws-1");
assert.ok(wsParams.initiator, "webSocketCreated should have initiator");
console.log("PASS: webSocketCreated with initiator");

// Test 6: Network.disable stops events
await session.post("Network.disable");
let afterDisable = false;
session.on("Network.loadingFailed", () => {
  afterDisable = true;
});
inspector.Network.loadingFailed({
  requestId: "req-2",
  timestamp: 3000,
  type: "Document",
  errorText: "error",
});
await new Promise((r) => setTimeout(r, 10));
assert.equal(afterDisable, false, "should not receive events after disable");
console.log("PASS: no events after disable");

// Test 7: All Network methods exist
const expectedMethods = [
  "requestWillBeSent",
  "responseReceived",
  "loadingFinished",
  "loadingFailed",
  "dataReceived",
  "dataSent",
  "webSocketCreated",
  "webSocketHandshakeResponseReceived",
  "webSocketClosed",
];
for (const method of expectedMethods) {
  assert.equal(
    typeof inspector.Network[method],
    "function",
    `Network.${method} should be a function`,
  );
}
console.log("PASS: all Network methods exist");

session.disconnect();
console.log("ALL PASSED");
