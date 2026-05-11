// Copyright 2018-2026 the Deno authors. MIT license.
import { core, internals } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/worker_threads.ts");

export const {
  BroadcastChannel,
  MessagePort,
  MessageChannel,
  Worker,
  markAsUntransferable,
  moveMessagePortToContext,
  receiveMessageOnPort,
  getEnvironmentData,
  setEnvironmentData,
  SHARE_ENV,
} = mod;

// These are populated by `internals.__initWorkerThreads()` after Node bootstrap
// runs. Use `let` exports so consumers' ESM live bindings see the updated
// values once `__refreshWorkerThreadsWrapper` syncs them.
export let parentPort = mod.parentPort;
export let threadId = mod.threadId;
export let workerData = mod.workerData;
export let isMainThread = mod.isMainThread;
export let resourceLimits = mod.resourceLimits;

internals.__refreshWorkerThreadsWrapper = () => {
  parentPort = mod.parentPort;
  threadId = mod.threadId;
  workerData = mod.workerData;
  isMainThread = mod.isMainThread;
  resourceLimits = mod.resourceLimits;
};

export default mod;
