// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { SafeArrayIterator } = primordials;

const event = core.loadExtScript("ext:deno_web/02_event.js");
const base64 = core.loadExtScript("ext:deno_web/05_base64.js");
const encoding = core.loadExtScript("ext:deno_web/08_text_encoding.js");
const console = core.loadExtScript("ext:deno_web/01_console.js");
const worker = core.loadExtScript("ext:runtime/11_workers.js");
const performance = core.loadExtScript("ext:deno_web/15_performance.js");
const crypto = core.loadExtScript("ext:deno_crypto/00_crypto.js");
const url = core.loadExtScript("ext:deno_web/00_url.js");
const urlPattern = core.loadExtScript("ext:deno_web/01_urlpattern.js");
const headers = core.loadExtScript("ext:deno_fetch/20_headers.js");
// 06_streams.js is the 208 KB web-streams polyfill. Defer until a global
// stream class (ReadableStream/WritableStream/TransformStream/etc.) is
// accessed.
let _streamsMod;
const lazyStreams = () =>
  _streamsMod ??
    (_streamsMod = core.loadExtScript("ext:deno_web/06_streams.js"));
// caches/CacheStorage/Cache transitively pulls 06_streams via 22_body.
let _cacheMod;
const lazyCache = () =>
  _cacheMod ??
    (_cacheMod = core.loadExtScript("ext:deno_cache/01_cache.js"));
// CompressionStream/DecompressionStream pull TransformStream from 06_streams.
let _compressionMod;
const lazyCompression = () =>
  _compressionMod ??
    (_compressionMod = core.loadExtScript("ext:deno_web/14_compression.js"));
// Request/Response/fetch/EventSource each chain through 22_body -> 06_streams.
let _requestMod;
const lazyRequest = () =>
  _requestMod ??
    (_requestMod = core.loadExtScript("ext:deno_fetch/23_request.js"));
let _responseMod;
const lazyResponse = () =>
  _responseMod ??
    (_responseMod = core.loadExtScript("ext:deno_fetch/23_response.js"));
let _fetchMod;
const lazyFetch = () =>
  _fetchMod ?? (_fetchMod = core.loadExtScript("ext:deno_fetch/26_fetch.js"));
let _eventSourceMod;
const lazyEventSource = () =>
  _eventSourceMod ??
    (_eventSourceMod = core.loadExtScript("ext:deno_fetch/27_eventsource.js"));
const fileReader = core.loadExtScript("ext:deno_web/10_filereader.js");
const broadcastChannel = core.loadExtScript(
  "ext:deno_web/01_broadcast_channel.js",
);
const file = core.loadExtScript("ext:deno_web/09_file.js");
const formData = core.loadExtScript("ext:deno_fetch/21_formdata.js");
const messagePort = core.loadExtScript("ext:deno_web/13_message_port.js");
const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const {
  DOMException,
  QuotaExceededError,
} = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
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
let _imageDataMod;
const loadImageData = () =>
  _imageDataMod ??
    (_imageDataMod = core.loadExtScript("ext:deno_web/16_image_data.js"));
let _geometryMod;
const loadGeometry = () =>
  _geometryMod ??
    (_geometryMod = core.loadExtScript("ext:deno_web/17_geometry.js"));
const loadWebSocket = core.createLazyLoader(
  "ext:deno_websocket/01_websocket.js",
);
const loadWebSocketStream = core.createLazyLoader(
  "ext:deno_websocket/02_websocketstream.js",
);
const loadWebTransport = core.createLazyLoader(
  "ext:deno_web/webtransport.js",
);

