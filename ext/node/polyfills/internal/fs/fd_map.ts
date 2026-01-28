// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

const { MapPrototypeGet, MapPrototypeSet, MapPrototypeDelete, SafeMap } =
  primordials;

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
  if (rid === undefined) {
    // For backwards compatibility, if the FD is not in the map,
    // assume the caller is using a RID directly (e.g., stdio FDs).
    return fd;
  }
  return rid;
}

export function unregisterFd(fd: number): void {
  MapPrototypeDelete(fdMap, fd);
}
