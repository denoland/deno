// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = Deno.core;
const {
  ObjectDefineProperties,
  SymbolFor,
} = globalThis.__bootstrap.primordials;

const util = globalThis.__bootstrap.util;
const location = globalThis.__bootstrap.location;
const event = globalThis.__bootstrap.event;
const eventTarget = globalThis.__bootstrap.eventTarget;
const timers = globalThis.__bootstrap.timers;
const base64 = globalThis.__bootstrap.base64;
const encoding = globalThis.__bootstrap.encoding;
const Console = globalThis.__bootstrap.console.Console;
const caches = globalThis.__bootstrap.caches;
const compression = globalThis.__bootstrap.compression;
const worker = globalThis.__bootstrap.worker;
const performance = globalThis.__bootstrap.performance;
const crypto = globalThis.__bootstrap.crypto;
import * as url from "deno:ext/url/00_url.js";
import * as urlPattern from "deno:ext/url/01_urlpattern.js";
const headers = globalThis.__bootstrap.headers;
const streams = globalThis.__bootstrap.streams;
const fileReader = globalThis.__bootstrap.fileReader;
const webgpu = globalThis.__bootstrap.webgpu;
const webSocket = globalThis.__bootstrap.webSocket;
const broadcastChannel = globalThis.__bootstrap.broadcastChannel;
const file = globalThis.__bootstrap.file;
const formData = globalThis.__bootstrap.formData;
const fetch = globalThis.__bootstrap.fetch;
const messagePort = globalThis.__bootstrap.messagePort;
const webidl = globalThis.__bootstrap.webidl;
const domException = globalThis.__bootstrap.domException;
const abortSignal = globalThis.__bootstrap.abortSignal;
const globalInterfaces = globalThis.__bootstrap.globalInterfaces;
const webStorage = globalThis.__bootstrap.webStorage;
const prompt = globalThis.__bootstrap.prompt;

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
  GPUAdapterInfo: util.nonEnumerable(webgpu.GPUAdapterInfo),
  GPUSupportedLimits: util.nonEnumerable(webgpu.GPUSupportedLimits),
  GPUSupportedFeatures: util.nonEnumerable(webgpu.GPUSupportedFeatures),
  GPUDeviceLostInfo: util.nonEnumerable(webgpu.GPUDeviceLostInfo),
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
  GPUError: util.nonEnumerable(webgpu.GPUError),
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
  window: util.getterOnly(() => globalThis),
  self: util.getterOnly(() => globalThis),
  Navigator: util.nonEnumerable(Navigator),
  navigator: util.getterOnly(() => navigator),
  alert: util.writable(prompt.alert),
  confirm: util.writable(prompt.confirm),
  prompt: util.writable(prompt.prompt),
  localStorage: util.getterOnly(webStorage.localStorage),
  sessionStorage: util.getterOnly(webStorage.sessionStorage),
  Storage: util.nonEnumerable(webStorage.Storage),
};

const workerRuntimeGlobalProperties = {
  WorkerLocation: location.workerLocationConstructorDescriptor,
  location: location.workerLocationDescriptor,
  WorkerGlobalScope: globalInterfaces.workerGlobalScopeConstructorDescriptor,
  DedicatedWorkerGlobalScope:
    globalInterfaces.dedicatedWorkerGlobalScopeConstructorDescriptor,
  WorkerNavigator: util.nonEnumerable(WorkerNavigator),
  navigator: util.getterOnly(() => workerNavigator),
  self: util.getterOnly(() => globalThis),
};

globalThis.__bootstrap.globalScope = {
  windowOrWorkerGlobalScope,
  unstableWindowOrWorkerGlobalScope,
  mainRuntimeGlobalProperties,
  workerRuntimeGlobalProperties,

  setNumCpus,
  setUserAgent,
  setLanguage,
};
