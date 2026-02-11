// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { op_node_dup_fd } from "ext:core/ops";

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
  if (rid !== undefined) {
    return rid;
  }
  // The FD is not in the map. This can happen when a raw OS file descriptor
  // is received from another thread (e.g. via worker_threads postMessage).
  // OS file descriptors are process-wide, so we can create a local resource
  // by dup'ing the fd. The dup'd fd is independently owned and closeable.
  if (fd >= 3) {
    try {
      const newRid = op_node_dup_fd(fd);
      MapPrototypeSet(fdMap, fd, newRid);
      return newRid;
    } catch {
      // Fall through - fd may not be valid
    }
  }
  return fd;
}

export function unregisterFd(fd: number): void {
  MapPrototypeDelete(fdMap, fd);
}
