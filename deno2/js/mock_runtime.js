// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno.
const window = eval("this");
window["foo"] = () => {
  deno_print("Hello world from foo");
  return "foo";
};

function assert(cond) {
  if (!cond) throw Error("mock_runtime.js assert failed");
}

function subabc() {
  deno_sub(msg => {
    assert(msg instanceof ArrayBuffer);
    assert(msg.byteLength === 3);
  });
}

function typedArrayToArrayBuffer(ta) {
  return ta.buffer.slice(ta.byteOffset, ta.byteOffset + ta.byteLength);
}

function pubReturnEmpty() {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  let r = deno_pub(ab);
  assert(r == null);
  r = deno_pub(ab);
  assert(r == null);
}

function pubReturnBar() {
  const ui8 = new Uint8Array("abc".split("").map(c => c.charCodeAt(0)));
  const ab = typedArrayToArrayBuffer(ui8);
  const r = deno_pub(ab);
  assert(r instanceof ArrayBuffer);
  assert(r.byteLength === 3);
  const rui8 = new Uint8Array(r);
  const rstr = String.fromCharCode(...rui8);
  assert(rstr === "bar");
}
