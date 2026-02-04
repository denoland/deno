// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import { op_node_dup_fd, op_node_get_fd } from "ext:core/ops";

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
    // Verify the resource still exists - it might have been closed.
    // For fd 0, 1, 2 (stdio), the rid equals fd and they're always valid.
    if (fd <= 2) {
      return rid;
    }
    // For other fds, check if the resource is still valid
    try {
      op_node_get_fd(rid);
      return rid;
    } catch {
      // Resource was closed, remove stale entry and create new one
      MapPrototypeDelete(fdMap, fd);
    }
  }
  // The FD is not in the map or the cached resource was stale.
  // This can happen when a raw OS file descriptor is received from
  // another source (e.g. node-pty NAPI, worker_threads postMessage).
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
