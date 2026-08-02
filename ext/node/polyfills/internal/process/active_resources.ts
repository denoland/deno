// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { primordials } = __bootstrap;
const {
  ArrayPrototypePush,
  SafeSet,
  SafeSetIterator,
  SafeWeakMap,
  WeakMapPrototypeDelete,
  WeakMapPrototypeGet,
  WeakMapPrototypeSet,
} = primordials;

const activeRequests = new SafeSet();
const activeHandles = new SafeSet();
// Maps a tracked resource to the libuv-style wrap name Node reports from
// `process.getActiveResourcesInfo()` (e.g. "TCPSocketWrap"). The role (server
// vs. socket) isn't recoverable from the handle alone, so callers tag it here.
const resourceNames = new SafeWeakMap();

class FSReqCallback {}

function snapshot(set: Set<any>) {
  const resources = [];
  for (const resource of new SafeSetIterator(set)) {
    ArrayPrototypePush(resources, resource);
  }
  return resources;
}

function resourceTypeName(resource: any) {
  const name = WeakMapPrototypeGet(resourceNames, resource);
  if (name !== undefined) {
    return name;
  }
  const ctor = resource?.constructor;
  return (ctor && ctor.name) || "Unknown";
}

function registerActiveRequest(request: any, name?: string) {
  activeRequests.add(request);
  if (name !== undefined) {
    WeakMapPrototypeSet(resourceNames, request, name);
  }
  return request;
}

function unregisterActiveRequest(request: any) {
  activeRequests.delete(request);
  WeakMapPrototypeDelete(resourceNames, request);
}

function createFSReqCallback() {
  return registerActiveRequest(new FSReqCallback(), "FSReqCallback");
}

function getActiveRequests() {
  return snapshot(activeRequests);
}

function registerActiveHandle(handle: any, name?: string) {
  activeHandles.add(handle);
  if (name !== undefined) {
    WeakMapPrototypeSet(resourceNames, handle, name);
  }
  return handle;
}

function unregisterActiveHandle(handle: any) {
  activeHandles.delete(handle);
  WeakMapPrototypeDelete(resourceNames, handle);
}

function getActiveHandles() {
  return snapshot(activeHandles);
}

// The wrap names of every tracked handle and request, for
// `process.getActiveResourcesInfo()`.
function getActiveResourceNames() {
  const names: string[] = [];
  for (const handle of new SafeSetIterator(activeHandles)) {
    ArrayPrototypePush(names, resourceTypeName(handle));
  }
  for (const request of new SafeSetIterator(activeRequests)) {
    ArrayPrototypePush(names, resourceTypeName(request));
  }
  return names;
}

return {
  createFSReqCallback,
  getActiveHandles,
  getActiveRequests,
  getActiveResourceNames,
  registerActiveHandle,
  registerActiveRequest,
  unregisterActiveHandle,
  unregisterActiveRequest,
};
})();
