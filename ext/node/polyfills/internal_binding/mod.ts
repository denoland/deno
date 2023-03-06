// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as asyncWrap from "internal:deno_node/internal_binding/async_wrap.ts";
import * as buffer from "internal:deno_node/internal_binding/buffer.ts";
import * as caresWrap from "internal:deno_node/internal_binding/cares_wrap.ts";
import * as constants from "internal:deno_node/internal_binding/constants.ts";
import * as crypto from "internal:deno_node/internal_binding/crypto.ts";
import * as pipeWrap from "internal:deno_node/internal_binding/pipe_wrap.ts";
import * as streamWrap from "internal:deno_node/internal_binding/stream_wrap.ts";
import * as stringDecoder from "internal:deno_node/internal_binding/string_decoder.ts";
import * as symbols from "internal:deno_node/internal_binding/symbols.ts";
import * as tcpWrap from "internal:deno_node/internal_binding/tcp_wrap.ts";
import * as types from "internal:deno_node/internal_binding/types.ts";
import * as udpWrap from "internal:deno_node/internal_binding/udp_wrap.ts";
import * as util from "internal:deno_node/internal_binding/util.ts";
import * as uv from "internal:deno_node/internal_binding/uv.ts";

const modules = {
  "async_wrap": asyncWrap,
  buffer,
  "cares_wrap": caresWrap,
  config: {},
  constants,
  contextify: {},
  credentials: {},
  crypto,
  errors: {},
  fs: {},
  "fs_dir": {},
  "fs_event_wrap": {},
  "heap_utils": {},
  "http_parser": {},
  icu: {},
  inspector: {},
  "js_stream": {},
  messaging: {},
  "module_wrap": {},
  "native_module": {},
  natives: {},
  options: {},
  os: {},
  performance: {},
  "pipe_wrap": pipeWrap,
  "process_methods": {},
  report: {},
  serdes: {},
  "signal_wrap": {},
  "spawn_sync": {},
  "stream_wrap": streamWrap,
  "string_decoder": stringDecoder,
  symbols,
  "task_queue": {},
  "tcp_wrap": tcpWrap,
  timers: {},
  "tls_wrap": {},
  "trace_events": {},
  "tty_wrap": {},
  types,
  "udp_wrap": udpWrap,
  url: {},
  util,
  uv,
  v8: {},
  worker: {},
  zlib: {},
};

export type BindingName = keyof typeof modules;

export function getBinding(name: BindingName) {
  const mod = modules[name];
  if (!mod) {
    throw new Error(`No such module: ${name}`);
  }
  return mod;
}
