// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/vfs.ts");

export const {
  create,
  VirtualFileSystem,
  VirtualProvider,
  MemoryProvider,
  VirtualFileHandle,
  MemoryFileHandle,
  VirtualStats,
  VirtualDirent,
} = mod;

export default mod.default;
