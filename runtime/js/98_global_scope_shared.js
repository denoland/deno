// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

// Eager modules: required at snapshot time by 99_main.js or by other eager
// modules. webidl is used by every class IIFE; event/DOMException/console/
// performance are used at snapshot time by 99_main.js itself; base64 has
// trivial atob/btoa values so laziness has near-zero benefit.
const event = core.loadExtScript("ext:deno_web/02_event.js");
const base64 = core.loadExtScript("ext:deno_web/05_base64.js");
const console = core.loadExtScript("ext:deno_web/01_console.js");
const performance = core.loadExtScript("ext:deno_web/15_performance.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  DOMException,
  QuotaExceededError,
} = core.loadExtScript("ext:deno_web/01_dom_exception.js");

// Lazy module loaders for the big web-platform surfaces. Each module's IIFE
// runs only when one of its exposed globals is first accessed.
const loadEncoding = () =>
  core.loadExtScript("ext:deno_web/08_text_encoding.js");
const loadCrypto = () => core.loadExtScript("ext:deno_crypto/00_crypto.js");
const loadUrl = () => core.loadExtScript("ext:deno_web/00_url.js");
const loadHeaders = () => core.loadExtScript("ext:deno_fetch/20_headers.js");
const loadStreams = () => core.loadExtScript("ext:deno_web/06_streams.js");
const loadFile = () => core.loadExtScript("ext:deno_web/09_file.js");
const loadRequest = () => core.loadExtScript("ext:deno_fetch/23_request.js");
const loadResponse = () => core.loadExtScript("ext:deno_fetch/23_response.js");
const loadFetch = () => core.loadExtScript("ext:deno_fetch/26_fetch.js");
const loadAbortSignal = () =>
  core.loadExtScript("ext:deno_web/03_abort_signal.js");
import process from "node:process";
import { Buffer } from "node:buffer";
import {
  clearImmediate,
  clearInterval as nodeClearInterval,
  clearTimeout as nodeClearTimeout,
  setImmediate,
  setInterval as nodeSetInterval,
  setTimeout as nodeSetTimeout,
} from "node:timers";
const { loadWebGPU } = core.loadExtScript("ext:deno_webgpu/00_init.js");
import { unstableIds } from "ext:runtime/90_deno_ns.js";

const loadImage = core.createLazyLoader("ext:deno_image/01_image.js");
const loadCanvas = core.createLazyLoader("ext:deno_canvas/01_canvas.js");
const loadGeometry = core.createLazyLoader("ext:deno_web/geometry.js");
const loadWebSocket = core.createLazyLoader(
  "ext:deno_websocket/01_websocket.js",
);
const loadWebSocketStream = core.createLazyLoader(
  "ext:deno_websocket/02_websocketstream.js",
);
const loadWebTransport = core.createLazyLoader(
  "ext:deno_web/webtransport.js",
);
const loadBroadcastChannel = () =>
  core.loadExtScript("ext:deno_web/01_broadcast_channel.js");
const loadEventSource = () =>
  core.loadExtScript("ext:deno_fetch/27_eventsource.js");
const loadFileReader = () =>
  core.loadExtScript("ext:deno_web/10_filereader.js");
const loadImageData = () =>
  core.loadExtScript("ext:deno_web/16_image_data.js");
const loadCaches = () => core.loadExtScript("ext:deno_cache/01_cache.js");
const loadCompression = () =>
  core.loadExtScript("ext:deno_web/14_compression.js");
const loadWorker = () => core.loadExtScript("ext:runtime/11_workers.js");
const loadUrlPattern = () =>
  core.loadExtScript("ext:deno_web/01_urlpattern.js");
const loadFormData = () =>
  core.loadExtScript("ext:deno_fetch/21_formdata.js");
const loadMessagePort = () =>
  core.loadExtScript("ext:deno_web/13_message_port.js");

// https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
const windowOrWorkerGlobalScope = {
  AbortController: core.propNonEnumerableLazyLoaded(
    (a) => a.AbortController,
    loadAbortSignal,
  ),
  AbortSignal: core.propNonEnumerableLazyLoaded(
    (a) => a.AbortSignal,
    loadAbortSignal,
  ),
  Blob: core.propNonEnumerableLazyLoaded((f) => f.Blob, loadFile),
  BroadcastChannel: core.propNonEnumerableLazyLoaded(
    (bc) => bc.BroadcastChannel,
    loadBroadcastChannel,
  ),
  ByteLengthQueuingStrategy: core.propNonEnumerableLazyLoaded(
    (s) => s.ByteLengthQueuingStrategy,
    loadStreams,
  ),
  CloseEvent: core.propNonEnumerable(event.CloseEvent),
  CompressionStream: core.propNonEnumerableLazyLoaded(
    (c) => c.CompressionStream,
    loadCompression,
  ),
  CountQueuingStrategy: core.propNonEnumerableLazyLoaded(
    (s) => s.CountQueuingStrategy,
    loadStreams,
  ),
  CryptoKey: core.propNonEnumerableLazyLoaded(
    (c) => c.CryptoKey,
    loadCrypto,
  ),
  CustomEvent: core.propNonEnumerable(event.CustomEvent),
  DecompressionStream: core.propNonEnumerableLazyLoaded(
    (c) => c.DecompressionStream,
    loadCompression,
  ),
  DOMException: core.propNonEnumerable(DOMException),
  QuotaExceededError: core.propNonEnumerable(QuotaExceededError),
  DOMMatrix: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrix,
    loadGeometry,
  ),
  DOMMatrixReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrixReadOnly,
    loadGeometry,
  ),
  DOMPoint: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPoint,
    loadGeometry,
  ),
  DOMPointReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPointReadOnly,
    loadGeometry,
  ),
  DOMQuad: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMQuad,
    loadGeometry,
  ),
  DOMRect: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRect,
    loadGeometry,
  ),
  DOMRectReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRectReadOnly,
    loadGeometry,
  ),
  ErrorEvent: core.propNonEnumerable(event.ErrorEvent),
  Event: core.propNonEnumerable(event.Event),
  EventTarget: core.propNonEnumerable(event.EventTarget),
  File: core.propNonEnumerableLazyLoaded((f) => f.File, loadFile),
  FileReader: core.propNonEnumerableLazyLoaded(
    (fr) => fr.FileReader,
    loadFileReader,
  ),
  FormData: core.propNonEnumerableLazyLoaded(
    (fd) => fd.FormData,
    loadFormData,
  ),
  Headers: core.propNonEnumerableLazyLoaded((h) => h.Headers, loadHeaders),
  ImageData: core.propNonEnumerableLazyLoaded(
    (id) => id.ImageData,
    loadImageData,
  ),
  ImageBitmap: core.propNonEnumerableLazyLoaded(
    (image) => image.ImageBitmap,
    loadImage,
  ),
  MessageEvent: core.propNonEnumerable(event.MessageEvent),
  Performance: core.propNonEnumerable(performance.Performance),
  PerformanceEntry: core.propNonEnumerable(performance.PerformanceEntry),
  PerformanceMark: core.propNonEnumerable(performance.PerformanceMark),
  PerformanceMeasure: core.propNonEnumerable(performance.PerformanceMeasure),
  PerformanceObserver: core.propNonEnumerable(performance.PerformanceObserver),
  PerformanceObserverEntryList: core.propNonEnumerable(
    performance.PerformanceObserverEntryList,
  ),
  PromiseRejectionEvent: core.propNonEnumerable(event.PromiseRejectionEvent),
  ProgressEvent: core.propNonEnumerable(event.ProgressEvent),
  ReadableStream: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStream,
    loadStreams,
  ),
  ReadableStreamDefaultReader: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamDefaultReader,
    loadStreams,
  ),
  Request: core.propNonEnumerableLazyLoaded((r) => r.Request, loadRequest),
  Response: core.propNonEnumerableLazyLoaded((r) => r.Response, loadResponse),
  TextDecoder: core.propNonEnumerableLazyLoaded(
    (e) => e.TextDecoder,
    loadEncoding,
  ),
  TextEncoder: core.propNonEnumerableLazyLoaded(
    (e) => e.TextEncoder,
    loadEncoding,
  ),
  TextDecoderStream: core.propNonEnumerableLazyLoaded(
    (e) => e.TextDecoderStream,
    loadEncoding,
  ),
  TextEncoderStream: core.propNonEnumerableLazyLoaded(
    (e) => e.TextEncoderStream,
    loadEncoding,
  ),
  TransformStream: core.propNonEnumerableLazyLoaded(
    (s) => s.TransformStream,
    loadStreams,
  ),
  URL: core.propNonEnumerableLazyLoaded((u) => u.URL, loadUrl),
  URLPattern: core.propNonEnumerableLazyLoaded(
    (p) => p.URLPattern,
    loadUrlPattern,
  ),
  URLSearchParams: core.propNonEnumerableLazyLoaded(
    (u) => u.URLSearchParams,
    loadUrl,
  ),
  WebSocket: core.propNonEnumerableLazyLoaded(
    (ws) => ws.WebSocket,
    loadWebSocket,
  ),
  MessageChannel: core.propNonEnumerableLazyLoaded(
    (mp) => mp.MessageChannel,
    loadMessagePort,
  ),
  MessagePort: core.propNonEnumerableLazyLoaded(
    (mp) => mp.MessagePort,
    loadMessagePort,
  ),
  Worker: core.propNonEnumerableLazyLoaded(
    (w) => w.Worker,
    loadWorker,
  ),
  WritableStream: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStream,
    loadStreams,
  ),
  WritableStreamDefaultWriter: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStreamDefaultWriter,
    loadStreams,
  ),
  WritableStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStreamDefaultController,
    loadStreams,
  ),
  ReadableByteStreamController: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableByteStreamController,
    loadStreams,
  ),
  ReadableStreamBYOBReader: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamBYOBReader,
    loadStreams,
  ),
  ReadableStreamBYOBRequest: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamBYOBRequest,
    loadStreams,
  ),
  ReadableStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamDefaultController,
    loadStreams,
  ),
  TransformStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.TransformStreamDefaultController,
    loadStreams,
  ),
  atob: core.propWritable(base64.atob),
  btoa: core.propWritable(base64.btoa),
  createImageBitmap: core.propWritableLazyLoaded(
    (image) => image.createImageBitmap,
    loadImage,
  ),
  clearInterval: core.propWritable(nodeClearInterval),
  clearTimeout: core.propWritable(nodeClearTimeout),
  caches: {
    enumerable: true,
    configurable: true,
    get() {
      return loadCaches().cacheStorage();
    },
  },
  CacheStorage: core.propNonEnumerableLazyLoaded(
    (c) => c.CacheStorage,
    loadCaches,
  ),
  Cache: core.propNonEnumerableLazyLoaded(
    (c) => c.Cache,
    loadCaches,
  ),
  console: core.propNonEnumerable(
    new console.Console((msg, level) => core.print(msg, level > 1)),
  ),
  crypto: {
    __proto__: null,
    enumerable: true,
    configurable: true,
    get() {
      return loadCrypto().crypto;
    },
  },
  Crypto: core.propNonEnumerableLazyLoaded((c) => c.Crypto, loadCrypto),
  SubtleCrypto: core.propNonEnumerableLazyLoaded(
    (c) => c.SubtleCrypto,
    loadCrypto,
  ),
  fetch: core.propWritableLazyLoaded((f) => f.fetch, loadFetch),
  EventSource: core.propWritableLazyLoaded(
    (es) => es.EventSource,
    loadEventSource,
  ),
  performance: core.propWritable(performance.performance),
  process: core.propWritable(process),
  setImmediate: core.propWritable(setImmediate),
  clearImmediate: core.propWritable(clearImmediate),
  Buffer: core.propWritable(Buffer),
  global: core.propWritable(globalThis),
  reportError: core.propWritable(event.reportError),
  setInterval: core.propWritable(nodeSetInterval),
  setTimeout: core.propWritable(nodeSetTimeout),
  structuredClone: core.propWritableLazyLoaded(
    (mp) => mp.structuredClone,
    loadMessagePort,
  ),
  // Branding as a WebIDL object
  [webidl.brand]: core.propNonEnumerable(webidl.brand),

  OffscreenCanvas: core.propNonEnumerableLazyLoaded(
    (canvas) => canvas.OffscreenCanvas,
    loadCanvas,
  ),
  ImageBitmapRenderingContext: core.propNonEnumerableLazyLoaded(
    (canvas) => canvas.ImageBitmapRenderingContext,
    loadCanvas,
  ),
  GPU: core.propNonEnumerableLazyLoaded((webgpu) => webgpu.GPU, loadWebGPU),
  GPUAdapter: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUAdapter,
    loadWebGPU,
  ),
  GPUAdapterInfo: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUAdapterInfo,
    loadWebGPU,
  ),
  GPUBuffer: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBuffer,
    loadWebGPU,
  ),
  GPUBufferUsage: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBufferUsage,
    loadWebGPU,
  ),
  GPUCanvasContext: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCanvasContext,
    loadWebGPU,
  ),
  GPUColorWrite: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUColorWrite,
    loadWebGPU,
  ),
  GPUCommandBuffer: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCommandBuffer,
    loadWebGPU,
  ),
  GPUCommandEncoder: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCommandEncoder,
    loadWebGPU,
  ),
  GPUCompilationInfo: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCompilationInfo,
    loadWebGPU,
  ),
  GPUCompilationMessage: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUCompilationMessage,
    loadWebGPU,
  ),
  GPUComputePassEncoder: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUComputePassEncoder,
    loadWebGPU,
  ),
  GPUComputePipeline: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUComputePipeline,
    loadWebGPU,
  ),
  GPUDevice: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUDevice,
    loadWebGPU,
  ),
  GPUDeviceLostInfo: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUDeviceLostInfo,
    loadWebGPU,
  ),
  GPUError: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUError,
    loadWebGPU,
  ),
  GPUBindGroup: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBindGroup,
    loadWebGPU,
  ),
  GPUBindGroupLayout: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUBindGroupLayout,
    loadWebGPU,
  ),
  GPUInternalError: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUInternalError,
    loadWebGPU,
  ),
  GPUPipelineError: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUPipelineError,
    loadWebGPU,
  ),
  GPUUncapturedErrorEvent: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUUncapturedErrorEvent,
    loadWebGPU,
  ),
  GPUPipelineLayout: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUPipelineLayout,
    loadWebGPU,
  ),
  GPUQueue: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUQueue,
    loadWebGPU,
  ),
  GPUQuerySet: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUQuerySet,
    loadWebGPU,
  ),
  GPUMapMode: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUMapMode,
    loadWebGPU,
  ),
  GPUOutOfMemoryError: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUOutOfMemoryError,
    loadWebGPU,
  ),
  GPURenderBundle: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderBundle,
    loadWebGPU,
  ),
  GPURenderBundleEncoder: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderBundleEncoder,
    loadWebGPU,
  ),
  GPURenderPassEncoder: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderPassEncoder,
    loadWebGPU,
  ),
  GPURenderPipeline: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPURenderPipeline,
    loadWebGPU,
  ),
  GPUSampler: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSampler,
    loadWebGPU,
  ),
  GPUShaderModule: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUShaderModule,
    loadWebGPU,
  ),
  GPUShaderStage: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUShaderStage,
    loadWebGPU,
  ),
  GPUSupportedFeatures: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSupportedFeatures,
    loadWebGPU,
  ),
  GPUSupportedLimits: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUSupportedLimits,
    loadWebGPU,
  ),
  GPUTexture: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTexture,
    loadWebGPU,
  ),
  GPUTextureView: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTextureView,
    loadWebGPU,
  ),
  GPUTextureUsage: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUTextureUsage,
    loadWebGPU,
  ),
  GPUValidationError: core.propNonEnumerableLazyLoaded(
    (webgpu) => webgpu.GPUValidationError,
    loadWebGPU,
  ),
};

const unstableForWindowOrWorkerGlobalScope = { __proto__: null };
unstableForWindowOrWorkerGlobalScope[unstableIds.net] = {
  WebSocketStream: core.propNonEnumerableLazyLoaded(
    (wss) => wss.WebSocketStream,
    loadWebSocketStream,
  ),
  WebSocketError: core.propNonEnumerableLazyLoaded(
    (wss) => wss.WebSocketError,
    loadWebSocketStream,
  ),
  WebTransport: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransport,
    loadWebTransport,
  ),
  WebTransportBidirectionalStream: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportBidirectionalStream,
    loadWebTransport,
  ),
  WebTransportDatagramDuplexStream: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportDatagramDuplexStream,
    loadWebTransport,
  ),
  WebTransportReceiveStream: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportReceiveStream,
    loadWebTransport,
  ),
  WebTransportSendGroup: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportSendGroup,
    loadWebTransport,
  ),
  WebTransportSendStream: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportSendStream,
    loadWebTransport,
  ),
  WebTransportError: core.propNonEnumerableLazyLoaded(
    (wt) => wt.WebTransportError,
    loadWebTransport,
  ),
};

unstableForWindowOrWorkerGlobalScope[unstableIds.nodeGlobals] = {
  clearInterval: core.propWritable(nodeClearInterval),
  clearTimeout: core.propWritable(nodeClearTimeout),
  setInterval: core.propWritable(nodeSetInterval),
  setTimeout: core.propWritable(nodeSetTimeout),
};
unstableForWindowOrWorkerGlobalScope[unstableIds.webgpu] = {};

export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };
