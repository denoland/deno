// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
/*
SharedQueue Binary Layout
+-------------------------------+-------------------------------+
|                        NUM_RECORDS (32)                       |
+---------------------------------------------------------------+
|                        NUM_SHIFTED_OFF (32)                   |
+---------------------------------------------------------------+
|                        HEAD (32)                              |
+---------------------------------------------------------------+
|                        OFFSETS (32)                           |
+---------------------------------------------------------------+
|                        RECORD_ENDS (*MAX_RECORDS)           ...
+---------------------------------------------------------------+
|                        RECORDS (*MAX_RECORDS)               ...
+---------------------------------------------------------------+
 */

/* eslint-disable @typescript-eslint/no-use-before-define */

(window => {
  if (Deno && Deno.core.maybeInit === undefined) {
    const GLOBAL_NAMESPACE = "Deno";
    const OPS_NAMESPACE = "ops";
    const CORE_NAMESPACE = "core";
    const MAX_RECORDS = 100;
    const INDEX_NUM_RECORDS = 0;
    const INDEX_NUM_SHIFTED_OFF = 1;
    const INDEX_HEAD = 2;
    const INDEX_OFFSETS = 3;
    const INDEX_RECORDS = INDEX_OFFSETS + 2 * MAX_RECORDS;
    const HEAD_INIT = 4 * INDEX_RECORDS;

    // Available on start due to bindings.
    const Deno = window[GLOBAL_NAMESPACE];
    const core = Deno[CORE_NAMESPACE];
    // Warning: DO NOT use window.Deno after this point.
    // It is possible that the Deno namespace has been deleted.
    // Use the above local Deno and core variable instead.

    // Async handler registry
    const asyncHandlerMap = [];

    // SharedQueue state
    let sharedBytes;
    let shared32;

    // Op registry state
    let opRecords = {};
    const opListeners = {};

    let initialized = false;

    function maybeInit() {
      if (!initialized) {
        init();
        initialized = true;
      }
    }

    function init() {
      let shared = Deno.core.shared;
      assert(shared.byteLength > 0);
      assert(sharedBytes == null);
      assert(shared32 == null);
      sharedBytes = new Uint8Array(shared);
      shared32 = new Int32Array(shared);
      // Callers should not call Deno.core.recv, use setAsyncHandler.
      Deno.core.recv(handleAsyncMsgFromRust);
      Deno.core.recvOpReg(handleOpUpdate);
    }

    function assert(cond) {
      if (!cond) {
        throw Error("assert");
      }
    }

    function reset() {
      maybeInit();
      shared32[INDEX_NUM_RECORDS] = 0;
      shared32[INDEX_NUM_SHIFTED_OFF] = 0;
      shared32[INDEX_HEAD] = HEAD_INIT;
    }

    function head() {
      maybeInit();
      return shared32[INDEX_HEAD];
    }

    function numRecords() {
      return shared32[INDEX_NUM_RECORDS];
    }

    function size() {
      return shared32[INDEX_NUM_RECORDS] - shared32[INDEX_NUM_SHIFTED_OFF];
    }

    // TODO(ry) rename to setMeta
    function setMeta(index, end, opId) {
      shared32[INDEX_OFFSETS + 2 * index] = end;
      shared32[INDEX_OFFSETS + 2 * index + 1] = opId;
    }

    function getMeta(index) {
      if (index < numRecords()) {
        const buf = shared32[INDEX_OFFSETS + 2 * index];
        const opId = shared32[INDEX_OFFSETS + 2 * index + 1];
        return [opId, buf];
      } else {
        return null;
      }
    }

    function getOffset(index) {
      if (index < numRecords()) {
        if (index == 0) {
          return HEAD_INIT;
        } else {
          return shared32[INDEX_OFFSETS + 2 * (index - 1)];
        }
      } else {
        return null;
      }
    }

    function push(opId, buf) {
      let off = head();
      let end = off + buf.byteLength;
      let index = numRecords();
      if (end > shared32.byteLength || index >= MAX_RECORDS) {
        // console.log("shared_queue.js push fail");
        return false;
      }
      setMeta(index, end, opId);
      assert(end - off == buf.byteLength);
      sharedBytes.set(buf, off);
      shared32[INDEX_NUM_RECORDS] += 1;
      shared32[INDEX_HEAD] = end;
      return true;
    }

    /// Returns null if empty.
    function shift() {
      let i = shared32[INDEX_NUM_SHIFTED_OFF];
      if (size() == 0) {
        assert(i == 0);
        return null;
      }

      const off = getOffset(i);
      const [opId, end] = getMeta(i);

      if (size() > 1) {
        shared32[INDEX_NUM_SHIFTED_OFF] += 1;
      } else {
        reset();
      }

      assert(off != null);
      assert(end != null);
      const buf = sharedBytes.subarray(off, end);
      return [opId, buf];
    }

    function setAsyncHandler(opId, cb) {
      maybeInit();
      asyncHandlerMap[opId] = cb;
    }

    function handleAsyncMsgFromRust(opId, buf) {
      if (buf) {
        // This is the overflow_response case of deno::Isolate::poll().
        asyncHandlerMap[opId](buf);
      } else {
        while (true) {
          let opIdBuf = shift();
          if (opIdBuf == null) {
            break;
          }
          asyncHandlerMap[opIdBuf[0]](opIdBuf[1]);
        }
      }
    }

    function dispatch(opId, control, zeroCopy = null) {
      maybeInit();
      return Deno.core.send(opId, control, zeroCopy);
    }

    // Op registry handlers

    function handleOpUpdate(opId, namespace, name) {
      // If we recieve a call with no params reset opRecords
      if (opId === undefined) {
        resetOps();
        return;
      }
      registerOp(opId, namespace, name);
    }

    function resetOps() {
      // Reset records
      opRecords = {};
      // Call all listeners with undefined
      for (const space in opListeners) {
        for (const name in opListeners[space]) {
          for (const listener of opListeners[space][name]) {
            listener(undefined);
          }
        }
      }
    }

    function registerOp(opId, namespace, name) {
      // Ensure namespace exists in object
      if (opRecords[namespace] === undefined) {
        opRecords[namespace] = {};
      }
      // Set record to opId
      opRecords[namespace][name] = opId;
      // Check for listeners
      if (opListeners[namespace] !== undefined) {
        if (opListeners[namespace][name] !== undefined) {
          // Call all listeners with new id
          for (const listener of opListeners[namespace][name]) {
            listener(opId);
          }
        }
      }
    }

    // Op registry external backplane
    // (stuff that makes the external interface work)
    // This part relies on Proxy for custom index handling. I would normally
    // avoid Proxy, but I don't think there is a preferable way to do this.

    // TODO(afinch7) maybe implement enumerablity, and some others functions?

    const namespaceHandler = {
      get: (target, prop, _receiver) => {
        if (typeof prop === "symbol") {
          throw new TypeError("symbol isn't a valid index");
        }
        // If namespace exists return value of op from namespace (maybe undefined)
        if (target.root.ops[target.namespace]) {
          return target.root.ops[target.namespace][prop];
        }
        // Otherwise return undefined
        return undefined;
      },
      set: (target, prop, value, _receiver) => {
        if (typeof prop === "symbol") {
          throw new TypeError("symbol isn't a valid index");
        }
        // Init namespace if not present
        if (target.root.listeners[target.namespace] === undefined) {
          target.root.listeners[target.namespace] = {};
        }
        // Init op in namespace if not present
        if (target.root.listeners[target.namespace][prop] === undefined) {
          target.root.listeners[target.namespace][prop] = [];
        }
        // Notify the listener of the current value.
        if (target.root.ops[target.namespace]) {
          value(target.root.ops[target.namespace][prop]);
        }
        // Push our new listener
        target.root.listeners[target.namespace][prop].push(value);
        return true;
      }
    };

    const rootHandler = {
      get: (target, prop, _receiver) => {
        const namespaceObject = {
          root: target,
          namespace: prop.toString()
        };

        const namespaceProxy = new Proxy(namespaceObject, namespaceHandler);

        return namespaceProxy;
      }
    };

    const registryRootObject = {
      // This needs to be a accessor since opRecords is let
      get ops() {
        return opRecords;
      },
      get listeners() {
        return opListeners;
      }
    };

    const registryProxy = new Proxy(registryRootObject, rootHandler);

    Object.seal(registryProxy);

    const denoCore = {
      setAsyncHandler,
      dispatch,
      maybeInit,
      sharedQueue: {
        MAX_RECORDS,
        head,
        numRecords,
        size,
        push,
        reset,
        shift
      }
    };

    assert(window[GLOBAL_NAMESPACE] != null);
    assert(window[GLOBAL_NAMESPACE][CORE_NAMESPACE] != null);
    assert(window[GLOBAL_NAMESPACE][OPS_NAMESPACE] != null);
    Object.assign(core, denoCore);
    Deno[OPS_NAMESPACE] = registryProxy;
  }
})(this);
