const w1 = new Worker("./039_worker_deno_ns/has_ns.ts");
const w2 = new Worker("./039_worker_deno_ns/no_ns.ts", {
  noDenoNamespace: true
});
let w1MsgCount = 0;
let w2MsgCount = 0;
w1.onmessage = (msg): void => {
  console.log(msg.data);
  w1MsgCount++;
  if (w1MsgCount === 1) {
    w1.postMessage("CONTINUE");
  } else {
    w2.postMessage("START");
  }
};
w2.onmessage = (msg): void => {
  console.log(msg.data);
  w2MsgCount++;
  if (w2MsgCount === 1) {
    w2.postMessage("CONTINUE");
  } else {
    Deno.exit(0);
  }
};
w1.postMessage("START");
