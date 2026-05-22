// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

(function () {
const { core } = globalThis.__bootstrap;

const webStreams = core.loadExtScript("ext:deno_web/06_streams.js");

const transferMode = Symbol("kTransferMode");
const kCloneable = 1;
const kTransferable = 2;
const kDisallowCloneAndTransfer = 4;

const kClone = Symbol.for("nodejs.messaging.kClone");
const kDeserialize = Symbol.for("nodejs.messaging.kDeserialize");
const kTransfer = webStreams.kNodeMessagingTransfer ??
  Symbol.for("nodejs.messaging.kTransfer");
const kTransferList = Symbol.for("nodejs.messaging.kTransferList");

function markTransferMode(obj, cloneable = false, transferable = false) {
  if ((typeof obj !== "object" && typeof obj !== "function") || obj === null) {
    return;
  }
  let mode = kDisallowCloneAndTransfer;
  if (cloneable) mode |= kCloneable;
  if (transferable) mode |= kTransferable;
  obj[transferMode] = mode;
}

function setup() {}

function structuredClone(value, options) {
  return globalThis.structuredClone(value, options);
}

return {
  markTransferMode,
  setup,
  structuredClone,
  kClone,
  kDeserialize,
  kTransfer,
  kTransferList,
};
})();
