// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";

/** A Worker object contains all public information and method about a worker.
 * In the primary it can be obtained using cluster.workers. In a worker it can
 * be obtained using cluster.worker.
 */
export class Worker {
  constructor() {
    notImplemented("cluster.Worker.prototype.constructor");
  }
}
/** Calls .disconnect() on each worker in cluster.workers. */
export function disconnected() {
  notImplemented("cluster.disconnected");
}
/** Spawn a new worker process. */
export function fork() {
  notImplemented("cluster.fork");
}
/** True if the process is a primary. This is determined by
 * the process.env.NODE_UNIQUE_ID. If process.env.NODE_UNIQUE_ID is undefined,
 * then isPrimary is true. */
export const isPrimary = undefined;
/** True if the process is not a primary (it is the negation of
 * cluster.isPrimary). */
export const isWorker = undefined;
/** Deprecated alias for cluster.isPrimary. details. */
export const isMaster = isPrimary;
/** The scheduling policy, either cluster.SCHED_RR for round-robin or
 * cluster.SCHED_NONE to leave it to the operating system. This is a global
 * setting and effectively frozen once either the first worker is spawned, or
 * .setupPrimary() is called, whichever comes first. */
export const schedulingPolicy = undefined;
/** The settings object */
export const settings = undefined;
/** Deprecated alias for .setupPrimary(). */
export function setupMaster() {
  notImplemented("cluster.setupMaster");
}
/** setupPrimary is used to change the default 'fork' behavior. Once called,
 * the settings will be present in cluster.settings. */
export function setupPrimary() {
  notImplemented("cluster.setupPrimary");
}
/** A reference to the current worker object. Not available in the primary
 * process. */
export const worker = undefined;
/** A hash that stores the active worker objects, keyed by id field. Makes it
 * easy to loop through all the workers. It is only available in the primary
 * process. */
export const workers = undefined;

export default {
  Worker,
  disconnected,
  fork,
  isPrimary,
  isWorker,
  isMaster,
  schedulingPolicy,
  settings,
  setupMaster,
  setupPrimary,
  worker,
  workers,
};
