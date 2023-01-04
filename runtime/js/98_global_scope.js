// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = Deno.core;
  const {
    ObjectDefineProperties,
    SymbolFor,
  } = window.__bootstrap.primordials;

  const util = window.__bootstrap.util;
  const location = window.__bootstrap.location;
  const event = window.__bootstrap.event;
  const eventTarget = window.__bootstrap.eventTarget;
  const timers = window.__bootstrap.timers;
  const base64 = window.__bootstrap.base64;
  const encoding = window.__bootstrap.encoding;
  const Console = window.__bootstrap.console.Console;
  const caches = window.__bootstrap.caches;
  const compression = window.__bootstrap.compression;
  const worker = window.__bootstrap.worker;
  const performance = window.__bootstrap.performance;
  const crypto = window.__bootstrap.crypto;
  const url = window.__bootstrap.url;
  const urlPattern = window.__bootstrap.urlPattern;
  const headers = window.__bootstrap.headers;
  const streams = window.__bootstrap.streams;
  const fileReader = window.__bootstrap.fileReader;
  const webgpu = window.__bootstrap.webgpu;
  const webSocket = window.__bootstrap.webSocket;
  const broadcastChannel = window.__bootstrap.broadcastChannel;
  const file = window.__bootstrap.file;
  const formData = window.__bootstrap.formData;
  const fetch = window.__bootstrap.fetch;
  const messagePort = window.__bootstrap.messagePort;
  const webidl = window.__bootstrap.webidl;
  const domException = window.__bootstrap.domException;
  const abortSignal = window.__bootstrap.abortSignal;
  const globalInterfaces = window.__bootstrap.globalInterfaces;
  const webStorage = window.__bootstrap.webStorage;
  const prompt = window.__bootstrap.prompt;

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScope = {
    AbortController: util.nonEnumerable(abortSignal.AbortController),
    AbortSignal: util.nonEnumerable(abortSignal.AbortSignal),
    Blob: util.nonEnumerable(file.Blob),
    ByteLengthQueuingStrategy: util.nonEnumerable(
      streams.ByteLengthQueuingStrategy,
    ),
    CloseEvent: util.nonEnumerable(event.CloseEvent),
    CompressionStream: util.nonEnumerable(compression.CompressionStream),
    CountQueuingStrategy: util.nonEnumerable(
      streams.CountQueuingStrategy,
    ),
    CryptoKey: util.nonEnumerable(crypto.CryptoKey),
    CustomEvent: util.nonEnumerable(event.CustomEvent),
    DecompressionStream: util.nonEnumerable(compression.DecompressionStream),
    DOMException: util.nonEnumerable(domException.DOMException),
    ErrorEvent: util.nonEnumerable(event.ErrorEvent),
    Event: util.nonEnumerable(event.Event),
    EventTarget: util.nonEnumerable(eventTarget.EventTarget),
    File: util.nonEnumerable(file.File),
    FileReader: util.nonEnumerable(fileReader.FileReader),
    FormData: util.nonEnumerable(formData.FormData),
    Headers: util.nonEnumerable(headers.Headers),
    MessageEvent: util.nonEnumerable(event.MessageEvent),
    Performance: util.nonEnumerable(performance.Performance),
    PerformanceEntry: util.nonEnumerable(performance.PerformanceEntry),
    PerformanceMark: util.nonEnumerable(performance.PerformanceMark),
    PerformanceMeasure: util.nonEnumerable(performance.PerformanceMeasure),
    PromiseRejectionEvent: util.nonEnumerable(event.PromiseRejectionEvent),
    ProgressEvent: util.nonEnumerable(event.ProgressEvent),
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
    caches: {
      enumerable: true,
      configurable: true,
      get: caches.cacheStorage,
    },
    CacheStorage: util.nonEnumerable(caches.CacheStorage),
    Cache: util.nonEnumerable(caches.Cache),
    console: util.nonEnumerable(
      new Console((msg, level) => core.print(msg, level > 1)),
    ),
    crypto: util.readOnly(crypto.crypto),
    Crypto: util.nonEnumerable(crypto.Crypto),
    SubtleCrypto: util.nonEnumerable(crypto.SubtleCrypto),
    fetch: util.writable(fetch.fetch),
    performance: util.writable(performance.performance),
    reportError: util.writable(event.reportError),
    setInterval: util.writable(timers.setInterval),
    setTimeout: util.writable(timers.setTimeout),
    structuredClone: util.writable(messagePort.structuredClone),
    // Branding as a WebIDL object
    [webidl.brand]: util.nonEnumerable(webidl.brand),
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

  class Navigator {
    constructor() {
      webidl.illegalConstructor();
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect) {
      return `${this.constructor.name} ${inspect({})}`;
    }
  }

  const navigator = webidl.createBranded(Navigator);

  let numCpus, userAgent, language;

  function setNumCpus(val) {
    numCpus = val;
  }

  function setUserAgent(val) {
    userAgent = val;
  }

  function setLanguage(val) {
    language = val;
  }

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
    language: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, NavigatorPrototype);
        return language;
      },
    },
    languages: {
      configurable: true,
      enumerable: true,
      get() {
        webidl.assertBranded(this, NavigatorPrototype);
        return [language];
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
      language: {
        configurable: true,
        enumerable: true,
        get() {
          webidl.assertBranded(this, WorkerNavigatorPrototype);
          return language;
        },
      },
      languages: {
        configurable: true,
        enumerable: true,
        get() {
          webidl.assertBranded(this, WorkerNavigatorPrototype);
          return [language];
        },
      },
    },
  });
  const WorkerNavigatorPrototype = WorkerNavigator.prototype;

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
    alert: util.writable(prompt.alert),
    confirm: util.writable(prompt.confirm),
    prompt: util.writable(prompt.prompt),
    localStorage: {
      configurable: true,
      enumerable: true,
      get: webStorage.localStorage,
      // Makes this reassignable to make astro work
      set: () => {},
    },
    sessionStorage: {
      configurable: true,
      enumerable: true,
      get: webStorage.sessionStorage,
      // Makes this reassignable to make astro work
      set: () => {},
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
  };

  window.__bootstrap.globalScope = {
    windowOrWorkerGlobalScope,
    unstableWindowOrWorkerGlobalScope,
    mainRuntimeGlobalProperties,
    workerRuntimeGlobalProperties,

    setNumCpus,
    setUserAgent,
    setLanguage,
  };
})(this);
