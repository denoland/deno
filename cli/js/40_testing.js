// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
import { setExitHandler } from "ext:runtime/30_os.js";
import { Console } from "ext:deno_console/02_console.js";
import { serializePermissions } from "ext:runtime/10_permissions.js";
import { assert } from "ext:deno_web/00_infra.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  DateNow,
  Error,
  FunctionPrototype,
  Map,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  MathCeil,
  ObjectKeys,
  ObjectPrototypeHasOwnProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  SafeArrayIterator,
  Set,
  SymbolToStringTag,
  TypeError,
} = primordials;

const opSanitizerDelayResolveQueue = [];
let hasSetOpSanitizerDelayMacrotask = false;

// Even if every resource is closed by the end of a test, there can be a delay
// until the pending ops have all finished. This function returns a promise
// that resolves when it's (probably) fine to run the op sanitizer.
//
// This is implemented by adding a macrotask callback that runs after the
// timer macrotasks, so we can guarantee that a currently running interval
// will have an associated op. An additional `setTimeout` of 0 is needed
// before that, though, in order to give time for worker message ops to finish
// (since timeouts of 0 don't queue tasks in the timer queue immediately).
function opSanitizerDelay() {
  if (!hasSetOpSanitizerDelayMacrotask) {
    core.setMacrotaskCallback(handleOpSanitizerDelayMacrotask);
    hasSetOpSanitizerDelayMacrotask = true;
  }
  return new Promise((resolve) => {
    setTimeout(() => {
      ArrayPrototypePush(opSanitizerDelayResolveQueue, resolve);
    }, 0);
  });
}

function handleOpSanitizerDelayMacrotask() {
  ArrayPrototypeShift(opSanitizerDelayResolveQueue)?.();
  return opSanitizerDelayResolveQueue.length === 0;
}

