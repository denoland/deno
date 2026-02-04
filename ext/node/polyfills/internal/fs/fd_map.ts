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
  logToFile('[registerFd] registering fd: ' + fd + ' with rid: ' + rid);
  // Capture stack trace
  try {
    throw new Error('stack trace');
  } catch (e) {
    logToFile('[registerFd] stack: ' + e.stack);
  }
  MapPrototypeSet(fdMap, fd, rid);
}

// Debug logging
const logFile = "/tmp/fdmap-debug.log";
function logToFile(msg) {
  try {
    const encoder = new TextEncoder();
    const data = encoder.encode(new Date().toISOString() + " " + msg + "\n");
    Deno.writeFileSync(logFile, data, { append: true });
  } catch (e) {
    // ignore
  }
}

export function getRid(fd: number): number {
  logToFile('[getRid] called with fd: ' + fd);
  const rid = MapPrototypeGet(fdMap, fd);
  if (rid !== undefined) {
    logToFile('[getRid] found cached rid: ' + rid + ' for fd: ' + fd);
    // Verify the resource still exists - it might have been closed
    // For fd 0, 1, 2 (stdio), the rid equals fd and they're always valid
    if (fd <= 2) {
      return rid;
    }
    // For other fds, check if the resource is still valid
    try {
      const testFd = op_node_get_fd(rid);
      logToFile('[getRid] resource ' + rid + ' still valid, fd: ' + testFd);
      return rid;
    } catch {
      logToFile('[getRid] resource ' + rid + ' is stale, will create new one');
      // Resource was closed, remove stale entry and create new one
      MapPrototypeDelete(fdMap, fd);
    }
  }
  logToFile('[getRid] fd ' + fd + ' not in map (or stale), will create new resource');
  // The FD is not in the map. This can happen when a raw OS file descriptor
  // is received from another thread (e.g. via worker_threads postMessage).
  // OS file descriptors are process-wide, so we can create a local resource
  // by dup'ing the fd. The dup'd fd is independently owned and closeable.
  if (fd >= 3) {
    try {
      logToFile('[getRid] calling op_node_dup_fd(' + fd + ')');
      const newRid = op_node_dup_fd(fd);
      logToFile('[getRid] op_node_dup_fd returned rid: ' + newRid);
      MapPrototypeSet(fdMap, fd, newRid);
      return newRid;
    } catch (e) {
      logToFile('[getRid] op_node_dup_fd failed: ' + e.message);
      // Fall through - fd may not be valid
    }
  }
  logToFile('[getRid] returning fd as rid: ' + fd);
  return fd;
}

export function unregisterFd(fd: number): void {
  logToFile('[unregisterFd] unregistering fd: ' + fd);
  // Capture stack trace
  try {
    throw new Error('stack trace');
  } catch (e) {
    logToFile('[unregisterFd] stack: ' + e.stack);
  }
  MapPrototypeDelete(fdMap, fd);
}
