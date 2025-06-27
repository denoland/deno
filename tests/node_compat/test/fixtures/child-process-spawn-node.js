const assert = require("assert");
const debug = require('util').debuglog('test');
const process = require("process");

function onmessage(m) {
  debug("CHILD got message:", m);
  assert.ok(m.hello);
  process.removeListener("message", onmessage);
}

process.on("message", onmessage);
// TODO(kt3k): Uncomment the below when the ipc features are ready
// process.send({ foo: 'bar' });
