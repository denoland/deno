// A simple runtime that doesn't involve typescript or protobufs to test
// libdeno.
const globalEval = eval;
const window = globalEval("this");
window['foo'] = () => {
  deno_print("Hello world from foo");
  return "foo";
}

function assert(cond) {
  if (!cond) throw Error("assert failed");
}

function recvabc() {
  deno_recv((msg) => {
    assert(msg instanceof ArrayBuffer);
    assert(msg.byteLength === 3);
  });
}