// An async operation to $0 was started in this test, but never completed. This is often caused by not $1.
// An async operation to $0 was started in this test, but never completed. Async operations should not complete in a test if they were not started in that test.
// deno-fmt-ignore
const OP_DETAILS = {
  "op_blob_read_part": ["read from a Blob or File", "awaiting the result of a Blob or File read"],
  "op_broadcast_recv": ["receive a message from a BroadcastChannel", "closing the BroadcastChannel"],
  "op_broadcast_send": ["send a message to a BroadcastChannel", "closing the BroadcastChannel"],
  "op_chmod_async": ["change the permissions of a file", "awaiting the result of a `Deno.chmod` call"],
  "op_chown_async": ["change the owner of a file", "awaiting the result of a `Deno.chown` call"],
  "op_copy_file_async": ["copy a file", "awaiting the result of a `Deno.copyFile` call"],
  "op_crypto_decrypt": ["decrypt data", "awaiting the result of a `crypto.subtle.decrypt` call"],
  "op_crypto_derive_bits": ["derive bits from a key", "awaiting the result of a `crypto.subtle.deriveBits` call"],
  "op_crypto_encrypt": ["encrypt data", "awaiting the result of a `crypto.subtle.encrypt` call"],
  "op_crypto_generate_key": ["generate a key", "awaiting the result of a `crypto.subtle.generateKey` call"],
  "op_crypto_sign_key": ["sign data", "awaiting the result of a `crypto.subtle.sign` call"],
  "op_crypto_subtle_digest": ["digest data", "awaiting the result of a `crypto.subtle.digest` call"],
  "op_crypto_verify_key": ["verify data", "awaiting the result of a `crypto.subtle.verify` call"],
  "op_net_recv_udp": ["receive a datagram message via UDP", "awaiting the result of `Deno.DatagramConn#receive` call, or not breaking out of a for await loop looping over a `Deno.DatagramConn`"],
  "op_net_recv_unixpacket": ["receive a datagram message via Unixpacket", "awaiting the result of `Deno.DatagramConn#receive` call, or not breaking out of a for await loop looping over a `Deno.DatagramConn`"],
  "op_net_send_udp": ["send a datagram message via UDP", "awaiting the result of `Deno.DatagramConn#send` call"],
  "op_net_send_unixpacket": ["send a datagram message via Unixpacket", "awaiting the result of `Deno.DatagramConn#send` call"],
  "op_dns_resolve": ["resolve a DNS name", "awaiting the result of a `Deno.resolveDns` call"],
  "op_fdatasync_async": ["flush pending data operations for a file to disk", "awaiting the result of a `Deno.fdatasync` call"],
  "op_fetch_send": ["send a HTTP request", "awaiting the result of a `fetch` call"],
  "op_ffi_call_nonblocking": ["do a non blocking ffi call", "awaiting the returned promise"] ,
  "op_ffi_call_ptr_nonblocking": ["do a non blocking ffi call",  "awaiting the returned promise"],
  "op_flock_async": ["lock a file", "awaiting the result of a `Deno.flock` call"],
  "op_fs_events_poll": ["get the next file system event", "breaking out of a for await loop looping over `Deno.FsEvents`"],
  "op_fstat_async": ["get file metadata", "awaiting the result of a `Deno.File#fstat` call"],
  "op_fsync_async": ["flush pending data operations for a file to disk", "awaiting the result of a `Deno.fsync` call"],
  "op_ftruncate_async": ["truncate a file", "awaiting the result of a `Deno.ftruncate` call"],
  "op_funlock_async": ["unlock a file", "awaiting the result of a `Deno.funlock` call"],
  "op_futime_async": ["change file timestamps", "awaiting the result of a `Deno.futime` call"],
  "op_http_accept": ["accept a HTTP request", "closing a `Deno.HttpConn`"],
  "op_http_shutdown": ["shutdown a HTTP connection", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_upgrade_websocket": ["upgrade a HTTP connection to a WebSocket", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_write_headers": ["write HTTP response headers", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_write": ["write HTTP response body", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_link_async": ["create a hard link", "awaiting the result of a `Deno.link` call"],
  "op_make_temp_dir_async": ["create a temporary directory", "awaiting the result of a `Deno.makeTempDir` call"],
  "op_make_temp_file_async": ["create a temporary file", "awaiting the result of a `Deno.makeTempFile` call"],
  "op_message_port_recv_message": ["receive a message from a MessagePort", "awaiting the result of not closing a `MessagePort`"],
  "op_mkdir_async": ["create a directory", "awaiting the result of a `Deno.mkdir` call"],
  "op_net_accept_tcp": ["accept a TCP stream", "closing a `Deno.Listener`"],
  "op_net_accept_unix": ["accept a Unix stream", "closing a `Deno.Listener`"],
  "op_net_connect_tcp": ["connect to a TCP server", "awaiting a `Deno.connect` call"],
  "op_net_connect_unix": ["connect to a Unix server", "awaiting a `Deno.connect` call"],
  "op_open_async": ["open a file", "awaiting the result of a `Deno.open` call"],
  "op_read_dir_async": ["read a directory", "collecting all items in the async iterable returned from a `Deno.readDir` call"],
  "op_read_link_async": ["read a symlink", "awaiting the result of a `Deno.readLink` call"],
  "op_realpath_async": ["resolve a path", "awaiting the result of a `Deno.realpath` call"],
  "op_remove_async": ["remove a file or directory", "awaiting the result of a `Deno.remove` call"],
  "op_rename_async": ["rename a file or directory", "awaiting the result of a `Deno.rename` call"],
  "op_run_status": ["get the status of a subprocess", "awaiting the result of a `Deno.Process#status` call"],
  "op_seek_async": ["seek in a file", "awaiting the result of a `Deno.File#seek` call"],
  "op_signal_poll": ["get the next signal", "un-registering a OS signal handler"],
  "op_sleep": ["sleep for a duration", "cancelling a `setTimeout` or `setInterval` call"],
  "op_stat_async": ["get file metadata", "awaiting the result of a `Deno.stat` call"],
  "op_symlink_async": ["create a symlink", "awaiting the result of a `Deno.symlink` call"],
  "op_net_accept_tls": ["accept a TLS stream", "closing a `Deno.TlsListener`"],
  "op_net_connect_tls": ["connect to a TLS server", "awaiting a `Deno.connectTls` call"],
  "op_tls_handshake": ["perform a TLS handshake", "awaiting a `Deno.TlsConn#handshake` call"],
  "op_tls_start": ["start a TLS connection", "awaiting a `Deno.startTls` call"],
  "op_truncate_async": ["truncate a file", "awaiting the result of a `Deno.truncate` call"],
  "op_utime_async": ["change file timestamps", "awaiting the result of a `Deno.utime` call"],
  "op_worker_recv_message":  ["receive a message from a web worker", "terminating a `Worker`"],
  "op_ws_close": ["close a WebSocket", "awaiting until the `close` event is emitted on a `WebSocket`, or the `WebSocketStream#closed` promise resolves"],
  "op_ws_create": ["create a WebSocket", "awaiting until the `open` event is emitted on a `WebSocket`, or the result of a `WebSocketStream#connection` promise"],
  "op_ws_next_event": ["receive the next message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_ws_send": ["send a message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
};

// Wrap test function in additional assertion that makes sure
// the test case does not leak async "ops" - ie. number of async
// completed ops after the test is the same as number of dispatched
// ops. Note that "unref" ops are ignored since in nature that are
// optional.
function assertOps(fn) {
  /** @param desc {TestDescription | TestStepDescription} */
  return async function asyncOpSanitizer(desc) {
    const pre = core.metrics();
    const preTraces = new Map(core.opCallTraces);
    try {
      const innerResult = await fn(desc);
      if (innerResult) return innerResult;
    } finally {
      // Defer until next event loop turn - that way timeouts and intervals
      // cleared can actually be removed from resource table, otherwise
      // false positives may occur (https://github.com/denoland/deno/issues/4591)
      await opSanitizerDelay();
      await opSanitizerDelay();
    }
    const post = core.metrics();
    const postTraces = new Map(core.opCallTraces);

    // We're checking diff because one might spawn HTTP server in the background
    // that will be a pending async op before test starts.
    const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
    const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;

    if (dispatchedDiff === completedDiff) return null;

    const details = [];
    for (const key in post.ops) {
      if (!ObjectPrototypeHasOwnProperty(post.ops, key)) {
        continue;
      }
      const preOp = pre.ops[key] ??
        { opsDispatchedAsync: 0, opsCompletedAsync: 0 };
      const postOp = post.ops[key];
      const dispatchedDiff = postOp.opsDispatchedAsync -
        preOp.opsDispatchedAsync;
      const completedDiff = postOp.opsCompletedAsync -
        preOp.opsCompletedAsync;

      if (dispatchedDiff > completedDiff) {
        const [name, hint] = OP_DETAILS[key] || [key, null];
        const count = dispatchedDiff - completedDiff;
        let message = `${count} async operation${
          count === 1 ? "" : "s"
        } to ${name} ${
          count === 1 ? "was" : "were"
        } started in this test, but never completed.`;
        if (hint) {
          message += ` This is often caused by not ${hint}.`;
        }
        const traces = [];
        for (const [id, { opName, stack }] of postTraces) {
          if (opName !== key) continue;
          if (MapPrototypeHas(preTraces, id)) continue;
          ArrayPrototypePush(traces, stack);
        }
        if (traces.length === 1) {
          message += " The operation was started here:\n";
          message += traces[0];
        } else if (traces.length > 1) {
          message += " The operations were started here:\n";
          message += ArrayPrototypeJoin(traces, "\n\n");
        }
        ArrayPrototypePush(details, message);
      } else if (dispatchedDiff < completedDiff) {
        const [name, hint] = OP_DETAILS[key] || [key, null];
        const count = completedDiff - dispatchedDiff;
        ArrayPrototypePush(
          details,
          `${count} async operation${count === 1 ? "" : "s"} to ${name} ${
            count === 1 ? "was" : "were"
          } started before this test, but ${
            count === 1 ? "was" : "were"
          } completed during the test. Async operations should not complete in a test if they were not started in that test.
            ${hint ? `This is often caused by not ${hint}.` : ""}`,
        );
      }
    }
    return { failed: { leakedOps: [details, core.isOpCallTracingEnabled()] } };
  };
}

function prettyResourceNames(name) {
  switch (name) {
    case "fsFile":
      return ["A file", "opened", "closed"];
    case "fetchRequest":
      return ["A fetch request", "started", "finished"];
    case "fetchRequestBody":
      return ["A fetch request body", "created", "closed"];
    case "fetchResponseBody":
      return ["A fetch response body", "created", "consumed"];
    case "httpClient":
      return ["An HTTP client", "created", "closed"];
    case "dynamicLibrary":
      return ["A dynamic library", "loaded", "unloaded"];
    case "httpConn":
      return ["An inbound HTTP connection", "accepted", "closed"];
    case "httpStream":
      return ["An inbound HTTP request", "accepted", "closed"];
    case "tcpStream":
      return ["A TCP connection", "opened/accepted", "closed"];
    case "unixStream":
      return ["A Unix connection", "opened/accepted", "closed"];
    case "tlsStream":
      return ["A TLS connection", "opened/accepted", "closed"];
    case "tlsListener":
      return ["A TLS listener", "opened", "closed"];
    case "unixListener":
      return ["A Unix listener", "opened", "closed"];
    case "unixDatagram":
      return ["A Unix datagram", "opened", "closed"];
    case "tcpListener":
      return ["A TCP listener", "opened", "closed"];
    case "udpSocket":
      return ["A UDP socket", "opened", "closed"];
    case "timer":
      return ["A timer", "started", "fired/cleared"];
    case "textDecoder":
      return ["A text decoder", "created", "finished"];
    case "messagePort":
      return ["A message port", "created", "closed"];
    case "webSocketStream":
      return ["A WebSocket", "opened", "closed"];
    case "fsEvents":
      return ["A file system watcher", "created", "closed"];
    case "childStdin":
      return ["A child process stdin", "opened", "closed"];
    case "childStdout":
      return ["A child process stdout", "opened", "closed"];
    case "childStderr":
      return ["A child process stderr", "opened", "closed"];
    case "child":
      return ["A child process", "started", "closed"];
    case "signal":
      return ["A signal listener", "created", "fired/cleared"];
    case "stdin":
      return ["The stdin pipe", "opened", "closed"];
    case "stdout":
      return ["The stdout pipe", "opened", "closed"];
    case "stderr":
      return ["The stderr pipe", "opened", "closed"];
    case "compression":
      return ["A CompressionStream", "created", "closed"];
    default:
      return [`A "${name}" resource`, "created", "cleaned up"];
  }
}

function resourceCloseHint(name) {
  switch (name) {
    case "fsFile":
      return "Close the file handle by calling `file.close()`.";
    case "fetchRequest":
      return "Await the promise returned from `fetch()` or abort the fetch with an abort signal.";
    case "fetchRequestBody":
      return "Terminate the request body `ReadableStream` by closing or erroring it.";
    case "fetchResponseBody":
      return "Consume or close the response body `ReadableStream`, e.g `await resp.text()` or `await resp.body.cancel()`.";
    case "httpClient":
      return "Close the HTTP client by calling `httpClient.close()`.";
    case "dynamicLibrary":
      return "Unload the dynamic library by calling `dynamicLibrary.close()`.";
    case "httpConn":
      return "Close the inbound HTTP connection by calling `httpConn.close()`.";
    case "httpStream":
      return "Close the inbound HTTP request by responding with `e.respondWith()` or closing the HTTP connection.";
    case "tcpStream":
      return "Close the TCP connection by calling `tcpConn.close()`.";
    case "unixStream":
      return "Close the Unix socket connection by calling `unixConn.close()`.";
    case "tlsStream":
      return "Close the TLS connection by calling `tlsConn.close()`.";
    case "tlsListener":
      return "Close the TLS listener by calling `tlsListener.close()`.";
    case "unixListener":
      return "Close the Unix socket listener by calling `unixListener.close()`.";
    case "unixDatagram":
      return "Close the Unix datagram socket by calling `unixDatagram.close()`.";
    case "tcpListener":
      return "Close the TCP listener by calling `tcpListener.close()`.";
    case "udpSocket":
      return "Close the UDP socket by calling `udpSocket.close()`.";
    case "timer":
      return "Clear the timer by calling `clearInterval` or `clearTimeout`.";
    case "textDecoder":
      return "Close the text decoder by calling `textDecoder.decode('')` or `await textDecoderStream.readable.cancel()`.";
    case "messagePort":
      return "Close the message port by calling `messagePort.close()`.";
    case "webSocketStream":
      return "Close the WebSocket by calling `webSocket.close()`.";
    case "fsEvents":
      return "Close the file system watcher by calling `watcher.close()`.";
    case "childStdin":
      return "Close the child process stdin by calling `proc.stdin.close()`.";
    case "childStdout":
      return "Close the child process stdout by calling `proc.stdout.close()` or `await child.stdout.cancel()`.";
    case "childStderr":
      return "Close the child process stderr by calling `proc.stderr.close()` or `await child.stderr.cancel()`.";
    case "child":
      return "Close the child process by calling `proc.kill()` or `proc.close()`.";
    case "signal":
      return "Clear the signal listener by calling `Deno.removeSignalListener`.";
    case "stdin":
      return "Close the stdin pipe by calling `Deno.stdin.close()`.";
    case "stdout":
      return "Close the stdout pipe by calling `Deno.stdout.close()`.";
    case "stderr":
      return "Close the stderr pipe by calling `Deno.stderr.close()`.";
    case "compression":
      return "Close the compression stream by calling `await stream.writable.close()`.";
    default:
      return "Close the resource before the end of the test.";
  }
}

// Wrap test function in additional assertion that makes sure
// the test case does not "leak" resources - ie. resource table after
// the test has exactly the same contents as before the test.
function assertResources(fn) {
  /** @param desc {TestDescription | TestStepDescription} */
  return async function resourceSanitizer(desc) {
    const pre = core.resources();
    const innerResult = await fn(desc);
    if (innerResult) return innerResult;
    const post = core.resources();

    const allResources = new Set([
      ...new SafeArrayIterator(ObjectKeys(pre)),
      ...new SafeArrayIterator(ObjectKeys(post)),
    ]);

    const details = [];
    for (const resource of allResources) {
      const preResource = pre[resource];
      const postResource = post[resource];
      if (preResource === postResource) continue;

      if (preResource === undefined) {
        const [name, action1, action2] = prettyResourceNames(postResource);
        const hint = resourceCloseHint(postResource);
        const detail =
          `${name} (rid ${resource}) was ${action1} during the test, but not ${action2} during the test. ${hint}`;
        ArrayPrototypePush(details, detail);
      } else {
        const [name, action1, action2] = prettyResourceNames(preResource);
        const detail =
          `${name} (rid ${resource}) was ${action1} before the test started, but was ${action2} during the test. Do not close resources in a test that were not created during that test.`;
        ArrayPrototypePush(details, detail);
      }
    }
    if (details.length == 0) {
      return null;
    }
    return { failed: { leakedResources: details } };
  };
}

// Wrap test function in additional assertion that makes sure
// that the test case does not accidentally exit prematurely.
function assertExit(fn, isTest) {
  return async function exitSanitizer(...params) {
    setExitHandler((exitCode) => {
      assert(
        false,
        `${
          isTest ? "Test case" : "Bench"
        } attempted to exit with exit code: ${exitCode}`,
      );
    });

    try {
      const innerResult = await fn(...new SafeArrayIterator(params));
      if (innerResult) return innerResult;
    } finally {
      setExitHandler(null);
    }
  };
}

function wrapOuter(fn, desc) {
  return async function outerWrapped() {
    try {
      if (desc.ignore) {
        return "ignored";
      }
      return await fn(desc) ?? "ok";
    } catch (error) {
      return { failed: { jsError: core.destructureError(error) } };
    } finally {
      const state = MapPrototypeGet(testStates, desc.id);
      for (const childDesc of state.children) {
        stepReportResult(childDesc, { failed: "incomplete" }, 0);
      }
      state.completed = true;
    }
  };
}

function wrapInner(fn) {
  /** @param desc {TestDescription | TestStepDescription} */
  return async function innerWrapped(desc) {
    function getRunningStepDescs() {
      const results = [];
      let childDesc = desc;
      while (childDesc.parent != null) {
        const state = MapPrototypeGet(testStates, childDesc.parent.id);
        for (const siblingDesc of state.children) {
          if (siblingDesc.id == childDesc.id) {
            continue;
          }
          const siblingState = MapPrototypeGet(testStates, siblingDesc.id);
          if (!siblingState.completed) {
            ArrayPrototypePush(results, siblingDesc);
          }
        }
        childDesc = childDesc.parent;
      }
      return results;
    }
    const runningStepDescs = getRunningStepDescs();
    const runningStepDescsWithSanitizers = ArrayPrototypeFilter(
      runningStepDescs,
      (d) => usesSanitizer(d),
    );

    if (runningStepDescsWithSanitizers.length > 0) {
      return {
        failed: {
          overlapsWithSanitizers: runningStepDescsWithSanitizers.map(
            getFullName,
          ),
        },
      };
    }

    if (usesSanitizer(desc) && runningStepDescs.length > 0) {
      return {
        failed: { hasSanitizersAndOverlaps: runningStepDescs.map(getFullName) },
      };
    }
    await fn(MapPrototypeGet(testStates, desc.id).context);
    let failedSteps = 0;
    for (const childDesc of MapPrototypeGet(testStates, desc.id).children) {
      const state = MapPrototypeGet(testStates, childDesc.id);
      if (!state.completed) {
        return { failed: "incompleteSteps" };
      }
      if (state.failed) {
        failedSteps++;
      }
    }
    return failedSteps == 0 ? null : { failed: { failedSteps } };
  };
}

function pledgePermissions(permissions) {
  return ops.op_pledge_test_permissions(
    serializePermissions(permissions),
  );
}

function restorePermissions(token) {
  ops.op_restore_test_permissions(token);
}

function withPermissions(fn, permissions) {
  return async function applyPermissions(...params) {
    const token = pledgePermissions(permissions);

    try {
      return await fn(...new SafeArrayIterator(params));
    } finally {
      restorePermissions(token);
    }
  };
}

/**
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: TestFunction
 *   origin: string,
 *   location: TestLocation,
 *   ignore: boolean,
 *   only: boolean.
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 *   permissions: PermissionOptions,
 * }} TestDescription
 *
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: TestFunction
 *   origin: string,
 *   location: TestLocation,
 *   ignore: boolean,
 *   level: number,
 *   parent: TestDescription | TestStepDescription,
 *   rootId: number,
 *   rootName: String,
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 * }} TestStepDescription
 *
 * @typedef {{
 *   context: TestContext,
 *   children: TestStepDescription[],
 *   completed: boolean,
 * }} TestState
 *
 * @typedef {{
 *   context: TestContext,
 *   children: TestStepDescription[],
 *   completed: boolean,
 *   failed: boolean,
 * }} TestStepState
 *
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: BenchFunction
 *   origin: string,
 *   ignore: boolean,
 *   only: boolean.
 *   sanitizeExit: boolean,
 *   permissions: PermissionOptions,
 * }} BenchDescription
 */

/** @type {Map<number, TestState | TestStepState>} */
const testStates = new Map();

// Main test function provided by Deno.
function test(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
) {
  if (typeof ops.op_register_test != "function") {
    return;
  }

  let testDesc;
  const defaults = {
    ignore: false,
    only: false,
    sanitizeOps: true,
    sanitizeResources: true,
    sanitizeExit: true,
    permissions: null,
  };

  if (typeof nameOrFnOrOptions === "string") {
    if (!nameOrFnOrOptions) {
      throw new TypeError("The test name can't be empty");
    }
    if (typeof optionsOrFn === "function") {
      testDesc = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
    } else {
      if (!maybeFn || typeof maybeFn !== "function") {
        throw new TypeError("Missing test function");
      }
      if (optionsOrFn.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, test function is already provided as the third argument.",
        );
      }
      if (optionsOrFn.name != undefined) {
        throw new TypeError(
          "Unexpected 'name' field in options, test name is already provided as the first argument.",
        );
      }
      testDesc = {
        ...defaults,
        ...optionsOrFn,
        fn: maybeFn,
        name: nameOrFnOrOptions,
      };
    }
  } else if (typeof nameOrFnOrOptions === "function") {
    if (!nameOrFnOrOptions.name) {
      throw new TypeError("The test function must have a name");
    }
    if (optionsOrFn != undefined) {
      throw new TypeError("Unexpected second argument to Deno.test()");
    }
    if (maybeFn != undefined) {
      throw new TypeError("Unexpected third argument to Deno.test()");
    }
    testDesc = {
      ...defaults,
      fn: nameOrFnOrOptions,
      name: nameOrFnOrOptions.name,
    };
  } else {
    let fn;
    let name;
    if (typeof optionsOrFn === "function") {
      fn = optionsOrFn;
      if (nameOrFnOrOptions.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, test function is already provided as the second argument.",
        );
      }
      name = nameOrFnOrOptions.name ?? fn.name;
    } else {
      if (
        !nameOrFnOrOptions.fn || typeof nameOrFnOrOptions.fn !== "function"
      ) {
        throw new TypeError(
          "Expected 'fn' field in the first argument to be a test function.",
        );
      }
      fn = nameOrFnOrOptions.fn;
      name = nameOrFnOrOptions.name ?? fn.name;
    }
    if (!name) {
      throw new TypeError("The test name can't be empty");
    }
    testDesc = { ...defaults, ...nameOrFnOrOptions, fn, name };
  }

  // Delete this prop in case the user passed it. It's used to detect steps.
  delete testDesc.parent;
  const jsError = core.destructureError(new Error());
  testDesc.location = {
    fileName: jsError.frames[1].fileName,
    lineNumber: jsError.frames[1].lineNumber,
    columnNumber: jsError.frames[1].columnNumber,
  };
  testDesc.fn = wrapTest(testDesc);

  const { id, origin } = ops.op_register_test(testDesc);
  testDesc.id = id;
  testDesc.origin = origin;
  MapPrototypeSet(testStates, testDesc.id, {
    context: createTestContext(testDesc),
    children: [],
    completed: false,
  });
}

// Main bench function provided by Deno.
function bench(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
) {
  if (typeof ops.op_register_bench != "function") {
    return;
  }

  let benchDesc;
  const defaults = {
    ignore: false,
    baseline: false,
    only: false,
    sanitizeExit: true,
    permissions: null,
  };

  if (typeof nameOrFnOrOptions === "string") {
    if (!nameOrFnOrOptions) {
      throw new TypeError("The bench name can't be empty");
    }
    if (typeof optionsOrFn === "function") {
      benchDesc = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
    } else {
      if (!maybeFn || typeof maybeFn !== "function") {
        throw new TypeError("Missing bench function");
      }
      if (optionsOrFn.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, bench function is already provided as the third argument.",
        );
      }
      if (optionsOrFn.name != undefined) {
        throw new TypeError(
          "Unexpected 'name' field in options, bench name is already provided as the first argument.",
        );
      }
      benchDesc = {
        ...defaults,
        ...optionsOrFn,
        fn: maybeFn,
        name: nameOrFnOrOptions,
      };
    }
  } else if (typeof nameOrFnOrOptions === "function") {
    if (!nameOrFnOrOptions.name) {
      throw new TypeError("The bench function must have a name");
    }
    if (optionsOrFn != undefined) {
      throw new TypeError("Unexpected second argument to Deno.bench()");
    }
    if (maybeFn != undefined) {
      throw new TypeError("Unexpected third argument to Deno.bench()");
    }
    benchDesc = {
      ...defaults,
      fn: nameOrFnOrOptions,
      name: nameOrFnOrOptions.name,
    };
  } else {
    let fn;
    let name;
    if (typeof optionsOrFn === "function") {
      fn = optionsOrFn;
      if (nameOrFnOrOptions.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, bench function is already provided as the second argument.",
        );
      }
      name = nameOrFnOrOptions.name ?? fn.name;
    } else {
      if (
        !nameOrFnOrOptions.fn || typeof nameOrFnOrOptions.fn !== "function"
      ) {
        throw new TypeError(
          "Expected 'fn' field in the first argument to be a bench function.",
        );
      }
      fn = nameOrFnOrOptions.fn;
      name = nameOrFnOrOptions.name ?? fn.name;
    }
    if (!name) {
      throw new TypeError("The bench name can't be empty");
    }
    benchDesc = { ...defaults, ...nameOrFnOrOptions, fn, name };
  }

  const AsyncFunction = (async () => {}).constructor;
  benchDesc.async = AsyncFunction === benchDesc.fn.constructor;
  benchDesc.fn = wrapBenchmark(benchDesc);

  const { id, origin } = ops.op_register_bench(benchDesc);
  benchDesc.id = id;
  benchDesc.origin = origin;
}

