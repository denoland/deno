// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_node_start_sigint_watchdog,
  op_node_stop_sigint_watchdog,
  op_node_watchdog_has_pending_sigint,
} from "ext:core/ops";
const {
  Error,
  MathFloor,
  ObjectDefineProperty,
  ObjectKeys,
  SafeArrayIterator,
  StringPrototypeStartsWith,
} = primordials;
const asyncWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/async_wrap.ts",
);
const { default: blockList } = core.loadExtScript(
  "ext:deno_node/internal_binding/block_list.ts",
);
const buffer = core.loadExtScript(
  "ext:deno_node/internal_binding/buffer.ts",
);
const { default: caresWrap } = core.loadExtScript(
  "ext:deno_node/internal_binding/cares_wrap.ts",
);
const constants = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
);
const crypto = core.loadExtScript(
  "ext:deno_node/internal_binding/crypto.ts",
);
const pipeWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/pipe_wrap.ts",
);
const streamWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/stream_wrap.ts",
);
const stringDecoder = core.loadExtScript(
  "ext:deno_node/internal_binding/string_decoder.ts",
);
const symbols = core.loadExtScript(
  "ext:deno_node/internal_binding/symbols.ts",
);
const tcpWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/tcp_wrap.ts",
);
const ttyWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/tty_wrap.ts",
);
const types = core.loadExtScript("ext:deno_node/internal_binding/types.ts");
const udpWrap = core.loadExtScript(
  "ext:deno_node/internal_binding/udp_wrap.ts",
);
const util = core.loadExtScript(
  "ext:deno_node/internal_binding/util.ts",
);
const uvNamespace = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
const httpParser = core.loadExtScript(
  "ext:deno_node/internal_binding/http_parser.ts",
);
const http2Binding = core.loadExtScript(
  "ext:deno_node/internal_binding/http2.ts",
);
const inspectorBinding = core.loadExtScript(
  "ext:deno_node/internal_binding/inspector.js",
);

// Mutable shallow copy so callers can replace properties (e.g. wrap
// `errname` with a deprecation warning when --pending-deprecation is set).
// Match Node's C++ binding: UV_* error code constants are read-only and
// non-deletable. See `Initialize` in `src/uv.cc`.
const uv: Record<string, unknown> = {};
for (const key of new SafeArrayIterator(ObjectKeys(uvNamespace))) {
  const value = (uvNamespace as Record<string, unknown>)[key];
  if (StringPrototypeStartsWith(key, "UV_")) {
    ObjectDefineProperty(uv, key, {
      __proto__: null,
      value,
      writable: false,
      enumerable: true,
      configurable: false,
    });
  } else {
    uv[key] = value;
  }
}

const modules = {
  "async_wrap": asyncWrap,
  "block_list": blockList,
  buffer,
  "cares_wrap": caresWrap,
  config: {},
  constants,
  contextify: {
    startSigintWatchdog: op_node_start_sigint_watchdog,
    stopSigintWatchdog: op_node_stop_sigint_watchdog,
    watchdogHasPendingSigint: op_node_watchdog_has_pending_sigint,
  },
  credentials: {},
  crypto,
  errors: {},
  fs: {},
  "fs_dir": {},
  "fs_event_wrap": {},
  "heap_utils": {},
  "http_parser": httpParser,
  "http2": http2Binding,
  icu: {},
  inspector: inspectorBinding,
  "js_stream": {},
  messaging: {},
  "module_wrap": {},
  "native_module": {},
  natives: {},
  options: {},
  os: {},
  performance: {
    // observerCounts is an array where index is entry type and value is observer count
    // Initialize with zeros for all entry types (0-8)
    observerCounts: [0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
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
  timers: {
    getLibuvNow() {
      return MathFloor(performance.now());
    },
  },
  "tls_wrap": {},
  "trace_events": {},
  "tty_wrap": ttyWrap,
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
