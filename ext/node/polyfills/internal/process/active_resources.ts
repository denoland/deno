// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { primordials } = globalThis.__bootstrap;
const { ArrayPrototypePush, SafeSet, SafeSetIterator } = primordials;

const activeRequests = new SafeSet();
const activeHandles = new SafeSet();

class FSReqCallback {}

function snapshot(set: Set<any>) {
  const resources = [];
  for (const resource of new SafeSetIterator(set)) {
    ArrayPrototypePush(resources, resource);
  }
  return resources;
}

function registerActiveRequest(request: any) {
  activeRequests.add(request);
  return request;
}

function unregisterActiveRequest(request: any) {
  activeRequests.delete(request);
}

function createFSReqCallback() {
  return registerActiveRequest(new FSReqCallback());
}

function getActiveRequests() {
  return snapshot(activeRequests);
}

function registerActiveHandle(handle: any) {
  activeHandles.add(handle);
  return handle;
}

function unregisterActiveHandle(handle: any) {
  activeHandles.delete(handle);
}

function getActiveHandles() {
  return snapshot(activeHandles);
}

return {
  createFSReqCallback,
  getActiveHandles,
  getActiveRequests,
  registerActiveHandle,
  registerActiveRequest,
  unregisterActiveHandle,
  unregisterActiveRequest,
};
})();
