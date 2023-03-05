// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const internals = globalThis.__bootstrap.internals;
import _httpAgent from "internal:deno_node/_http_agent.mjs";
import _httpOutgoing from "internal:deno_node/_http_outgoing.ts";
import _streamDuplex from "internal:deno_node/internal/streams/duplex.mjs";
import _streamPassthrough from "internal:deno_node/internal/streams/passthrough.mjs";
import _streamReadable from "internal:deno_node/internal/streams/readable.mjs";
import _streamTransform from "internal:deno_node/internal/streams/transform.mjs";
import _streamWritable from "internal:deno_node/internal/streams/writable.mjs";
import assert from "internal:deno_node/assert.ts";
import assertStrict from "internal:deno_node/assert/strict.ts";
import asyncHooks from "internal:deno_node/async_hooks.ts";
import buffer from "internal:deno_node/buffer.ts";
import childProcess from "internal:deno_node/child_process.ts";
import cluster from "internal:deno_node/cluster.ts";
import console from "internal:deno_node/console.ts";
import constants from "internal:deno_node/constants.ts";
import crypto from "internal:deno_node/crypto.ts";
import dgram from "internal:deno_node/dgram.ts";
import diagnosticsChannel from "internal:deno_node/diagnostics_channel.ts";
import dns from "internal:deno_node/dns.ts";
import dnsPromises from "internal:deno_node/dns/promises.ts";
import domain from "internal:deno_node/domain.ts";
import events from "internal:deno_node/events.ts";
import fs from "internal:deno_node/fs.ts";
import fsPromises from "internal:deno_node/fs/promises.ts";
import http from "internal:deno_node/http.ts";
import http2 from "internal:deno_node/http2.ts";
import https from "internal:deno_node/https.ts";
import inspector from "internal:deno_node/inspector.ts";
import internalCp from "internal:deno_node/internal/child_process.ts";
import internalCryptoCertificate from "internal:deno_node/internal/crypto/certificate.ts";
import internalCryptoCipher from "internal:deno_node/internal/crypto/cipher.ts";
import internalCryptoDiffiehellman from "internal:deno_node/internal/crypto/diffiehellman.ts";
import internalCryptoHash from "internal:deno_node/internal/crypto/hash.ts";
import internalCryptoHkdf from "internal:deno_node/internal/crypto/hkdf.ts";
import internalCryptoKeygen from "internal:deno_node/internal/crypto/keygen.ts";
import internalCryptoKeys from "internal:deno_node/internal/crypto/keys.ts";
import internalCryptoPbkdf2 from "internal:deno_node/internal/crypto/pbkdf2.ts";
import internalCryptoRandom from "internal:deno_node/internal/crypto/random.ts";
import internalCryptoScrypt from "internal:deno_node/internal/crypto/scrypt.ts";
import internalCryptoSig from "internal:deno_node/internal/crypto/sig.ts";
import internalCryptoUtil from "internal:deno_node/internal/crypto/util.ts";
import internalCryptoX509 from "internal:deno_node/internal/crypto/x509.ts";
import internalDgram from "internal:deno_node/internal/dgram.ts";
import internalDnsPromises from "internal:deno_node/internal/dns/promises.ts";
import internalErrors from "internal:deno_node/internal/errors.ts";
import internalEventTarget from "internal:deno_node/internal/event_target.mjs";
import internalFsUtils from "internal:deno_node/internal/fs/utils.mjs";
import internalHttp from "internal:deno_node/internal/http.ts";
import internalReadlineUtils from "internal:deno_node/internal/readline/utils.mjs";
import internalStreamsAddAbortSignal from "internal:deno_node/internal/streams/add-abort-signal.mjs";
import internalStreamsBufferList from "internal:deno_node/internal/streams/buffer_list.mjs";
import internalStreamsLazyTransform from "internal:deno_node/internal/streams/lazy_transform.mjs";
import internalStreamsState from "internal:deno_node/internal/streams/state.mjs";
import internalTestBinding from "internal:deno_node/internal/test/binding.ts";
import internalTimers from "internal:deno_node/internal/timers.mjs";
import internalUtil from "internal:deno_node/internal/util.mjs";
import internalUtilInspect from "internal:deno_node/internal/util/inspect.mjs";
import net from "internal:deno_node/net.ts";
import os from "internal:deno_node/os.ts";
import pathPosix from "internal:deno_node/path/posix.ts";
import pathWin32 from "internal:deno_node/path/win32.ts";
import path from "internal:deno_node/path.ts";
import perfHooks from "internal:deno_node/perf_hooks.ts";
import punycode from "internal:deno_node/punycode.ts";
import process from "internal:deno_node/process.ts";
import querystring from "internal:deno_node/querystring.ts";
import readline from "internal:deno_node/readline.ts";
import readlinePromises from "internal:deno_node/readline/promises.ts";
import repl from "internal:deno_node/repl.ts";
import stream from "internal:deno_node/stream.ts";
import streamConsumers from "internal:deno_node/stream/consumers.mjs";
import streamPromises from "internal:deno_node/stream/promises.mjs";
import streamWeb from "internal:deno_node/stream/web.ts";
import stringDecoder from "internal:deno_node/string_decoder.ts";
import sys from "internal:deno_node/sys.ts";
import timers from "internal:deno_node/timers.ts";
import timersPromises from "internal:deno_node/timers/promises.ts";
import tls from "internal:deno_node/tls.ts";
import tty from "internal:deno_node/tty.ts";
import url from "internal:deno_node/url.ts";
import utilTypes from "internal:deno_node/util/types.ts";
import util from "internal:deno_node/util.ts";
import v8 from "internal:deno_node/v8.ts";
import vm from "internal:deno_node/vm.ts";
import workerThreads from "internal:deno_node/worker_threads.ts";
import wasi from "internal:deno_node/wasi.ts";
import zlib from "internal:deno_node/zlib.ts";

