// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

import * as util from "ext:runtime/06_util.js";
import * as location from "ext:deno_web/12_location.js";
import * as event from "ext:deno_web/02_event.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as base64 from "ext:deno_web/05_base64.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import * as console from "ext:deno_console/01_console.js";
import * as caches from "ext:deno_cache/01_cache.js";
import * as compression from "ext:deno_web/14_compression.js";
import * as worker from "ext:runtime/11_workers.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as crypto from "ext:deno_crypto/00_crypto.js";
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";
import * as headers from "ext:deno_fetch/20_headers.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as fileReader from "ext:deno_web/10_filereader.js";
import * as webSocket from "ext:deno_websocket/01_websocket.js";
import * as webSocketStream from "ext:deno_websocket/02_websocketstream.js";
import * as broadcastChannel from "ext:deno_broadcast_channel/01_broadcast_channel.js";
import * as file from "ext:deno_web/09_file.js";
import * as formData from "ext:deno_fetch/21_formdata.js";
import * as request from "ext:deno_fetch/23_request.js";
import * as response from "ext:deno_fetch/23_response.js";
import * as fetch from "ext:deno_fetch/26_fetch.js";
import * as eventSource from "ext:deno_fetch/27_eventsource.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as webStorage from "ext:deno_webstorage/01_webstorage.js";
import * as prompt from "ext:runtime/41_prompt.js";
import * as imageData from "ext:deno_web/16_image_data.js";
import { unstableIds } from "ext:runtime/90_deno_ns.js";

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
  ImageData: util.nonEnumerable(imageData.ImageData),
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
  EventSource: util.writable(eventSource.EventSource),
  performance: util.writable(performance.performance),
  reportError: util.writable(event.reportError),
  setInterval: util.writable(timers.setInterval),
  setTimeout: util.writable(timers.setTimeout),
  structuredClone: util.writable(messagePort.structuredClone),
  // Branding as a WebIDL object
  [webidl.brand]: util.nonEnumerable(webidl.brand),
};

const unstableForWindowOrWorkerGlobalScope = {};
unstableForWindowOrWorkerGlobalScope[unstableIds.broadcastChannel] = {
  BroadcastChannel: util.nonEnumerable(broadcastChannel.BroadcastChannel),
};
unstableForWindowOrWorkerGlobalScope[unstableIds.net] = {
  WebSocketStream: util.nonEnumerable(webSocketStream.WebSocketStream),
};
unstableForWindowOrWorkerGlobalScope[unstableIds.webgpu] = {
  GPU: webGPUNonEnumerable(() => webgpu.GPU),
  GPUAdapter: webGPUNonEnumerable(() => webgpu.GPUAdapter),
  GPUAdapterInfo: webGPUNonEnumerable(() => webgpu.GPUAdapterInfo),
  GPUSupportedLimits: webGPUNonEnumerable(() => webgpu.GPUSupportedLimits),
  GPUSupportedFeatures: webGPUNonEnumerable(() => webgpu.GPUSupportedFeatures),
  GPUDeviceLostInfo: webGPUNonEnumerable(() => webgpu.GPUDeviceLostInfo),
  GPUDevice: webGPUNonEnumerable(() => webgpu.GPUDevice),
  GPUQueue: webGPUNonEnumerable(() => webgpu.GPUQueue),
  GPUBuffer: webGPUNonEnumerable(() => webgpu.GPUBuffer),
  GPUBufferUsage: webGPUNonEnumerable(() => webgpu.GPUBufferUsage),
  GPUMapMode: webGPUNonEnumerable(() => webgpu.GPUMapMode),
  GPUTextureUsage: webGPUNonEnumerable(() => webgpu.GPUTextureUsage),
  GPUTexture: webGPUNonEnumerable(() => webgpu.GPUTexture),
  GPUTextureView: webGPUNonEnumerable(() => webgpu.GPUTextureView),
  GPUSampler: webGPUNonEnumerable(() => webgpu.GPUSampler),
  GPUBindGroupLayout: webGPUNonEnumerable(() => webgpu.GPUBindGroupLayout),
  GPUPipelineLayout: webGPUNonEnumerable(() => webgpu.GPUPipelineLayout),
  GPUBindGroup: webGPUNonEnumerable(() => webgpu.GPUBindGroup),
  GPUShaderModule: webGPUNonEnumerable(() => webgpu.GPUShaderModule),
  GPUShaderStage: webGPUNonEnumerable(() => webgpu.GPUShaderStage),
  GPUComputePipeline: webGPUNonEnumerable(() => webgpu.GPUComputePipeline),
  GPURenderPipeline: webGPUNonEnumerable(() => webgpu.GPURenderPipeline),
  GPUColorWrite: webGPUNonEnumerable(() => webgpu.GPUColorWrite),
  GPUCommandEncoder: webGPUNonEnumerable(() => webgpu.GPUCommandEncoder),
  GPURenderPassEncoder: webGPUNonEnumerable(() => webgpu.GPURenderPassEncoder),
  GPUComputePassEncoder: webGPUNonEnumerable(() =>
    webgpu.GPUComputePassEncoder
  ),
  GPUCommandBuffer: webGPUNonEnumerable(() => webgpu.GPUCommandBuffer),
  GPURenderBundleEncoder: webGPUNonEnumerable(() =>
    webgpu.GPURenderBundleEncoder
  ),
  GPURenderBundle: webGPUNonEnumerable(() => webgpu.GPURenderBundle),
  GPUQuerySet: webGPUNonEnumerable(() => webgpu.GPUQuerySet),
  GPUError: webGPUNonEnumerable(() => webgpu.GPUError),
  GPUValidationError: webGPUNonEnumerable(() => webgpu.GPUValidationError),
  GPUOutOfMemoryError: webGPUNonEnumerable(() => webgpu.GPUOutOfMemoryError),
};

class Navigator {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(NavigatorPrototype, this),
        keys: [
          "hardwareConcurrency",
          "userAgent",
          "language",
          "languages",
        ],
      }),
      inspectOptions,
    );
  }
}

const navigator = webidl.createBranded(Navigator);

function memoizeLazy(f) {
  let v_ = null;
  return () => {
    if (v_ === null) {
      v_ = f();
    }
    return v_;
  };
}

const numCpus = memoizeLazy(() => ops.op_bootstrap_numcpus());
const userAgent = memoizeLazy(() => ops.op_bootstrap_user_agent());
const language = memoizeLazy(() => ops.op_bootstrap_language());

let webgpu;

function webGPUNonEnumerable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadWebGPU();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadWebGPU();

      valueIsSet = true;
      value = v;
    },
    enumerable: false,
    configurable: true,
  };
}

function loadWebGPU() {
  if (!webgpu) {
    webgpu = ops.op_lazy_load_esm("ext:deno_webgpu/01_webgpu.js");
  }
}

ObjectDefineProperties(Navigator.prototype, {
  gpu: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return language();
    },
  },
  languages: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, NavigatorPrototype);
      return [language()];
    },
  },
});
const NavigatorPrototype = Navigator.prototype;

class WorkerNavigator {
  constructor() {
    webidl.illegalConstructor();
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      console.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WorkerNavigatorPrototype, this),
        keys: [
          "hardwareConcurrency",
          "userAgent",
          "language",
          "languages",
        ],
      }),
      inspectOptions,
    );
  }
}

const workerNavigator = webidl.createBranded(WorkerNavigator);

ObjectDefineProperties(WorkerNavigator.prototype, {
  gpu: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      loadWebGPU();
      return webgpu.gpu;
    },
  },
  hardwareConcurrency: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return numCpus();
    },
  },
  userAgent: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return userAgent();
    },
  },
  language: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return language();
    },
  },
  languages: {
    configurable: true,
    enumerable: true,
    get() {
      webidl.assertBranded(this, WorkerNavigatorPrototype);
      return [language()];
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
  memoizeLazy,
  unstableForWindowOrWorkerGlobalScope,
  windowOrWorkerGlobalScope,
  workerRuntimeGlobalProperties,
};
