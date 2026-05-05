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

// ESM exports are live bindings: importers see the current value of the
// local `let`. We sync from `cluster.*` after `initPrimary` (now), and
// re-sync after `initChild` runs in `__initCluster` so workers don't see
// stale primary-side values like `isWorker = false` or a primary-side
// `fork` that was overwritten by the child-side init.
let isPrimary = cluster.isPrimary;
let isMaster = cluster.isMaster;
let isWorker = cluster.isWorker;
let workers = cluster.workers;
let settings = cluster.settings;
let schedulingPolicy = cluster.schedulingPolicy;
let fork = cluster.fork;
let disconnect = cluster.disconnect;
let setupPrimary = cluster.setupPrimary;
let setupMaster = cluster.setupMaster;
let worker = cluster.worker;

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

  // Resync the live bindings now that the child side has populated `cluster`
  // (including `cluster.worker` set by `_setupWorker`).
  isPrimary = cluster.isPrimary;
  isMaster = cluster.isMaster;
  isWorker = cluster.isWorker;
  workers = cluster.workers;
  settings = cluster.settings;
  schedulingPolicy = cluster.schedulingPolicy;
  fork = cluster.fork;
  disconnect = cluster.disconnect;
  setupPrimary = cluster.setupPrimary;
  setupMaster = cluster.setupMaster;
  worker = cluster.worker;
};

// Stable across primary/child: `Worker`, `SCHED_NONE`, `SCHED_RR` are set by
// both init paths to the same values.
export const Worker = cluster.Worker;
export const SCHED_NONE = cluster.SCHED_NONE;
export const SCHED_RR = cluster.SCHED_RR;

export {
  disconnect,
  fork,
  isMaster,
  isPrimary,
  isWorker,
  schedulingPolicy,
  settings,
  setupMaster,
  setupPrimary,
  worker,
  workers,
};

export default cluster;
