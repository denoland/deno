// deno-fmt-ignore-file
// deno-lint-ignore-file

"use strict";

const assert = require("assert/strict");
const worker_threads = require("worker_threads");

assert.equal(BroadcastChannel, worker_threads.BroadcastChannel);
