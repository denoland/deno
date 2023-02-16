// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as asyncWrap from "internal:deno_node/polyfills/internal_binding/async_wrap.ts";
import * as buffer from "internal:deno_node/polyfills/internal_binding/buffer.ts";
import * as config from "internal:deno_node/polyfills/internal_binding/config.ts";
import * as caresWrap from "internal:deno_node/polyfills/internal_binding/cares_wrap.ts";
import * as constants from "internal:deno_node/polyfills/internal_binding/constants.ts";
import * as contextify from "internal:deno_node/polyfills/internal_binding/contextify.ts";
import * as crypto from "internal:deno_node/polyfills/internal_binding/crypto.ts";
import * as credentials from "internal:deno_node/polyfills/internal_binding/credentials.ts";
import * as errors from "internal:deno_node/polyfills/internal_binding/errors.ts";
import * as fs from "internal:deno_node/polyfills/internal_binding/fs.ts";
import * as fsDir from "internal:deno_node/polyfills/internal_binding/fs_dir.ts";
import * as fsEventWrap from "internal:deno_node/polyfills/internal_binding/fs_event_wrap.ts";
import * as heapUtils from "internal:deno_node/polyfills/internal_binding/heap_utils.ts";
import * as httpParser from "internal:deno_node/polyfills/internal_binding/http_parser.ts";
import * as icu from "internal:deno_node/polyfills/internal_binding/icu.ts";
import * as inspector from "internal:deno_node/polyfills/internal_binding/inspector.ts";
import * as jsStream from "internal:deno_node/polyfills/internal_binding/js_stream.ts";
import * as messaging from "internal:deno_node/polyfills/internal_binding/messaging.ts";
import * as moduleWrap from "internal:deno_node/polyfills/internal_binding/module_wrap.ts";
import * as nativeModule from "internal:deno_node/polyfills/internal_binding/native_module.ts";
import * as natives from "internal:deno_node/polyfills/internal_binding/natives.ts";
import * as options from "internal:deno_node/polyfills/internal_binding/options.ts";
import * as os from "internal:deno_node/polyfills/internal_binding/os.ts";
import * as pipeWrap from "internal:deno_node/polyfills/internal_binding/pipe_wrap.ts";
import * as performance from "internal:deno_node/polyfills/internal_binding/performance.ts";
import * as processMethods from "internal:deno_node/polyfills/internal_binding/process_methods.ts";
import * as report from "internal:deno_node/polyfills/internal_binding/report.ts";
import * as serdes from "internal:deno_node/polyfills/internal_binding/serdes.ts";
import * as signalWrap from "internal:deno_node/polyfills/internal_binding/signal_wrap.ts";
import * as spawnSync from "internal:deno_node/polyfills/internal_binding/spawn_sync.ts";
import * as streamWrap from "internal:deno_node/polyfills/internal_binding/stream_wrap.ts";
import * as stringDecoder from "internal:deno_node/polyfills/internal_binding/string_decoder.ts";
import * as symbols from "internal:deno_node/polyfills/internal_binding/symbols.ts";
import * as taskQueue from "internal:deno_node/polyfills/internal_binding/task_queue.ts";
import * as tcpWrap from "internal:deno_node/polyfills/internal_binding/tcp_wrap.ts";
import * as timers from "internal:deno_node/polyfills/internal_binding/timers.ts";
import * as tlsWrap from "internal:deno_node/polyfills/internal_binding/tls_wrap.ts";
import * as traceEvents from "internal:deno_node/polyfills/internal_binding/trace_events.ts";
import * as ttyWrap from "internal:deno_node/polyfills/internal_binding/tty_wrap.ts";
import * as types from "internal:deno_node/polyfills/internal_binding/types.ts";
import * as udpWrap from "internal:deno_node/polyfills/internal_binding/udp_wrap.ts";
import * as url from "internal:deno_node/polyfills/internal_binding/url.ts";
import * as util from "internal:deno_node/polyfills/internal_binding/util.ts";
import * as uv from "internal:deno_node/polyfills/internal_binding/uv.ts";
import * as v8 from "internal:deno_node/polyfills/internal_binding/v8.ts";
import * as worker from "internal:deno_node/polyfills/internal_binding/worker.ts";
import * as zlib from "internal:deno_node/polyfills/internal_binding/zlib.ts";

const modules = {
  "async_wrap": asyncWrap,
  buffer,
  "cares_wrap": caresWrap,
  config,
  constants,
  contextify,
  credentials,
  crypto,
  errors,
  fs,
  "fs_dir": fsDir,
  "fs_event_wrap": fsEventWrap,
  "heap_utils": heapUtils,
  "http_parser": httpParser,
  icu,
  inspector,
  "js_stream": jsStream,
  messaging,
  "module_wrap": moduleWrap,
  "native_module": nativeModule,
  natives,
  options,
  os,
  performance,
  "pipe_wrap": pipeWrap,
  "process_methods": processMethods,
  report,
  serdes,
  "signal_wrap": signalWrap,
  "spawn_sync": spawnSync,
  "stream_wrap": streamWrap,
  "string_decoder": stringDecoder,
  symbols,
  "task_queue": taskQueue,
  "tcp_wrap": tcpWrap,
  timers,
  "tls_wrap": tlsWrap,
  "trace_events": traceEvents,
  "tty_wrap": ttyWrap,
  types,
  "udp_wrap": udpWrap,
  url,
  util,
  uv,
  v8,
  worker,
  zlib,
};

export type BindingName = keyof typeof modules;

export function getBinding(name: BindingName) {
  const mod = modules[name];
  if (!mod) {
    throw new Error(`No such module: ${name}`);
  }
  return mod;
}
