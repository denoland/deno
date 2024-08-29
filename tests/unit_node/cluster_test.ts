// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "@std/assert";
import cluster from "node:cluster";
import * as clusterNamed from "node:cluster";

Deno.test("[node/cluster] has all node exports", () => {
  assertEquals(cluster.isPrimary, true);
  assertEquals(cluster.isMaster, true);
  assertEquals(cluster.isWorker, false);
  assertEquals(typeof cluster.disconnect, "function");
  assertEquals(typeof cluster.on, "function");
  assertEquals(cluster.workers, {});
  assertEquals(cluster.settings, {});
  assertEquals(cluster.SCHED_NONE, 1);
  assertEquals(cluster.SCHED_RR, 2);
  assertEquals(typeof cluster.fork, "function");
  assertEquals(typeof cluster.disconnect, "function");
  assertEquals(typeof cluster.setupPrimary, "function");
  assertEquals(cluster.setupPrimary, cluster.setupMaster);

  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.setupPrimary, clusterNamed.setupPrimary);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.setupMaster, clusterNamed.setupMaster);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.workers, clusterNamed.workers);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.settings, clusterNamed.settings);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.fork, clusterNamed.fork);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.disconnect, clusterNamed.disconnect);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.SCHED_NONE, clusterNamed.SCHED_NONE);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.SCHED_RR, clusterNamed.SCHED_RR);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.isWorker, clusterNamed.isWorker);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.isPrimary, clusterNamed.isPrimary);
  // @ts-ignore Our @types/node version is too old
  assertEquals(cluster.isMaster, clusterNamed.isMaster);
});
