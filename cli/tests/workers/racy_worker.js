// See issue for details
// https://github.com/denoland/deno/issues/4080
//
// After first call to `postMessage() this worker schedules
// [close(), postMessage()] ops on the same turn of microtask queue
// (because message is rather big).
// Only single `postMessage()` call should make it
// to host, ie. after calling `close()` no more code should be run.

setTimeout(() => {
  close();
}, 50);

while (true) {
  await new Promise((done) => {
    setTimeout(() => {
      postMessage({ buf: new Array(999999) });
      done();
    });
  });
}
