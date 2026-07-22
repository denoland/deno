// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/cluster.ts");

export const Worker = mod.Worker;
export const SCHED_NONE = mod.SCHED_NONE;
export const SCHED_RR = mod.SCHED_RR;
export const isPrimary = mod.isPrimary;
export const isMaster = mod.isMaster;
export const isWorker = mod.isWorker;
export const workers = mod.workers;
export const settings = mod.settings;
export const schedulingPolicy = mod.schedulingPolicy;
export const fork = mod.fork;
export const disconnect = mod.disconnect;
export const setupPrimary = mod.setupPrimary;
export const setupMaster = mod.setupMaster;
export const worker = mod.worker;

export default mod.default;
