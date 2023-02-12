// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const primordials = globalThis.__bootstrap.primordials;
const {
  ObjectDefineProperties,
  SymbolFor,
} = primordials;

import * as util from "internal:runtime/js/06_util.js";
import * as location from "internal:deno_web/12_location.js";
import * as event from "internal:deno_web/02_event.js";
import * as timers from "internal:deno_web/02_timers.js";
import * as base64 from "internal:deno_web/05_base64.js";
import * as encoding from "internal:deno_web/08_text_encoding.js";
import * as console from "internal:deno_console/02_console.js";
import * as caches from "internal:deno_cache/01_cache.js";
import * as compression from "internal:deno_web/14_compression.js";
import * as worker from "internal:runtime/js/11_workers.js";
import * as performance from "internal:deno_web/15_performance.js";
import * as crypto from "internal:deno_crypto/00_crypto.js";
import * as url from "internal:deno_url/00_url.js";
import * as urlPattern from "internal:deno_url/01_urlpattern.js";
import * as headers from "internal:deno_fetch/20_headers.js";
import * as streams from "internal:deno_web/06_streams.js";
import * as fileReader from "internal:deno_web/10_filereader.js";
import * as webgpu from "internal:deno_webgpu/01_webgpu.js";
import * as webSocket from "internal:deno_websocket/01_websocket.js";
import * as webSocketStream from "internal:deno_websocket/02_websocketstream.js";
import * as broadcastChannel from "internal:deno_broadcast_channel/01_broadcast_channel.js";
import * as file from "internal:deno_web/09_file.js";
import * as formData from "internal:deno_fetch/21_formdata.js";
import * as request from "internal:deno_fetch/23_request.js";
import * as response from "internal:deno_fetch/23_response.js";
import * as fetch from "internal:deno_fetch/26_fetch.js";
import * as messagePort from "internal:deno_web/13_message_port.js";
import * as webidl from "internal:deno_webidl/00_webidl.js";
import DOMException from "internal:deno_web/01_dom_exception.js";
import * as abortSignal from "internal:deno_web/03_abort_signal.js";
import * as globalInterfaces from "internal:deno_web/04_global_interfaces.js";
import * as webStorage from "internal:deno_webstorage/01_webstorage.js";
import * as prompt from "internal:runtime/js/41_prompt.js";

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
  DOMException: util.nonEnumerable(DOMException),
  ErrorEvent: util.nonEnumerable(event.ErrorEvent),
  Event: util.nonEnumerable(event.Event),
  EventTarget: util.nonEnumerable(event.EventTarget),
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
  Request: util.nonEnumerable(request.Request),
  Response: util.nonEnumerable(response.Response),
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
    new console.Console((msg, level) => core.print(msg, level > 1)),
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
  WebSocketStream: util.nonEnumerable(webSocketStream.WebSocketStream),

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

export {
  mainRuntimeGlobalProperties,
  setLanguage,
  setNumCpus,
  setUserAgent,
  unstableWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
  workerRuntimeGlobalProperties,
};
