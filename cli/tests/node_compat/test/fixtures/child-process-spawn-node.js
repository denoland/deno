const assert = require('assert');
// TODO(kt3k): Uncomment this when util.debuglog is added
// const debug = require('util').debuglog('test');
const debug = console.log;

function onmessage(m) {
  debug('CHILD got message:', m);
  assert.ok(m.hello);
  process.removeListener('message', onmessage);
}

process.on('message', onmessage);
// TODO(kt3k): Uncomment the below when the ipc features are ready
// process.send({ foo: 'bar' });
