// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

// Removes the `__proto__` for security reasons.
// https://tc39.es/ecma262/#sec-get-object.prototype.__proto__
delete Object.prototype.__proto__;

// Remove Intl.v8BreakIterator because it is a non-standard API.
delete Intl.v8BreakIterator;

((window) => {
  const core = Deno.core;
  const ops = core.ops;
  const {
    ArrayPrototypeIndexOf,
    ArrayPrototypePush,
    ArrayPrototypeShift,
    ArrayPrototypeSplice,
    ArrayPrototypeMap,
    DateNow,
    Error,
    FunctionPrototypeCall,
    FunctionPrototypeBind,
    ObjectAssign,
    ObjectDefineProperty,
    ObjectDefineProperties,
    ObjectFreeze,
    ObjectPrototypeIsPrototypeOf,
    ObjectSetPrototypeOf,
    PromiseResolve,
    Symbol,
    SymbolFor,
    SymbolIterator,
    PromisePrototypeThen,
    SafeWeakMap,
    TypeError,
    WeakMapPrototypeDelete,
    WeakMapPrototypeGet,
    WeakMapPrototypeSet,
  } = window.__bootstrap.primordials;
  const util = window.__bootstrap.util;
  const eventTarget = window.__bootstrap.eventTarget;
  const globalInterfaces = window.__bootstrap.globalInterfaces;
  const location = window.__bootstrap.location;
  const build = window.__bootstrap.build;
  const version = window.__bootstrap.version;
  const os = window.__bootstrap.os;
  const timers = window.__bootstrap.timers;
  const base64 = window.__bootstrap.base64;
  const encoding = window.__bootstrap.encoding;
  const colors = window.__bootstrap.colors;
  const Console = window.__bootstrap.console.Console;
  const CacheStorage = window.__bootstrap.caches.CacheStorage;
  const inspectArgs = window.__bootstrap.console.inspectArgs;
  const quoteString = window.__bootstrap.console.quoteString;
  const compression = window.__bootstrap.compression;
  const worker = window.__bootstrap.worker;
  const internals = window.__bootstrap.internals;
  const performance = window.__bootstrap.performance;
  const crypto = window.__bootstrap.crypto;
  const url = window.__bootstrap.url;
  const urlPattern = window.__bootstrap.urlPattern;
  const headers = window.__bootstrap.headers;
  const streams = window.__bootstrap.streams;
  const fileReader = window.__bootstrap.fileReader;
  const webgpu = window.__bootstrap.webgpu;
  const webSocket = window.__bootstrap.webSocket;
  const webStorage = window.__bootstrap.webStorage;
  const broadcastChannel = window.__bootstrap.broadcastChannel;
  const file = window.__bootstrap.file;
  const formData = window.__bootstrap.formData;
  const fetch = window.__bootstrap.fetch;
  const prompt = window.__bootstrap.prompt;
  const messagePort = window.__bootstrap.messagePort;
  const denoNs = window.__bootstrap.denoNs;
  const denoNsUnstable = window.__bootstrap.denoNsUnstable;
  const errors = window.__bootstrap.errors.errors;
  const webidl = window.__bootstrap.webidl;
  const domException = window.__bootstrap.domException;
  const { defineEventHandler, reportException } = window.__bootstrap.event;
  const { deserializeJsMessageData, serializeJsMessageData } =
    window.__bootstrap.messagePort;

  let windowIsClosing = false;

  function windowClose() {
    if (!windowIsClosing) {
      windowIsClosing = true;
      // Push a macrotask to exit after a promise resolve.
      // This is not perfect, but should be fine for first pass.
      PromisePrototypeThen(
        PromiseResolve(),
        () =>
          FunctionPrototypeCall(timers.setTimeout, null, () => {
            // This should be fine, since only Window/MainWorker has .close()
            os.exit(0);
          }, 0),
      );
    }
  }

  function workerClose() {
    if (isClosing) {
      return;
    }

    isClosing = true;
    ops.op_worker_close();
  }

  function postMessage(message, transferOrOptions = {}) {
    const prefix =
      "Failed to execute 'postMessage' on 'DedicatedWorkerGlobalScope'";
    webidl.requiredArguments(arguments.length, 1, { prefix });
    message = webidl.converters.any(message);
    let options;
    if (
      webidl.type(transferOrOptions) === "Object" &&
      transferOrOptions !== undefined &&
      transferOrOptions[SymbolIterator] !== undefined
    ) {
      const transfer = webidl.converters["sequence<object>"](
        transferOrOptions,
        { prefix, context: "Argument 2" },
      );
      options = { transfer };
    } else {
      options = webidl.converters.StructuredSerializeOptions(
        transferOrOptions,
        {
          prefix,
          context: "Argument 2",
        },
      );
    }
    const { transfer } = options;
    const data = serializeJsMessageData(message, transfer);
    ops.op_worker_post_message(data);
  }

  let isClosing = false;
  let globalDispatchEvent;

  async function pollForMessages() {
    if (!globalDispatchEvent) {
      globalDispatchEvent = FunctionPrototypeBind(
        globalThis.dispatchEvent,
        globalThis,
      );
    }
    while (!isClosing) {
      const data = await core.opAsync("op_worker_recv_message");
      if (data === null) break;
      const v = deserializeJsMessageData(data);
      const message = v[0];
      const transferables = v[1];

      const msgEvent = new MessageEvent("message", {
        cancelable: false,
        data: message,
        ports: transferables.filter((t) =>
          ObjectPrototypeIsPrototypeOf(messagePort.MessagePortPrototype, t)
        ),
      });

      try {
        globalDispatchEvent(msgEvent);
      } catch (e) {
        const errorEvent = new ErrorEvent("error", {
          cancelable: true,
          message: e.message,
          lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
          colno: e.columnNumber ? e.columnNumber + 1 : undefined,
          filename: e.fileName,
          error: e,
        });

        globalDispatchEvent(errorEvent);
        if (!errorEvent.defaultPrevented) {
          throw e;
        }
      }
    }
  }

  let loadedMainWorkerScript = false;

  function importScripts(...urls) {
    if (ops.op_worker_get_type() === "module") {
      throw new TypeError("Can't import scripts in a module worker.");
    }

    const baseUrl = location.getLocationHref();
    const parsedUrls = ArrayPrototypeMap(urls, (scriptUrl) => {
      try {
        return new url.URL(scriptUrl, baseUrl ?? undefined).href;
      } catch {
        throw new domException.DOMException(
          "Failed to parse URL.",
          "SyntaxError",
        );
      }
    });

    // A classic worker's main script has looser MIME type checks than any
    // imported scripts, so we use `loadedMainWorkerScript` to distinguish them.
    // TODO(andreubotella) Refactor worker creation so the main script isn't
    // loaded with `importScripts()`.
    const scripts = ops.op_worker_sync_fetch(
      parsedUrls,
      !loadedMainWorkerScript,
    );
    loadedMainWorkerScript = true;

    for (const { url, script } of scripts) {
      const err = core.evalContext(script, url)[1];
      if (err !== null) {
        throw err.thrown;
      }
    }
  }

  function opMainModule() {
    return ops.op_main_module();
  }

  function formatException(error) {
    if (error instanceof Error) {
      return null;
    } else if (typeof error == "string") {
      return `Uncaught ${
        inspectArgs([quoteString(error)], {
          colors: !colors.getNoColor(),
        })
      }`;
    } else {
      return `Uncaught ${
        inspectArgs([error], { colors: !colors.getNoColor() })
      }`;
    }
  }

  function runtimeStart(runtimeOptions, source) {
    core.setMacrotaskCallback(timers.handleTimerMacrotask);
    core.setMacrotaskCallback(promiseRejectMacrotaskCallback);
    core.setWasmStreamingCallback(fetch.handleWasmStreaming);
    core.setReportExceptionCallback(reportException);
    ops.op_set_format_exception_callback(formatException);
    version.setVersions(
      runtimeOptions.denoVersion,
      runtimeOptions.v8Version,
      runtimeOptions.tsVersion,
    );
    build.setBuildInfo(runtimeOptions.target);
    util.setLogDebug(runtimeOptions.debugFlag, source);
    // deno-lint-ignore prefer-primordials
    Error.prepareStackTrace = core.prepareStackTrace;
  }

  function registerErrors() {
    core.registerErrorClass("NotFound", errors.NotFound);
    core.registerErrorClass("PermissionDenied", errors.PermissionDenied);
    core.registerErrorClass("ConnectionRefused", errors.ConnectionRefused);
    core.registerErrorClass("ConnectionReset", errors.ConnectionReset);
    core.registerErrorClass("ConnectionAborted", errors.ConnectionAborted);
    core.registerErrorClass("NotConnected", errors.NotConnected);
    core.registerErrorClass("AddrInUse", errors.AddrInUse);
    core.registerErrorClass("AddrNotAvailable", errors.AddrNotAvailable);
    core.registerErrorClass("BrokenPipe", errors.BrokenPipe);
    core.registerErrorClass("AlreadyExists", errors.AlreadyExists);
    core.registerErrorClass("InvalidData", errors.InvalidData);
    core.registerErrorClass("TimedOut", errors.TimedOut);
    core.registerErrorClass("Interrupted", errors.Interrupted);
    core.registerErrorClass("WriteZero", errors.WriteZero);
    core.registerErrorClass("UnexpectedEof", errors.UnexpectedEof);
    core.registerErrorClass("BadResource", errors.BadResource);
    core.registerErrorClass("Http", errors.Http);
    core.registerErrorClass("Busy", errors.Busy);
    core.registerErrorClass("NotSupported", errors.NotSupported);
    core.registerErrorBuilder(
      "DOMExceptionOperationError",
      function DOMExceptionOperationError(msg) {
        return new domException.DOMException(msg, "OperationError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionQuotaExceededError",
      function DOMExceptionQuotaExceededError(msg) {
        return new domException.DOMException(msg, "QuotaExceededError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionNotSupportedError",
      function DOMExceptionNotSupportedError(msg) {
        return new domException.DOMException(msg, "NotSupported");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionNetworkError",
      function DOMExceptionNetworkError(msg) {
        return new domException.DOMException(msg, "NetworkError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionAbortError",
      function DOMExceptionAbortError(msg) {
        return new domException.DOMException(msg, "AbortError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionInvalidCharacterError",
      function DOMExceptionInvalidCharacterError(msg) {
        return new domException.DOMException(msg, "InvalidCharacterError");
      },
    );
    core.registerErrorBuilder(
      "DOMExceptionDataError",
      function DOMExceptionDataError(msg) {
        return new domException.DOMException(msg, "DataError");
      },
    );
  }

  class Navigator {
    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  const navigator = webidl.createBranded(Navigator);

  let numCpus, userAgent;

  ObjectDefineProperties(Navigator.prototype, {
    gpu: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, NavigatorPrototype);
        return webgpu.gpu;
      },
    },
    hardwareConcurrency: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, NavigatorPrototype);
        return numCpus;
      },
    },
    userAgent: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, NavigatorPrototype);
        return userAgent;
      },
    },
  });
  const NavigatorPrototype = Navigator.prototype;

  class WorkerNavigator {
    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  const workerNavigator = webidl.createBranded(WorkerNavigator);

  ObjectDefineProperties(WorkerNavigator.prototype, {
    gpu: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, WorkerNavigatorPrototype);
        return webgpu.gpu;
      },
    },
    hardwareConcurrency: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, WorkerNavigatorPrototype);
        return numCpus;
      },
    },
  });
  const WorkerNavigatorPrototype = WorkerNavigator.prototype;

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScope = {
    Blob: util.nonEnumerable(file.Blob),
    ByteLengthQueuingStrategy: util.nonEnumerable(
      streams.ByteLengthQueuingStrategy,
    ),
    CloseEvent: util.nonEnumerable(CloseEvent),
    CompressionStream: util.nonEnumerable(compression.CompressionStream),
    CountQueuingStrategy: util.nonEnumerable(
      streams.CountQueuingStrategy,
    ),
    CryptoKey: util.nonEnumerable(crypto.CryptoKey),
    CustomEvent: util.nonEnumerable(CustomEvent),
    DecompressionStream: util.nonEnumerable(compression.DecompressionStream),
    DOMException: util.nonEnumerable(domException.DOMException),
    ErrorEvent: util.nonEnumerable(ErrorEvent),
    Event: util.nonEnumerable(Event),
    EventTarget: util.nonEnumerable(EventTarget),
    File: util.nonEnumerable(file.File),
    FileReader: util.nonEnumerable(fileReader.FileReader),
    FormData: util.nonEnumerable(formData.FormData),
    Headers: util.nonEnumerable(headers.Headers),
    MessageEvent: util.nonEnumerable(MessageEvent),
    Performance: util.nonEnumerable(performance.Performance),
    PerformanceEntry: util.nonEnumerable(performance.PerformanceEntry),
    PerformanceMark: util.nonEnumerable(performance.PerformanceMark),
    PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasure),
    PromiseRejectionEvent: util.nonEnumerable(PromiseRejectionEvent),
    ProgressEvent: util.nonEnumerable(ProgressEvent),
    ReadableStream: util.nonEnumerable(streams.ReadableStream),
    ReadableStreamDefaultReader: util.nonEnumerable(
      streams.ReadableStreamDefaultReader,
    ),
    Request: util.nonEnumerable(fetch.Request),
    Response: util.nonEnumerable(fetch.Response),
    TextDecoder: util.nonEnumerable(encoding.TextDecoder),
    TextEncoder: util.nonEnumerable(encoding.TextEncoder),
    TextDecoderStream: util.nonEnumerable(encoding.TextDecoderStream),
    TextEncoderStream: util.nonEnumerable(encoding.TextEncoderStream),
    TransformStream: util.nonEnumerable(streams.TransformStream),
    URL: util.nonEnumerable(url.URL),
    URLPattern: util.nonEnumerable(urlPattern.URLPattern),
    URLSearchParams: util.nonEnumerable(url.URLSearchParams),
    WebSocket: util.nonEnumerable(webSocket.WebSocket),
    MessageChannel: util.nonEnumerable(messagePort.MessageChannel),
    MessagePort: util.nonEnumerable(messagePort.MessagePort),
    Worker: util.nonEnumerable(worker.Worker),
    WritableStream: util.nonEnumerable(streams.WritableStream),
    WritableStreamDefaultWriter: util.nonEnumerable(
      streams.WritableStreamDefaultWriter,
    ),
    WritableStreamDefaultController: util.nonEnumerable(
      streams.WritableStreamDefaultController,
    ),
    ReadableByteStreamController: util.nonEnumerable(
      streams.ReadableByteStreamController,
    ),
    ReadableStreamBYOBReader: util.nonEnumerable(
      streams.ReadableStreamBYOBReader,
    ),
    ReadableStreamBYOBRequest: util.nonEnumerable(
      streams.ReadableStreamBYOBRequest,
    ),
    ReadableStreamDefaultController: util.nonEnumerable(
      streams.ReadableStreamDefaultController,
    ),
    TransformStreamDefaultController: util.nonEnumerable(
      streams.TransformStreamDefaultController,
    ),
    atob: util.writable(base64.atob),
    btoa: util.writable(base64.btoa),
    clearInterval: util.writable(timers.clearInterval),
    clearTimeout: util.writable(timers.clearTimeout),
    caches: util.nonEnumerable(new CacheStorage()),
    console: util.nonEnumerable(
      new Console((msg, level) => core.print(msg, level > 1)),
    ),
    crypto: util.readOnly(crypto.crypto),
    Crypto: util.nonEnumerable(crypto.Crypto),
    SubtleCrypto: util.nonEnumerable(crypto.SubtleCrypto),
    fetch: util.writable(fetch.fetch),
    performance: util.writable(performance.performance),
    setInterval: util.writable(timers.setInterval),
    setTimeout: util.writable(timers.setTimeout),
    structuredClone: util.writable(messagePort.structuredClone),
  };

  const unstableWindowOrWorkerGlobalScope = {
    BroadcastChannel: util.nonEnumerable(broadcastChannel.BroadcastChannel),
    WebSocketStream: util.nonEnumerable(webSocket.WebSocketStream),

    GPU: util.nonEnumerable(webgpu.GPU),
    GPUAdapter: util.nonEnumerable(webgpu.GPUAdapter),
    GPUSupportedLimits: util.nonEnumerable(webgpu.GPUSupportedLimits),
    GPUSupportedFeatures: util.nonEnumerable(webgpu.GPUSupportedFeatures),
    GPUDevice: util.nonEnumerable(webgpu.GPUDevice),
    GPUQueue: util.nonEnumerable(webgpu.GPUQueue),
    GPUBuffer: util.nonEnumerable(webgpu.GPUBuffer),
    GPUBufferUsage: util.nonEnumerable(webgpu.GPUBufferUsage),
    GPUMapMode: util.nonEnumerable(webgpu.GPUMapMode),
    GPUTexture: util.nonEnumerable(webgpu.GPUTexture),
    GPUTextureUsage: util.nonEnumerable(webgpu.GPUTextureUsage),
    GPUTextureView: util.nonEnumerable(webgpu.GPUTextureView),
    GPUSampler: util.nonEnumerable(webgpu.GPUSampler),
    GPUBindGroupLayout: util.nonEnumerable(webgpu.GPUBindGroupLayout),
    GPUPipelineLayout: util.nonEnumerable(webgpu.GPUPipelineLayout),
    GPUBindGroup: util.nonEnumerable(webgpu.GPUBindGroup),
    GPUShaderModule: util.nonEnumerable(webgpu.GPUShaderModule),
    GPUShaderStage: util.nonEnumerable(webgpu.GPUShaderStage),
    GPUComputePipeline: util.nonEnumerable(webgpu.GPUComputePipeline),
    GPURenderPipeline: util.nonEnumerable(webgpu.GPURenderPipeline),
    GPUColorWrite: util.nonEnumerable(webgpu.GPUColorWrite),
    GPUCommandEncoder: util.nonEnumerable(webgpu.GPUCommandEncoder),
    GPURenderPassEncoder: util.nonEnumerable(webgpu.GPURenderPassEncoder),
    GPUComputePassEncoder: util.nonEnumerable(webgpu.GPUComputePassEncoder),
    GPUCommandBuffer: util.nonEnumerable(webgpu.GPUCommandBuffer),
    GPURenderBundleEncoder: util.nonEnumerable(webgpu.GPURenderBundleEncoder),
    GPURenderBundle: util.nonEnumerable(webgpu.GPURenderBundle),
    GPUQuerySet: util.nonEnumerable(webgpu.GPUQuerySet),
    GPUOutOfMemoryError: util.nonEnumerable(webgpu.GPUOutOfMemoryError),
    GPUValidationError: util.nonEnumerable(webgpu.GPUValidationError),
  };

  const mainRuntimeGlobalProperties = {
    Location: location.locationConstructorDescriptor,
    location: location.locationDescriptor,
    Window: globalInterfaces.windowConstructorDescriptor,
    window: util.readOnly(globalThis),
    self: util.writable(globalThis),
    Navigator: util.nonEnumerable(Navigator),
    navigator: {
      configurable: true,
      enumerable: true,
      get: () => navigator,
    },
    close: util.writable(windowClose),
    closed: util.getterOnly(() => windowIsClosing),
    alert: util.writable(prompt.alert),
    confirm: util.writable(prompt.confirm),
    prompt: util.writable(prompt.prompt),
    localStorage: {
      configurable: true,
      enumerable: true,
      get: webStorage.localStorage,
    },
    sessionStorage: {
      configurable: true,
      enumerable: true,
      get: webStorage.sessionStorage,
    },
    Storage: util.nonEnumerable(webStorage.Storage),
  };

  const workerRuntimeGlobalProperties = {
    WorkerLocation: location.workerLocationConstructorDescriptor,
    location: location.workerLocationDescriptor,
    WorkerGlobalScope: globalInterfaces.workerGlobalScopeConstructorDescriptor,
    DedicatedWorkerGlobalScope:
      globalInterfaces.dedicatedWorkerGlobalScopeConstructorDescriptor,
    WorkerNavigator: util.nonEnumerable(WorkerNavigator),
    navigator: {
      configurable: true,
      enumerable: true,
      get: () => workerNavigator,
    },
    self: util.readOnly(globalThis),
    // TODO(bartlomieju): should be readonly?
    close: util.nonEnumerable(workerClose),
    postMessage: util.writable(postMessage),
  };

  const pendingRejections = [];
  const pendingRejectionsReasons = new SafeWeakMap();

  function promiseRejectCallback(type, promise, reason) {
    switch (type) {
      case 0: {
        ops.op_store_pending_promise_exception(promise, reason);
        ArrayPrototypePush(pendingRejections, promise);
        WeakMapPrototypeSet(pendingRejectionsReasons, promise, reason);
        break;
      }
      case 1: {
        ops.op_remove_pending_promise_exception(promise);
        const index = ArrayPrototypeIndexOf(pendingRejections, promise);
        if (index > -1) {
          ArrayPrototypeSplice(pendingRejections, index, 1);
          WeakMapPrototypeDelete(pendingRejectionsReasons, promise);
        }
        break;
      }
      default:
        return false;
    }

    return !!globalThis.onunhandledrejection ||
      eventTarget.listenerCount(globalThis, "unhandledrejection") > 0;
  }

  function promiseRejectMacrotaskCallback() {
    while (pendingRejections.length > 0) {
      const promise = ArrayPrototypeShift(pendingRejections);
      const hasPendingException = ops.op_has_pending_promise_exception(
        promise,
      );
      const reason = WeakMapPrototypeGet(pendingRejectionsReasons, promise);
      WeakMapPrototypeDelete(pendingRejectionsReasons, promise);

      if (!hasPendingException) {
        continue;
      }

      const event = new PromiseRejectionEvent("unhandledrejection", {
        cancelable: true,
        promise,
        reason,
      });

      const errorEventCb = (event) => {
        if (event.error === reason) {
          ops.op_remove_pending_promise_exception(promise);
        }
      };
      // Add a callback for "error" event - it will be dispatched
      // if error is thrown during dispatch of "unhandledrejection"
      // event.
      globalThis.addEventListener("error", errorEventCb);
      globalThis.dispatchEvent(event);
      globalThis.removeEventListener("error", errorEventCb);

      // If event was not prevented (or "unhandledrejection" listeners didn't
      // throw) we will let Rust side handle it.
      if (event.defaultPrevented) {
        ops.op_remove_pending_promise_exception(promise);
      }
    }
    return true;
  }

  let hasBootstrapped = false;

  function bootstrapMainRuntime(runtimeOptions) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }

    const {
      args,
      location: locationHref,
      noColor,
      isTty,
      pid,
      ppid,
      unstableFlag,
      cpuCount,
      userAgent: userAgentInfo,
    } = runtimeOptions;

    performance.setTimeOrigin(DateNow());
    const consoleFromV8 = window.console;
    const wrapConsole = window.__bootstrap.console.wrapConsole;

    // Remove bootstrapping data from the global scope
    delete globalThis.__bootstrap;
    delete globalThis.bootstrap;
    util.log("bootstrapMainRuntime");
    hasBootstrapped = true;

    // If the `--location` flag isn't set, make `globalThis.location` `undefined` and
    // writable, so that they can mock it themselves if they like. If the flag was
    // set, define `globalThis.location`, using the provided value.
    if (locationHref == null) {
      mainRuntimeGlobalProperties.location = {
        writable: true,
      };
    } else {
      location.setLocationHref(locationHref);
    }

    ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);
    if (runtimeOptions.unstableFlag) {
      ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
    }
    ObjectDefineProperties(globalThis, mainRuntimeGlobalProperties);
    ObjectSetPrototypeOf(globalThis, Window.prototype);

    const consoleFromDeno = globalThis.console;
    wrapConsole(consoleFromDeno, consoleFromV8);

    eventTarget.setEventTargetData(globalThis);

    defineEventHandler(window, "error");
    defineEventHandler(window, "load");
    defineEventHandler(window, "beforeunload");
    defineEventHandler(window, "unload");
    defineEventHandler(window, "unhandledrejection");

    core.setPromiseRejectCallback(promiseRejectCallback);

    const isUnloadDispatched = SymbolFor("isUnloadDispatched");
    // Stores the flag for checking whether unload is dispatched or not.
    // This prevents the recursive dispatches of unload events.
    // See https://github.com/denoland/deno/issues/9201.
    window[isUnloadDispatched] = false;
    window.addEventListener("unload", () => {
      window[isUnloadDispatched] = true;
    });

    runtimeStart(runtimeOptions);

    colors.setNoColor(noColor || !isTty);
    numCpus = cpuCount;
    userAgent = userAgentInfo;
    registerErrors();

    const internalSymbol = Symbol("Deno.internal");

    const finalDenoNs = {
      core,
      internal: internalSymbol,
      [internalSymbol]: internals,
      resources: core.resources,
      close: core.close,
      ...denoNs,
    };
    ObjectDefineProperties(finalDenoNs, {
      pid: util.readOnly(pid),
      ppid: util.readOnly(ppid),
      noColor: util.readOnly(noColor),
      args: util.readOnly(ObjectFreeze(args)),
      mainModule: util.getterOnly(opMainModule),
    });

    if (unstableFlag) {
      ObjectAssign(finalDenoNs, denoNsUnstable);
    }

    // Setup `Deno` global - we're actually overriding already existing global
    // `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));
    ObjectFreeze(globalThis.Deno.core);

    util.log("args", args);
  }

  function bootstrapWorkerRuntime(
    runtimeOptions,
    name,
    internalName,
  ) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }

    performance.setTimeOrigin(DateNow());
    const consoleFromV8 = window.console;
    const wrapConsole = window.__bootstrap.console.wrapConsole;

    // Remove bootstrapping data from the global scope
    delete globalThis.__bootstrap;
    delete globalThis.bootstrap;
    util.log("bootstrapWorkerRuntime");
    hasBootstrapped = true;
    ObjectDefineProperties(globalThis, windowOrWorkerGlobalScope);
    if (runtimeOptions.unstableFlag) {
      ObjectDefineProperties(globalThis, unstableWindowOrWorkerGlobalScope);
    }
    ObjectDefineProperties(globalThis, workerRuntimeGlobalProperties);
    ObjectDefineProperties(globalThis, { name: util.writable(name) });
    if (runtimeOptions.enableTestingFeaturesFlag) {
      ObjectDefineProperty(
        globalThis,
        "importScripts",
        util.writable(importScripts),
      );
    }
    ObjectSetPrototypeOf(globalThis, DedicatedWorkerGlobalScope.prototype);

    const consoleFromDeno = globalThis.console;
    wrapConsole(consoleFromDeno, consoleFromV8);

    eventTarget.setEventTargetData(globalThis);

    defineEventHandler(self, "message");
    defineEventHandler(self, "error", undefined, true);
    defineEventHandler(self, "unhandledrejection");

    core.setPromiseRejectCallback(promiseRejectCallback);

    // `Deno.exit()` is an alias to `self.close()`. Setting and exit
    // code using an op in worker context is a no-op.
    os.setExitHandler((_exitCode) => {
      workerClose();
    });

    runtimeStart(
      runtimeOptions,
      internalName ?? name,
    );
    const {
      unstableFlag,
      pid,
      noColor,
      isTty,
      args,
      location: locationHref,
      cpuCount,
    } = runtimeOptions;

    colors.setNoColor(noColor || !isTty);
    location.setLocationHref(locationHref);
    numCpus = cpuCount;
    registerErrors();

    globalThis.pollForMessages = pollForMessages;

    const internalSymbol = Symbol("Deno.internal");

    const finalDenoNs = {
      core,
      internal: internalSymbol,
      [internalSymbol]: internals,
      resources: core.resources,
      close: core.close,
      ...denoNs,
    };
    if (unstableFlag) {
      ObjectAssign(finalDenoNs, denoNsUnstable);
    }
    ObjectDefineProperties(finalDenoNs, {
      pid: util.readOnly(pid),
      noColor: util.readOnly(noColor),
      args: util.readOnly(ObjectFreeze(args)),
    });
    // Setup `Deno` global - we're actually overriding already
    // existing global `Deno` with `Deno` namespace from "./deno.ts".
    ObjectDefineProperty(globalThis, "Deno", util.readOnly(finalDenoNs));
    ObjectFreeze(globalThis.Deno.core);
  }

  ObjectDefineProperties(globalThis, {
    bootstrap: {
      value: {
        mainRuntime: bootstrapMainRuntime,
        workerRuntime: bootstrapWorkerRuntime,
      },
      configurable: true,
    },
  });
})(this);