// Canonical mapping of supported modules
const moduleAll = {
  "_http_agent": _httpAgent,
  "_http_outgoing": _httpOutgoing,
  "_stream_duplex": _streamDuplex,
  "_stream_passthrough": _streamPassthrough,
  "_stream_readable": _streamReadable,
  "_stream_transform": _streamTransform,
  "_stream_writable": _streamWritable,
  assert,
  "assert/strict": assertStrict,
  "async_hooks": asyncHooks,
  buffer,
  crypto,
  console,
  constants,
  child_process: childProcess,
  cluster,
  dgram,
  diagnostics_channel: diagnosticsChannel,
  dns,
  "dns/promises": dnsPromises,
  domain,
  events,
  fs,
  "fs/promises": fsPromises,
  http,
  http2,
  https,
  inspector,
  "internal/child_process": internalCp,
  "internal/crypto/certificate": internalCryptoCertificate,
  "internal/crypto/cipher": internalCryptoCipher,
  "internal/crypto/diffiehellman": internalCryptoDiffiehellman,
  "internal/crypto/hash": internalCryptoHash,
  "internal/crypto/hkdf": internalCryptoHkdf,
  "internal/crypto/keygen": internalCryptoKeygen,
  "internal/crypto/keys": internalCryptoKeys,
  "internal/crypto/pbkdf2": internalCryptoPbkdf2,
  "internal/crypto/random": internalCryptoRandom,
  "internal/crypto/scrypt": internalCryptoScrypt,
  "internal/crypto/sig": internalCryptoSig,
  "internal/crypto/util": internalCryptoUtil,
  "internal/crypto/x509": internalCryptoX509,
  "internal/dgram": internalDgram,
  "internal/dns/promises": internalDnsPromises,
  "internal/errors": internalErrors,
  "internal/event_target": internalEventTarget,
  "internal/fs/utils": internalFsUtils,
  "internal/http": internalHttp,
  "internal/readline/utils": internalReadlineUtils,
  "internal/streams/add-abort-signal": internalStreamsAddAbortSignal,
  "internal/streams/buffer_list": internalStreamsBufferList,
  "internal/streams/lazy_transform": internalStreamsLazyTransform,
  "internal/streams/state": internalStreamsState,
  "internal/test/binding": internalTestBinding,
  "internal/timers": internalTimers,
  "internal/util/inspect": internalUtilInspect,
  "internal/util": internalUtil,
  net,
  os,
  "path/posix": pathPosix,
  "path/win32": pathWin32,
  path,
  perf_hooks: perfHooks,
  process,
  get punycode() {
    process.emitWarning(
      "The `punycode` module is deprecated. Please use a userland " +
        "alternative instead.",
      "DeprecationWarning",
      "DEP0040",
    );
    return punycode;
  },
  querystring,
  readline,
  "readline/promises": readlinePromises,
  repl,
  stream,
  "stream/consumers": streamConsumers,
  "stream/promises": streamPromises,
  "stream/web": streamWeb,
  string_decoder: stringDecoder,
  sys,
  timers,
  "timers/promises": timersPromises,
  tls,
  tty,
  url,
  util,
  "util/types": utilTypes,
  v8,
  vm,
  wasi,
  worker_threads: workerThreads,
  zlib,
} as Record<string, unknown>;

internals.nodeModuleAll = moduleAll;
export default moduleAll;
