// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core } from "ext:core/mod.js";

const webStreams = core.loadExtScript("ext:deno_web/06_streams.js");

const transferMode = Symbol("kTransferMode");
const kCloneable = 1;
const kTransferable = 2;
const kDisallowCloneAndTransfer = 4;

export const kClone = Symbol.for("nodejs.messaging.kClone");
export const kDeserialize = Symbol.for("nodejs.messaging.kDeserialize");
export const kTransfer = webStreams.kNodeMessagingTransfer ??
  Symbol.for("nodejs.messaging.kTransfer");
export const kTransferList = Symbol.for("nodejs.messaging.kTransferList");

export function markTransferMode(obj, cloneable = false, transferable = false) {
  if ((typeof obj !== "object" && typeof obj !== "function") || obj === null) {
    return;
  }
  let mode = kDisallowCloneAndTransfer;
  if (cloneable) mode |= kCloneable;
  if (transferable) mode |= kTransferable;
  obj[transferMode] = mode;
}

export function setup() {}

export function structuredClone(value, options) {
  return globalThis.structuredClone(value, options);
}

export default {
  markTransferMode,
  setup,
  structuredClone,
  kClone,
  kDeserialize,
  kTransfer,
  kTransferList,
};
