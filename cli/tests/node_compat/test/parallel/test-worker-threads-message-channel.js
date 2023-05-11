// deno-fmt-ignore-file
// deno-lint-ignore-file

"use strict";

const assert = require("assert/strict");
const worker_threads = require("worker_threads");

assert.equal(MessageChannel, worker_threads.MessageChannel);
assert.equal(MessagePort, worker_threads.MessagePort);