function compareMeasurements(a, b) {
  if (a > b) return 1;
  if (a < b) return -1;

  return 0;
}

function benchStats(n, highPrecision, avg, min, max, all) {
  return {
    n,
    min,
    max,
    p75: all[MathCeil(n * (75 / 100)) - 1],
    p99: all[MathCeil(n * (99 / 100)) - 1],
    p995: all[MathCeil(n * (99.5 / 100)) - 1],
    p999: all[MathCeil(n * (99.9 / 100)) - 1],
    avg: !highPrecision ? (avg / n) : MathCeil(avg / n),
  };
}

async function benchMeasure(timeBudget, fn, async) {
  let n = 0;
  let avg = 0;
  let wavg = 0;
  const all = [];
  let min = Infinity;
  let max = -Infinity;
  const lowPrecisionThresholdInNs = 1e4;

  // warmup step
  let c = 0;
  let iterations = 20;
  let budget = 10 * 1e6;

  if (!async) {
    while (budget > 0 || iterations-- > 0) {
      const t1 = benchNow();

      fn();
      const iterationTime = benchNow() - t1;

      c++;
      wavg += iterationTime;
      budget -= iterationTime;
    }
  } else {
    while (budget > 0 || iterations-- > 0) {
      const t1 = benchNow();

      await fn();
      const iterationTime = benchNow() - t1;

      c++;
      wavg += iterationTime;
      budget -= iterationTime;
    }
  }

  wavg /= c;

  // measure step
  if (wavg > lowPrecisionThresholdInNs) {
    let iterations = 10;
    let budget = timeBudget * 1e6;

    if (!async) {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();

        fn();
        const iterationTime = benchNow() - t1;

        n++;
        avg += iterationTime;
        budget -= iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
      }
    } else {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();

        await fn();
        const iterationTime = benchNow() - t1;

        n++;
        avg += iterationTime;
        budget -= iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
      }
    }
  } else {
    let iterations = 10;
    let budget = timeBudget * 1e6;

    if (!async) {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        for (let c = 0; c < lowPrecisionThresholdInNs; c++) fn();
        const iterationTime = (benchNow() - t1) / lowPrecisionThresholdInNs;

        n++;
        avg += iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
        budget -= iterationTime * lowPrecisionThresholdInNs;
      }
    } else {
      while (budget > 0 || iterations-- > 0) {
        const t1 = benchNow();
        for (let c = 0; c < lowPrecisionThresholdInNs; c++) await fn();
        const iterationTime = (benchNow() - t1) / lowPrecisionThresholdInNs;

        n++;
        avg += iterationTime;
        ArrayPrototypePush(all, iterationTime);
        if (iterationTime < min) min = iterationTime;
        if (iterationTime > max) max = iterationTime;
        budget -= iterationTime * lowPrecisionThresholdInNs;
      }
    }
  }

  all.sort(compareMeasurements);
  return benchStats(n, wavg > lowPrecisionThresholdInNs, avg, min, max, all);
}

/** Wrap a user benchmark function in one which returns a structured result. */
function wrapBenchmark(desc) {
  const fn = desc.fn;
  return async function outerWrapped() {
    let token = null;
    const originalConsole = globalThis.console;

    try {
      globalThis.console = new Console((s) => {
        ops.op_dispatch_bench_event({ output: s });
      });

      if (desc.permissions) {
        token = pledgePermissions(desc.permissions);
      }

      if (desc.sanitizeExit) {
        setExitHandler((exitCode) => {
          assert(
            false,
            `Bench attempted to exit with exit code: ${exitCode}`,
          );
        });
      }

      const benchTimeInMs = 500;
      const stats = await benchMeasure(benchTimeInMs, fn, desc.async);

      return { ok: stats };
    } catch (error) {
      return { failed: core.destructureError(error) };
    } finally {
      globalThis.console = originalConsole;
      if (bench.sanitizeExit) setExitHandler(null);
      if (token !== null) restorePermissions(token);
    }
  };
}

function benchNow() {
  return ops.op_bench_now();
}

function getFullName(desc) {
  if ("parent" in desc) {
    return `${getFullName(desc.parent)} ... ${desc.name}`;
  }
  return desc.name;
}

function usesSanitizer(desc) {
  return desc.sanitizeResources || desc.sanitizeOps || desc.sanitizeExit;
}

function stepReportResult(desc, result, elapsed) {
  const state = MapPrototypeGet(testStates, desc.id);
  for (const childDesc of state.children) {
    stepReportResult(childDesc, { failed: "incomplete" }, 0);
  }
  ops.op_dispatch_test_event({
    stepResult: [desc.id, result, elapsed],
  });
}

/** @param desc {TestDescription | TestStepDescription} */
function createTestContext(desc) {
  let parent;
  let level;
  let rootId;
  let rootName;
  if ("parent" in desc) {
    parent = MapPrototypeGet(testStates, desc.parent.id).context;
    level = desc.level;
    rootId = desc.rootId;
    rootName = desc.rootName;
  } else {
    parent = undefined;
    level = 0;
    rootId = desc.id;
    rootName = desc.name;
  }
  return {
    [SymbolToStringTag]: "TestContext",
    /**
     * The current test name.
     */
    name: desc.name,
    /**
     * Parent test context.
     */
    parent,
    /**
     * File Uri of the test code.
     */
    origin: desc.origin,
    /**
     * @param nameOrFnOrOptions {string | TestStepDefinition | ((t: TestContext) => void | Promise<void>)}
     * @param maybeFn {((t: TestContext) => void | Promise<void>) | undefined}
     */
    async step(nameOrFnOrOptions, maybeFn) {
      if (MapPrototypeGet(testStates, desc.id).completed) {
        throw new Error(
          "Cannot run test step after parent scope has finished execution. " +
            "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
        );
      }

      let stepDesc;
      if (typeof nameOrFnOrOptions === "string") {
        if (!(ObjectPrototypeIsPrototypeOf(FunctionPrototype, maybeFn))) {
          throw new TypeError("Expected function for second argument.");
        }
        stepDesc = {
          name: nameOrFnOrOptions,
          fn: maybeFn,
        };
      } else if (typeof nameOrFnOrOptions === "function") {
        if (!nameOrFnOrOptions.name) {
          throw new TypeError("The step function must have a name.");
        }
        if (maybeFn != undefined) {
          throw new TypeError(
            "Unexpected second argument to TestContext.step()",
          );
        }
        stepDesc = {
          name: nameOrFnOrOptions.name,
          fn: nameOrFnOrOptions,
        };
      } else if (typeof nameOrFnOrOptions === "object") {
        stepDesc = nameOrFnOrOptions;
      } else {
        throw new TypeError(
          "Expected a test definition or name and function.",
        );
      }
      stepDesc.ignore ??= false;
      stepDesc.sanitizeOps ??= desc.sanitizeOps;
      stepDesc.sanitizeResources ??= desc.sanitizeResources;
      stepDesc.sanitizeExit ??= desc.sanitizeExit;
      const jsError = core.destructureError(new Error());
      stepDesc.location = {
        fileName: jsError.frames[1].fileName,
        lineNumber: jsError.frames[1].lineNumber,
        columnNumber: jsError.frames[1].columnNumber,
      };
      stepDesc.level = level + 1;
      stepDesc.parent = desc;
      stepDesc.rootId = rootId;
      stepDesc.rootName = rootName;
      stepDesc.fn = wrapTest(stepDesc);
      const { id, origin } = ops.op_register_test_step(stepDesc);
      stepDesc.id = id;
      stepDesc.origin = origin;
      const state = {
        context: createTestContext(stepDesc),
        children: [],
        failed: false,
        completed: false,
      };
      MapPrototypeSet(testStates, stepDesc.id, state);
      ArrayPrototypePush(
        MapPrototypeGet(testStates, stepDesc.parent.id).children,
        stepDesc,
      );

      ops.op_dispatch_test_event({ stepWait: stepDesc.id });
      const earlier = DateNow();
      const result = await stepDesc.fn(stepDesc);
      const elapsed = DateNow() - earlier;
      state.failed = !!result.failed;
      stepReportResult(stepDesc, result, elapsed);
      return result == "ok";
    },
  };
}

/**
 * Wrap a user test function in one which returns a structured result.
 * @template T {Function}
 * @param testFn {T}
 * @param desc {TestDescription | TestStepDescription}
 * @returns {T}
 */
function wrapTest(desc) {
  let testFn = wrapInner(desc.fn);
  if (desc.sanitizeOps) {
    testFn = assertOps(testFn);
  }
  if (desc.sanitizeResources) {
    testFn = assertResources(testFn);
  }
  if (desc.sanitizeExit) {
    testFn = assertExit(testFn, true);
  }
  if (!("parent" in desc) && desc.permissions) {
    testFn = withPermissions(testFn, desc.permissions);
  }
  return wrapOuter(testFn, desc);
}

import { denoNs } from "ext:runtime/90_deno_ns.js";
denoNs.bench = bench;
denoNs.test = test;
