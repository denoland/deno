// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";

import * as util from "ext:runtime/06_util.js";
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
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import { webgpu, webGPUNonEnumerable } from "ext:deno_webgpu/00_init.js";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";
import { unstableIds } from "ext:runtime/90_deno_ns.js";

const { op_lazy_load_esm } = core.ensureFastOps(true);
let image;

function ImageNonEnumerable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadImage();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadImage();

      valueIsSet = true;
      value = v;
    },
    enumerable: false,
    configurable: true,
  };
}
function ImageWritable(getter) {
  let valueIsSet = false;
  let value;

  return {
    get() {
      loadImage();

      if (valueIsSet) {
        return value;
      } else {
        return getter();
      }
    },
    set(v) {
      loadImage();

      valueIsSet = true;
      value = v;
    },
    enumerable: true,
    configurable: true,
  };
}
function loadImage() {
  if (!image) {
    image = op_lazy_load_esm("ext:deno_canvas/01_image.js");
  }
}

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
  ImageData: ImageNonEnumerable(() => image.ImageData),
  ImageBitmap: ImageNonEnumerable(() => image.ImageBitmap),
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
  createImageBitmap: ImageWritable(() => image.createImageBitmap),
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
  GPUCanvasContext: webGPUNonEnumerable(() => webgpuSurface.GPUCanvasContext),
};

export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };
