// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { setExitHandler } = window.__bootstrap.os;
  const { Console, inspectArgs } = window.__bootstrap.console;
  const { serializePermissions } = window.__bootstrap.permissions;
  const { assert } = window.__bootstrap.infra;
  const {
    AggregateErrorPrototype,
    ArrayFrom,
    ArrayPrototypeFilter,
    ArrayPrototypeJoin,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ArrayPrototypeShift,
    ArrayPrototypeSome,
    ArrayPrototypeSort,
    DateNow,
    Error,
    FunctionPrototype,
    Map,
    MapPrototypeHas,
    MathCeil,
    ObjectKeys,
    ObjectPrototypeIsPrototypeOf,
    Promise,
    RegExp,
    RegExpPrototypeTest,
    SafeArrayIterator,
    Set,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeSlice,
    StringPrototypeStartsWith,
    SymbolToStringTag,
    TypeError,
  } = window.__bootstrap.primordials;

  const opSanitizerDelayResolveQueue = [];

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
    "op_dgram_recv": ["receive a datagram message", "awaiting the result of `Deno.DatagramConn#receive` call, or not breaking out of a for await loop looping over a `Deno.DatagramConn`"],
    "op_dgram_send": ["send a datagram message", "awaiting the result of `Deno.DatagramConn#send` call"],
    "op_dns_resolve": ["resolve a DNS name", "awaiting the result of a `Deno.resolveDns` call"],
    "op_emit": ["transpile code", "awaiting the result of a `Deno.emit` call"],
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
    "op_http_read": ["read the body of a HTTP request", "consuming the entire request body"],
    "op_http_shutdown": ["shutdown a HTTP connection", "awaiting `Deno.HttpEvent#respondWith`"],
    "op_http_upgrade_websocket": ["upgrade a HTTP connection to a WebSocket", "awaiting `Deno.HttpEvent#respondWith`"],
    "op_http_write_headers": ["write HTTP response headers", "awaiting `Deno.HttpEvent#respondWith`"],
    "op_http_write": ["write HTTP response body", "awaiting `Deno.HttpEvent#respondWith`"],
    "op_link_async": ["create a hard link", "awaiting the result of a `Deno.link` call"],
    "op_make_temp_dir_async": ["create a temporary directory", "awaiting the result of a `Deno.makeTempDir` call"],
    "op_make_temp_file_async": ["create a temporary file", "awaiting the result of a `Deno.makeTempFile` call"],
    "op_message_port_recv_message": ["receive a message from a MessagePort", "awaiting the result of not closing a `MessagePort`"],
    "op_mkdir_async": ["create a directory", "awaiting the result of a `Deno.mkdir` call"],
    "op_net_accept": ["accept a TCP connection", "closing a `Deno.Listener`"],
    "op_net_connect": ["connect to a TCP or UDP server", "awaiting a `Deno.connect` call"],
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
    "op_tls_accept": ["accept a TLS connection", "closing a `Deno.TlsListener`"],
    "op_tls_connect": ["connect to a TLS server", "awaiting a `Deno.connectTls` call"],
    "op_tls_handshake": ["perform a TLS handshake", "awaiting a `Deno.TlsConn#handshake` call"],
    "op_tls_start": ["start a TLS connection", "awaiting a `Deno.startTls` call"],
    "op_truncate_async": ["truncate a file", "awaiting the result of a `Deno.truncate` call"],
    "op_utime_async": ["change file timestamps", "awaiting the result of a `Deno.utime` call"],
    "op_webgpu_buffer_get_map_async": ["map a WebGPU buffer", "awaiting the result of a `GPUBuffer#mapAsync` call"],
    "op_webgpu_request_adapter": ["request a WebGPU adapter", "awaiting the result of a `navigator.gpu.requestAdapter` call"],
    "op_webgpu_request_device": ["request a WebGPU device", "awaiting the result of a `GPUAdapter#requestDevice` call"],
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
    /** @param step {TestStep} */
    return async function asyncOpSanitizer(step) {
      const pre = core.metrics();
      const preTraces = new Map(core.opCallTraces);
      try {
        await fn(step);
      } finally {
        // Defer until next event loop turn - that way timeouts and intervals
        // cleared can actually be removed from resource table, otherwise
        // false positives may occur (https://github.com/denoland/deno/issues/4591)
        await opSanitizerDelay();
      }

      if (step.shouldSkipSanitizers) return;

      const post = core.metrics();
      const postTraces = new Map(core.opCallTraces);

      // We're checking diff because one might spawn HTTP server in the background
      // that will be a pending async op before test starts.
      const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
      const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;

      if (dispatchedDiff === completedDiff) return;

      const details = [];
      for (const key in post.ops) {
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

      let msg = `Test case is leaking async ops.

- ${ArrayPrototypeJoin(details, "\n - ")}`;

      if (!core.isOpCallTracingEnabled()) {
        msg +=
          `\n\nTo get more details where ops were leaked, run again with --trace-ops flag.`;
      }

      throw msg;
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
        return ["A text decoder", "created", "finsihed"];
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
        return "Close the inbound HTTP request by responding with `e.respondWith().` or closing the HTTP connection.";
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
        return "Close the child process stdout by calling `proc.stdout.close()`.";
      case "childStderr":
        return "Close the child process stderr by calling `proc.stderr.close()`.";
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
  function assertResources(
    fn,
  ) {
    /** @param step {TestStep} */
    return async function resourceSanitizer(step) {
      const pre = core.resources();
      await fn(step);

      if (step.shouldSkipSanitizers) {
        return;
      }

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
          details.push(detail);
        } else {
          const [name, action1, action2] = prettyResourceNames(preResource);
          const detail =
            `${name} (rid ${resource}) was ${action1} before the test started, but was ${action2} during the test. Do not close resources in a test that were not created during that test.`;
          details.push(detail);
        }
      }

      const message = `Test case is leaking ${details.length} resource${
        details.length === 1 ? "" : "s"
      }:

 - ${details.join("\n - ")}
`;
      assert(details.length === 0, message);
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
        await fn(...new SafeArrayIterator(params));
      } catch (err) {
        throw err;
      } finally {
        setExitHandler(null);
      }
    };
  }

  function assertTestStepScopes(fn) {
    /** @param step {TestStep} */
    return async function testStepSanitizer(step) {
      preValidation();
      // only report waiting after pre-validation
      if (step.canStreamReporting()) {
        step.reportWait();
      }
      await fn(createTestContext(step));
      postValidation();

      function preValidation() {
        const runningSteps = getPotentialConflictingRunningSteps();
        const runningStepsWithSanitizers = ArrayPrototypeFilter(
          runningSteps,
          (t) => t.usesSanitizer,
        );

        if (runningStepsWithSanitizers.length > 0) {
          throw new Error(
            "Cannot start test step while another test step with sanitizers is running.\n" +
              runningStepsWithSanitizers
                .map((s) => ` * ${s.getFullName()}`)
                .join("\n"),
          );
        }

        if (step.usesSanitizer && runningSteps.length > 0) {
          throw new Error(
            "Cannot start test step with sanitizers while another test step is running.\n" +
              runningSteps.map((s) => ` * ${s.getFullName()}`).join("\n"),
          );
        }

        function getPotentialConflictingRunningSteps() {
          /** @type {TestStep[]} */
          const results = [];

          let childStep = step;
          for (const ancestor of step.ancestors()) {
            for (const siblingStep of ancestor.children) {
              if (siblingStep === childStep) {
                continue;
              }
              if (!siblingStep.finalized) {
                ArrayPrototypePush(results, siblingStep);
              }
            }
            childStep = ancestor;
          }
          return results;
        }
      }

      function postValidation() {
        // check for any running steps
        if (step.hasRunningChildren) {
          throw new Error(
            "There were still test steps running after the current scope finished execution. " +
              "Ensure all steps are awaited (ex. `await t.step(...)`).",
          );
        }

        // check if an ancestor already completed
        for (const ancestor of step.ancestors()) {
          if (ancestor.finalized) {
            throw new Error(
              "Parent scope completed before test step finished execution. " +
                "Ensure all steps are awaited (ex. `await t.step(...)`).",
            );
          }
        }
      }
    };
  }

  function pledgePermissions(permissions) {
    return core.opSync(
      "op_pledge_test_permissions",
      serializePermissions(permissions),
    );
  }

  function restorePermissions(token) {
    core.opSync("op_restore_test_permissions", token);
  }

  function withPermissions(fn, permissions) {
    return async function applyPermissions(...params) {
      const token = pledgePermissions(permissions);

      try {
        await fn(...new SafeArrayIterator(params));
      } finally {
        restorePermissions(token);
      }
    };
  }

  const tests = [];
  const benches = [];

  // Main test function provided by Deno.
  function test(
    nameOrFnOrOptions,
    optionsOrFn,
    maybeFn,
  ) {
    let testDef;
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
        testDef = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
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
        testDef = {
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
      testDef = {
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
      testDef = { ...defaults, ...nameOrFnOrOptions, fn, name };
    }

    testDef.fn = wrapTestFnWithSanitizers(testDef.fn, testDef);

    if (testDef.permissions) {
      testDef.fn = withPermissions(
        testDef.fn,
        testDef.permissions,
      );
    }

    ArrayPrototypePush(tests, testDef);
  }

  // Main bench function provided by Deno.
  function bench(
    nameOrFnOrOptions,
    optionsOrFn,
    maybeFn,
  ) {
    core.opSync("op_bench_check_unstable");
    let benchDef;
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
        throw new TypeError("The bench name can't be empty");
      }
      if (typeof optionsOrFn === "function") {
        benchDef = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
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
        benchDef = {
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
      benchDef = {
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
      benchDef = { ...defaults, ...nameOrFnOrOptions, fn, name };
    }

    const AsyncFunction = (async () => {}).constructor;
    benchDef.async = AsyncFunction === benchDef.fn.constructor;

    ArrayPrototypePush(benches, benchDef);
  }

  function formatError(error) {
    if (ObjectPrototypeIsPrototypeOf(AggregateErrorPrototype, error)) {
      const message = error
        .errors
        .map((error) =>
          inspectArgs([error]).replace(/^(?!\s*$)/gm, " ".repeat(4))
        )
        .join("\n");

      return error.name + "\n" + message + error.stack;
    }

    return inspectArgs([error]);
  }

  /**
   * @param {string | { include?: string[], exclude?: string[] }} filter
   * @returns {(def: { name: string }) => boolean}
   */
  function createTestFilter(filter) {
    if (!filter) {
      return () => true;
    }

    const regex =
      typeof filter === "string" && StringPrototypeStartsWith(filter, "/") &&
        StringPrototypeEndsWith(filter, "/")
        ? new RegExp(StringPrototypeSlice(filter, 1, filter.length - 1))
        : undefined;

    const filterIsObject = filter != null && typeof filter === "object";

    return (def) => {
      if (regex) {
        return RegExpPrototypeTest(regex, def.name);
      }
      if (filterIsObject) {
        if (filter.include && !filter.include.includes(def.name)) {
          return false;
        } else if (filter.exclude && filter.exclude.includes(def.name)) {
          return false;
        } else {
          return true;
        }
      }
      return StringPrototypeIncludes(def.name, filter);
    };
  }

  async function runTest(test, description) {
    if (test.ignore) {
      return "ignored";
    }

    const step = new TestStep({
      name: test.name,
      parent: undefined,
      parentContext: undefined,
      rootTestDescription: description,
      sanitizeOps: test.sanitizeOps,
      sanitizeResources: test.sanitizeResources,
      sanitizeExit: test.sanitizeExit,
    });

    try {
      await test.fn(step);
      const failCount = step.failedChildStepsCount();
      return failCount === 0 ? "ok" : {
        "failed": core.destructureError(
          new Error(
            `${failCount} test step${failCount === 1 ? "" : "s"} failed.`,
          ),
        ),
      };
    } catch (error) {
      return {
        "failed": core.destructureError(error),
      };
    } finally {
      step.finalized = true;
      // ensure the children report their result
      for (const child of step.children) {
        child.reportResult();
      }
    }
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

  async function benchMeasure(timeBudget, fn, step, sync) {
    let n = 0;
    let avg = 0;
    let wavg = 0;
    const all = [];
    let min = Infinity;
    let max = -Infinity;
    const lowPrecisionThresholdInNs = 1e4;

    // warmup step
    let c = 0;
    step.warmup = true;
    let iterations = 20;
    let budget = 10 * 1e6;

    if (sync) {
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
    step.warmup = false;

    if (wavg > lowPrecisionThresholdInNs) {
      let iterations = 10;
      let budget = timeBudget * 1e6;

      if (sync) {
        while (budget > 0 || iterations-- > 0) {
          const t1 = benchNow();

          fn();
          const iterationTime = benchNow() - t1;

          n++;
          avg += iterationTime;
          budget -= iterationTime;
          all.push(iterationTime);
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
          all.push(iterationTime);
          if (iterationTime < min) min = iterationTime;
          if (iterationTime > max) max = iterationTime;
        }
      }
    } else {
      let iterations = 10;
      let budget = timeBudget * 1e6;

      if (sync) {
        while (budget > 0 || iterations-- > 0) {
          const t1 = benchNow();
          for (let c = 0; c < lowPrecisionThresholdInNs; c++) fn();
          const iterationTime = (benchNow() - t1) / lowPrecisionThresholdInNs;

          n++;
          avg += iterationTime;
          all.push(iterationTime);
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
          all.push(iterationTime);
          if (iterationTime < min) min = iterationTime;
          if (iterationTime > max) max = iterationTime;
          budget -= iterationTime * lowPrecisionThresholdInNs;
        }
      }
    }

    all.sort(compareMeasurements);
    return benchStats(n, wavg > lowPrecisionThresholdInNs, avg, min, max, all);
  }

  async function runBench(bench) {
    const step = new BenchStep({
      name: bench.name,
      sanitizeExit: bench.sanitizeExit,
      warmup: false,
    });

    let token = null;

    try {
      if (bench.permissions) {
        token = pledgePermissions(bench.permissions);
      }

      if (bench.sanitizeExit) {
        setExitHandler((exitCode) => {
          assert(
            false,
            `Bench attempted to exit with exit code: ${exitCode}`,
          );
        });
      }

      const benchTimeInMs = 500;
      const fn = bench.fn.bind(null, step);
      const stats = await benchMeasure(benchTimeInMs, fn, step, !bench.async);

      return { ok: { stats, ...bench } };
    } catch (error) {
      return { failed: { ...bench, error: formatError(error) } };
    } finally {
      if (bench.sanitizeExit) setExitHandler(null);
      if (token !== null) restorePermissions(token);
    }
  }

  function getTestOrigin() {
    return core.opSync("op_get_test_origin");
  }

  function getBenchOrigin() {
    return core.opSync("op_get_bench_origin");
  }

  function reportTestPlan(plan) {
    core.opSync("op_dispatch_test_event", {
      plan,
    });
  }

  function reportTestWait(test) {
    core.opSync("op_dispatch_test_event", {
      wait: test,
    });
  }

  function reportTestResult(test, result, elapsed) {
    core.opSync("op_dispatch_test_event", {
      result: [test, result, elapsed],
    });
  }

  function reportTestStepWait(testDescription) {
    core.opSync("op_dispatch_test_event", {
      stepWait: testDescription,
    });
  }

  function reportTestStepResult(testDescription, result, elapsed) {
    core.opSync("op_dispatch_test_event", {
      stepResult: [testDescription, result, elapsed],
    });
  }

  function reportBenchPlan(plan) {
    core.opSync("op_dispatch_bench_event", {
      plan,
    });
  }

  function reportBenchConsoleOutput(console) {
    core.opSync("op_dispatch_bench_event", {
      output: { console },
    });
  }

  function reportBenchWait(description) {
    core.opSync("op_dispatch_bench_event", {
      wait: description,
    });
  }

  function reportBenchResult(origin, result) {
    core.opSync("op_dispatch_bench_event", {
      result: [origin, result],
    });
  }

  function benchNow() {
    return core.opSync("op_bench_now");
  }

  async function runTests({
    filter = null,
    shuffle = null,
  } = {}) {
    core.setMacrotaskCallback(handleOpSanitizerDelayMacrotask);

    const origin = getTestOrigin();

    const only = ArrayPrototypeFilter(tests, (test) => test.only);
    const filtered = ArrayPrototypeFilter(
      only.length > 0 ? only : tests,
      createTestFilter(filter),
    );

    reportTestPlan({
      origin,
      total: filtered.length,
      filteredOut: tests.length - filtered.length,
      usedOnly: only.length > 0,
    });

    if (shuffle !== null) {
      // http://en.wikipedia.org/wiki/Linear_congruential_generator
      const nextInt = (function (state) {
        const m = 0x80000000;
        const a = 1103515245;
        const c = 12345;

        return function (max) {
          return state = ((a * state + c) % m) % max;
        };
      }(shuffle));

      for (let i = filtered.length - 1; i > 0; i--) {
        const j = nextInt(i);
        [filtered[i], filtered[j]] = [filtered[j], filtered[i]];
      }
    }

    for (const test of filtered) {
      const description = {
        origin,
        name: test.name,
      };
      const earlier = DateNow();

      reportTestWait(description);

      const result = await runTest(test, description);
      const elapsed = DateNow() - earlier;

      reportTestResult(description, result, elapsed);
    }
  }

  async function runBenchmarks({
    filter = null,
  } = {}) {
    core.setMacrotaskCallback(handleOpSanitizerDelayMacrotask);

    const origin = getBenchOrigin();
    const originalConsole = globalThis.console;

    globalThis.console = new Console(reportBenchConsoleOutput);

    const only = ArrayPrototypeFilter(benches, (bench) => bench.only);
    const filtered = ArrayPrototypeFilter(
      only.length > 0 ? only : benches,
      createTestFilter(filter),
    );

    let groups = new Set();
    const benchmarks = ArrayPrototypeFilter(filtered, (bench) => !bench.ignore);

    // make sure ungrouped benchmarks are placed above grouped
    groups.add(undefined);

    for (const bench of benchmarks) {
      bench.group ||= undefined;
      groups.add(bench.group);
    }

    groups = ArrayFrom(groups);
    ArrayPrototypeSort(
      benchmarks,
      (a, b) => groups.indexOf(a.group) - groups.indexOf(b.group),
    );

    reportBenchPlan({
      origin,
      total: benchmarks.length,
      usedOnly: only.length > 0,
      names: ArrayPrototypeMap(benchmarks, (bench) => bench.name),
    });

    for (const bench of benchmarks) {
      bench.baseline = !!bench.baseline;
      reportBenchWait({ origin, ...bench });
      reportBenchResult(origin, await runBench(bench));
    }

    globalThis.console = originalConsole;
  }

  /**
   * @typedef {{
   *   fn: (t: TestContext) => void | Promise<void>,
   *   name: string,
   *   ignore?: boolean,
   *   sanitizeOps?: boolean,
   *   sanitizeResources?: boolean,
   *   sanitizeExit?: boolean,
   * }} TestStepDefinition
   *
   * @typedef {{
   *   name: string,
   *   parent: TestStep | undefined,
   *   parentContext: TestContext | undefined,
   *   rootTestDescription: { origin: string; name: string };
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   * }} TestStepParams
   */

  class TestStep {
    /** @type {TestStepParams} */
    #params;
    reportedWait = false;
    #reportedResult = false;
    finalized = false;
    elapsed = 0;
    /** @type "ok" | "ignored" | "pending" | "failed" */
    status = "pending";
    error = undefined;
    /** @type {TestStep[]} */
    children = [];

    /** @param params {TestStepParams} */
    constructor(params) {
      this.#params = params;
    }

    get name() {
      return this.#params.name;
    }

    get parent() {
      return this.#params.parent;
    }

    get parentContext() {
      return this.#params.parentContext;
    }

    get rootTestDescription() {
      return this.#params.rootTestDescription;
    }

    get sanitizerOptions() {
      return {
        sanitizeResources: this.#params.sanitizeResources,
        sanitizeOps: this.#params.sanitizeOps,
        sanitizeExit: this.#params.sanitizeExit,
      };
    }

    get usesSanitizer() {
      return this.#params.sanitizeResources ||
        this.#params.sanitizeOps ||
        this.#params.sanitizeExit;
    }

    /** If a test validation error already occurred then don't bother checking
     * the sanitizers as that will create extra noise.
     */
    get shouldSkipSanitizers() {
      return this.hasRunningChildren || this.parent?.finalized;
    }

    get hasRunningChildren() {
      return ArrayPrototypeSome(
        this.children,
        /** @param step {TestStep} */
        (step) => step.status === "pending",
      );
    }

    failedChildStepsCount() {
      return ArrayPrototypeFilter(
        this.children,
        /** @param step {TestStep} */
        (step) => step.status === "failed",
      ).length;
    }

    canStreamReporting() {
      // there should only ever be one sub step running when running with
      // sanitizers, so we can use this to tell if we can stream reporting
      return this.selfAndAllAncestorsUseSanitizer() &&
        this.children.every((c) => c.usesSanitizer || c.finalized);
    }

    selfAndAllAncestorsUseSanitizer() {
      if (!this.usesSanitizer) {
        return false;
      }

      for (const ancestor of this.ancestors()) {
        if (!ancestor.usesSanitizer) {
          return false;
        }
      }

      return true;
    }

    *ancestors() {
      let ancestor = this.parent;
      while (ancestor) {
        yield ancestor;
        ancestor = ancestor.parent;
      }
    }

    getFullName() {
      if (this.parent) {
        return `${this.parent.getFullName()} > ${this.name}`;
      } else {
        return this.name;
      }
    }

    reportWait() {
      if (this.reportedWait || !this.parent) {
        return;
      }

      reportTestStepWait(this.#getTestStepDescription());

      this.reportedWait = true;
    }

    reportResult() {
      if (this.#reportedResult || !this.parent) {
        return;
      }

      this.reportWait();

      for (const child of this.children) {
        child.reportResult();
      }

      reportTestStepResult(
        this.#getTestStepDescription(),
        this.#getStepResult(),
        this.elapsed,
      );

      this.#reportedResult = true;
    }

    #getStepResult() {
      switch (this.status) {
        case "ok":
          return "ok";
        case "ignored":
          return "ignored";
        case "pending":
          return {
            "pending": this.error && core.destructureError(this.error),
          };
        case "failed":
          return {
            "failed": this.error && core.destructureError(this.error),
          };
        default:
          throw new Error(`Unhandled status: ${this.status}`);
      }
    }

    #getTestStepDescription() {
      return {
        test: this.rootTestDescription,
        name: this.name,
        level: this.#getLevel(),
      };
    }

    #getLevel() {
      let count = 0;
      for (const _ of this.ancestors()) {
        count++;
      }
      return count;
    }
  }

  /**
   * @typedef {{
   *   name: string;
   *   sanitizeExit: boolean,
   *   warmup: boolean,
   * }} BenchStepParams
   */
  class BenchStep {
    /** @type {BenchStepParams} */
    #params;

    /** @param params {BenchStepParams} */
    constructor(params) {
      this.#params = params;
    }

    get name() {
      return this.#params.name;
    }
  }

  /** @param parentStep {TestStep} */
  function createTestContext(parentStep) {
    return {
      [SymbolToStringTag]: "TestContext",
      /**
       * The current test name.
       */
      name: parentStep.name,
      /**
       * Parent test context.
       */
      parent: parentStep.parentContext ?? undefined,
      /**
       * File Uri of the test code.
       */
      origin: parentStep.rootTestDescription.origin,
      /**
       * @param nameOrTestDefinition {string | TestStepDefinition}
       * @param fn {(t: TestContext) => void | Promise<void>}
       */
      async step(nameOrTestDefinition, fn) {
        if (parentStep.finalized) {
          throw new Error(
            "Cannot run test step after parent scope has finished execution. " +
              "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
          );
        }

        const definition = getDefinition();
        const subStep = new TestStep({
          name: definition.name,
          parent: parentStep,
          parentContext: this,
          rootTestDescription: parentStep.rootTestDescription,
          sanitizeOps: getOrDefault(
            definition.sanitizeOps,
            parentStep.sanitizerOptions.sanitizeOps,
          ),
          sanitizeResources: getOrDefault(
            definition.sanitizeResources,
            parentStep.sanitizerOptions.sanitizeResources,
          ),
          sanitizeExit: getOrDefault(
            definition.sanitizeExit,
            parentStep.sanitizerOptions.sanitizeExit,
          ),
        });

        ArrayPrototypePush(parentStep.children, subStep);

        try {
          if (definition.ignore) {
            subStep.status = "ignored";
            subStep.finalized = true;
            if (subStep.canStreamReporting()) {
              subStep.reportResult();
            }
            return false;
          }

          const testFn = wrapTestFnWithSanitizers(
            definition.fn,
            subStep.sanitizerOptions,
          );
          const start = DateNow();

          try {
            await testFn(subStep);

            if (subStep.failedChildStepsCount() > 0) {
              subStep.status = "failed";
            } else {
              subStep.status = "ok";
            }
          } catch (error) {
            subStep.error = error;
            subStep.status = "failed";
          }

          subStep.elapsed = DateNow() - start;

          if (subStep.parent?.finalized) {
            // always point this test out as one that was still running
            // if the parent step finalized
            subStep.status = "pending";
          }

          subStep.finalized = true;

          if (subStep.reportedWait && subStep.canStreamReporting()) {
            subStep.reportResult();
          }

          return subStep.status === "ok";
        } finally {
          if (parentStep.canStreamReporting()) {
            // flush any buffered steps
            for (const parentChild of parentStep.children) {
              parentChild.reportResult();
            }
          }
        }

        /** @returns {TestStepDefinition} */
        function getDefinition() {
          if (typeof nameOrTestDefinition === "string") {
            if (!(ObjectPrototypeIsPrototypeOf(FunctionPrototype, fn))) {
              throw new TypeError("Expected function for second argument.");
            }
            return {
              name: nameOrTestDefinition,
              fn,
            };
          } else if (typeof nameOrTestDefinition === "object") {
            return nameOrTestDefinition;
          } else {
            throw new TypeError(
              "Expected a test definition or name and function.",
            );
          }
        }
      },
    };
  }

  /**
   * @template T {Function}
   * @param testFn {T}
   * @param opts {{
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   * }}
   * @returns {T}
   */
  function wrapTestFnWithSanitizers(testFn, opts) {
    testFn = assertTestStepScopes(testFn);

    if (opts.sanitizeOps) {
      testFn = assertOps(testFn);
    }
    if (opts.sanitizeResources) {
      testFn = assertResources(testFn);
    }
    if (opts.sanitizeExit) {
      testFn = assertExit(testFn, true);
    }
    return testFn;
  }

  /**
   * @template T
   * @param value {T | undefined}
   * @param defaultValue {T}
   * @returns T
   */
  function getOrDefault(value, defaultValue) {
    return value == null ? defaultValue : value;
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    runTests,
    runBenchmarks,
  };

  window.__bootstrap.testing = {
    test,
    bench,
  };
})(this);