// https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
const windowOrWorkerGlobalScope = {
  AbortController: core.propNonEnumerable(abortSignal.AbortController),
  AbortSignal: core.propNonEnumerable(abortSignal.AbortSignal),
  Blob: core.propNonEnumerable(file.Blob),
  BroadcastChannel: core.propNonEnumerable(broadcastChannel.BroadcastChannel),
  ByteLengthQueuingStrategy: core.propNonEnumerableLazyLoaded(
    (s) => s.ByteLengthQueuingStrategy,
    lazyStreams,
  ),
  CloseEvent: core.propNonEnumerable(event.CloseEvent),
  CompressionStream: core.propNonEnumerableLazyLoaded(
    (c) => c.CompressionStream,
    lazyCompression,
  ),
  CountQueuingStrategy: core.propNonEnumerableLazyLoaded(
    (s) => s.CountQueuingStrategy,
    lazyStreams,
  ),
  CryptoKey: core.propNonEnumerable(crypto.CryptoKey),
  CustomEvent: core.propNonEnumerable(event.CustomEvent),
  DecompressionStream: core.propNonEnumerableLazyLoaded(
    (c) => c.DecompressionStream,
    lazyCompression,
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
  File: core.propNonEnumerable(file.File),
  FileReader: core.propNonEnumerable(fileReader.FileReader),
  FormData: core.propNonEnumerable(formData.FormData),
  Headers: core.propNonEnumerable(headers.Headers),
  ImageData: core.propNonEnumerableLazyLoaded(
    (imageData) => imageData.ImageData,
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
    lazyStreams,
  ),
  ReadableStreamDefaultReader: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamDefaultReader,
    lazyStreams,
  ),
  Request: core.propNonEnumerableLazyLoaded(
    (r) => r.Request,
    lazyRequest,
  ),
  Response: core.propNonEnumerableLazyLoaded(
    (r) => r.Response,
    lazyResponse,
  ),
  TextDecoder: core.propNonEnumerable(encoding.TextDecoder),
  TextEncoder: core.propNonEnumerable(encoding.TextEncoder),
  TextDecoderStream: core.propNonEnumerable(encoding.TextDecoderStream),
  TextEncoderStream: core.propNonEnumerable(encoding.TextEncoderStream),
  TransformStream: core.propNonEnumerableLazyLoaded(
    (s) => s.TransformStream,
    lazyStreams,
  ),
  URL: core.propNonEnumerable(url.URL),
  URLPattern: core.propNonEnumerable(urlPattern.URLPattern),
  URLSearchParams: core.propNonEnumerable(url.URLSearchParams),
  WebSocket: core.propNonEnumerableLazyLoaded(
    (ws) => ws.WebSocket,
    loadWebSocket,
  ),
  MessageChannel: core.propNonEnumerable(messagePort.MessageChannel),
  MessagePort: core.propNonEnumerable(messagePort.MessagePort),
  Worker: core.propNonEnumerable(worker.Worker),
  WritableStream: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStream,
    lazyStreams,
  ),
  WritableStreamDefaultWriter: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStreamDefaultWriter,
    lazyStreams,
  ),
  WritableStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.WritableStreamDefaultController,
    lazyStreams,
  ),
  ReadableByteStreamController: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableByteStreamController,
    lazyStreams,
  ),
  ReadableStreamBYOBReader: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamBYOBReader,
    lazyStreams,
  ),
  ReadableStreamBYOBRequest: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamBYOBRequest,
    lazyStreams,
  ),
  ReadableStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.ReadableStreamDefaultController,
    lazyStreams,
  ),
  TransformStreamDefaultController: core.propNonEnumerableLazyLoaded(
    (s) => s.TransformStreamDefaultController,
    lazyStreams,
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
      return lazyCache().cacheStorage();
    },
  },
  CacheStorage: core.propNonEnumerableLazyLoaded(
    (c) => c.CacheStorage,
    lazyCache,
  ),
  Cache: core.propNonEnumerableLazyLoaded(
    (c) => c.Cache,
    lazyCache,
  ),
  console: core.propNonEnumerable(
    new console.Console((msg, level) => core.print(msg, level > 1)),
  ),
  crypto: core.propReadOnly(crypto.crypto),
  Crypto: core.propNonEnumerable(crypto.Crypto),
  SubtleCrypto: core.propNonEnumerable(crypto.SubtleCrypto),
  // `fetch` is installed as a plain data descriptor whose value is a lazy
  // wrapper function (not an accessor descriptor). node:test's `mock.method`
  // reads `descriptor.value` and rejects accessor descriptors as
  // non-mockable. The wrapper forwards to the real implementation, lazily
  // loading 26_fetch on first call.
  fetch: core.propWritable(function fetch(...args) {
    return lazyFetch().fetch(...new SafeArrayIterator(args));
  }),
  EventSource: core.propWritableLazyLoaded(
    (e) => e.EventSource,
    lazyEventSource,
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
  structuredClone: core.propWritable(messagePort.structuredClone),
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

let _cssStyleSheetMod;
const loadCssStyleSheet = () =>
  _cssStyleSheetMod ??
    (_cssStyleSheetMod = core.loadExtScript(
      "ext:deno_web/18_css_stylesheet.js",
    ));
unstableForWindowOrWorkerGlobalScope[unstableIds.rawImports] = {
  CSSRule: core.propNonEnumerableLazyLoaded(
    (css) => css.CSSRule,
    loadCssStyleSheet,
  ),
  CSSStyleSheet: core.propNonEnumerableLazyLoaded(
    (css) => css.CSSStyleSheet,
    loadCssStyleSheet,
  ),
};

export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };
