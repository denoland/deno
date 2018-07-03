// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno. Invoked by mock_runtime_test.cc

const global = this;

function assert(cond) {
  if (!cond) throw Error("mock_runtime.js assert failed");
}

global.typedArrayToArrayBuffer = ta => {
  return ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
};

global.CanCallFunction = () => {
  deno.print("Hello world from foo");
  return "foo";
};

// This object is created to test snapshotting.
// See DeserializeInternalFieldsCallback and SerializeInternalFieldsCallback.
const snapshotted = new Uint8Array([1, 3, 3, 7]);

global.TypedArraySnapshots = () => {
  assert(snapshotted[0] === 1);
  assert(snapshotted[1] === 3);
  assert(snapshotted[2] === 3);
  assert(snapshotted[3] === 7);
};

global.SendSuccess = () => {
  deno.recv((channel, msg) => {
    assert(channel === "SendSuccess");
    deno.print("SendSuccess: ok");
  });
};

global.SendByteLength = () => {
  deno.recv((channel, msg) => {
    assert(channel === "SendByteLength");
    assert(msg instanceof ArrayBuffer);
    assert(msg.byteLength === 3);
  });
};

global.RecvReturnEmpty = () => {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  let r = deno.send("RecvReturnEmpty", ab);
  assert(r == null);
  r = deno.send("RecvReturnEmpty", ab);
  assert(r == null);
};

global.RecvReturnBar = () => {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  const r = deno.send("RecvReturnBar", ab);
  assert(r instanceof ArrayBuffer);
  assert(r.byteLength === 3);
  const rui8 = new Uint8Array(r);
  const rstr = String.fromCharCode(...rui8);
  assert(rstr === "bar");
};

global.DoubleRecvFails = () => {
  // deno.recv is an internal function and should only be called once from the
  // runtime.
  deno.recv((channel, msg) => assert(false));
  deno.recv((channel, msg) => assert(false));
};

// The following join has caused SnapshotBug to segfault when using kKeep.
[].join("");

global.SnapshotBug = () => {
  assert("1,2,3" === String([1, 2, 3]));
};

global.ErrorHandling = () => {
  global.onerror = (message, source, line, col, error) => {
    deno.print(`line ${line} col ${col}`);
    assert("ReferenceError: notdefined is not defined" === message);
    assert(source === "helloworld.js");
    assert(line === 3);
    assert(col === 1);
    assert(error instanceof Error);
    deno.send("ErrorHandling", typedArrayToArrayBuffer(new Uint8Array([42])));
  };
  eval("\n\n notdefined()\n//# sourceURL=helloworld.js");
};
