// deno-fmt-ignore-file
// deno-lint-ignore-file

'use strict';

const tty = require('tty');
const assert = require('assert');

assert.throws(
  () => new tty.WriteStream(-1),
  {
    code: 'ERR_INVALID_FD',
    name: 'RangeError',
    message: '"fd" must be a positive integer: -1'
  }
);

assert.throws(
  () => new tty.ReadStream(-1),
  {
    code: 'ERR_INVALID_FD',
    name: 'RangeError',
    message: '"fd" must be a positive integer: -1'
  }
);