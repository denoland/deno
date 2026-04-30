// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Mirrors lib/cluster.js. Node decides at module-load whether to require
// `internal/cluster/primary` or `internal/cluster/child` from
// process.env.NODE_UNIQUE_ID. In Deno, cluster.ts is eagerly imported by
// 01_require.js (before bootstrap delivers env), so we initialize the
// primary side at module load and let `02_init.js` flip to the child side
// via the `__initCluster` callback.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { internals } from "ext:core/mod.js";
import { EventEmitter } from "node:events";
import { init as initPrimary } from "ext:deno_node/internal/cluster/primary.ts";
import { init as initChild } from "ext:deno_node/internal/cluster/child.ts";

const cluster: any = new EventEmitter();
initPrimary(cluster);

internals.__initCluster = (
  uniqueId: string,
  schedPolicyEnv: string | undefined,
) => {
  if (typeof uniqueId !== "string" || uniqueId.length === 0) {
    return;
  }

  initChild(cluster);

  if (typeof schedPolicyEnv === "string" && schedPolicyEnv.length > 0) {
    if (schedPolicyEnv === "rr") {
      cluster.schedulingPolicy = cluster.SCHED_RR;
    } else if (schedPolicyEnv === "none") {
      cluster.schedulingPolicy = cluster.SCHED_NONE;
    } else {
      const n = Number(schedPolicyEnv);
      if (!Number.isNaN(n)) {
        cluster.schedulingPolicy = n;
      }
    }
  }

  cluster._setupWorker();
};

export const isPrimary = cluster.isPrimary;
export const isMaster = cluster.isMaster;
export const isWorker = cluster.isWorker;
export const Worker = cluster.Worker;
export const workers = cluster.workers;
export const settings = cluster.settings;
export const SCHED_NONE = cluster.SCHED_NONE;
export const SCHED_RR = cluster.SCHED_RR;
export const schedulingPolicy = cluster.schedulingPolicy;
export const fork = cluster.fork;
export const disconnect = cluster.disconnect;
export const setupPrimary = cluster.setupPrimary;
export const setupMaster = cluster.setupMaster;
export const worker = cluster.worker;

export default cluster;
