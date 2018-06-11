// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno. Invoked by mock_runtime_test.cc
const window = eval("this");

function assert(cond) {
  if (!cond) throw Error("mock_runtime.js assert failed");
}

function typedArrayToArrayBuffer(ta) {
  return ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
}

function CanCallFunction() {
  deno_print("Hello world from foo");
  return "foo";
}

function PubSuccess() {
  deno_sub(msg => {
    deno_print("PubSuccess: ok");
  });
}

function PubByteLength() {
  deno_sub(msg => {
    assert(msg instanceof ArrayBuffer);
    assert(msg.byteLength === 3);
  });
}

function SubReturnEmpty() {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  let r = deno_pub(ab);
  assert(r == null);
  r = deno_pub(ab);
  assert(r == null);
}

function SubReturnBar() {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  const r = deno_pub(ab);
  assert(r instanceof ArrayBuffer);
  assert(r.byteLength === 3);
  const rui8 = new Uint8Array(r);
  const rstr = String.fromCharCode(...rui8);
  assert(rstr === "bar");
}
