// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

const {
  Error,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  MapPrototypeDelete,
  SafeMap,
} = primordials;

// Maps OS file descriptors to Deno resource IDs.
const fdMap = new SafeMap();

// Pre-populate with stdio (RIDs 0-2 map to FDs 0-2).
MapPrototypeSet(fdMap, 0, 0);
MapPrototypeSet(fdMap, 1, 1);
MapPrototypeSet(fdMap, 2, 2);

export function registerFd(fd: number, rid: number): void {
  MapPrototypeSet(fdMap, fd, rid);
}

export function getRid(fd: number): number {
  const rid = MapPrototypeGet(fdMap, fd);
  if (rid !== undefined) {
    return rid;
  }
  // For stdio fds (0-2), return as-is.
  if (fd < 3) {
    return fd;
  }
  throw new Error(
    `File descriptor ${fd} is not open or not managed by node:fs`,
  );
}

export function hasFd(fd: number): boolean {
  return MapPrototypeHas(fdMap, fd);
}

export function unregisterFd(fd: number): void {
  MapPrototypeDelete(fdMap, fd);
}
