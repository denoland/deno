// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/child_process.ts");

export const fork = mod.fork;
export const spawn = mod.spawn;
export const exec = mod.exec;
export const execFile = mod.execFile;
export const execFileSync = mod.execFileSync;
export const execSync = mod.execSync;
export const ChildProcess = mod.ChildProcess;
export const spawnSync = mod.spawnSync;

export default mod;
