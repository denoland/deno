// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const internals = globalThis.__bootstrap.internals;
import _httpAgent from "ext:deno_node/_http_agent.mjs";
import _httpOutgoing from "ext:deno_node/_http_outgoing.ts";
import _streamDuplex from "ext:deno_node/internal/streams/duplex.mjs";
import _streamPassthrough from "ext:deno_node/internal/streams/passthrough.mjs";
import _streamReadable from "ext:deno_node/internal/streams/readable.mjs";
import _streamTransform from "ext:deno_node/internal/streams/transform.mjs";
import _streamWritable from "ext:deno_node/internal/streams/writable.mjs";
import assert from "ext:deno_node/assert.ts";
import assertStrict from "ext:deno_node/assert/strict.ts";
import asyncHooks from "ext:deno_node/async_hooks.ts";
import buffer from "ext:deno_node/buffer.ts";
import childProcess from "ext:deno_node/child_process.ts";
import cluster from "ext:deno_node/cluster.ts";
import console from "ext:deno_node/console.ts";
import constants from "ext:deno_node/constants.ts";
import crypto from "ext:deno_node/crypto.ts";
import dgram from "ext:deno_node/dgram.ts";
import diagnosticsChannel from "ext:deno_node/diagnostics_channel.ts";
import dns from "ext:deno_node/dns.ts";
import dnsPromises from "ext:deno_node/dns/promises.ts";
import domain from "ext:deno_node/domain.ts";
import events from "ext:deno_node/events.ts";
import fs from "ext:deno_node/fs.ts";
import fsPromises from "ext:deno_node/fs/promises.ts";
import http from "ext:deno_node/http.ts";
import http2 from "ext:deno_node/http2.ts";
import https from "ext:deno_node/https.ts";
import inspector from "ext:deno_node/inspector.ts";
import internalCp from "ext:deno_node/internal/child_process.ts";
import internalCryptoCertificate from "ext:deno_node/internal/crypto/certificate.ts";
import internalCryptoCipher from "ext:deno_node/internal/crypto/cipher.ts";
import internalCryptoDiffiehellman from "ext:deno_node/internal/crypto/diffiehellman.ts";
import internalCryptoHash from "ext:deno_node/internal/crypto/hash.ts";
import internalCryptoHkdf from "ext:deno_node/internal/crypto/hkdf.ts";
import internalCryptoKeygen from "ext:deno_node/internal/crypto/keygen.ts";
import internalCryptoKeys from "ext:deno_node/internal/crypto/keys.ts";
import internalCryptoPbkdf2 from "ext:deno_node/internal/crypto/pbkdf2.ts";
import internalCryptoRandom from "ext:deno_node/internal/crypto/random.ts";
import internalCryptoScrypt from "ext:deno_node/internal/crypto/scrypt.ts";
import internalCryptoSig from "ext:deno_node/internal/crypto/sig.ts";
import internalCryptoUtil from "ext:deno_node/internal/crypto/util.ts";
import internalCryptoX509 from "ext:deno_node/internal/crypto/x509.ts";
import internalDgram from "ext:deno_node/internal/dgram.ts";
import internalDnsPromises from "ext:deno_node/internal/dns/promises.ts";
import internalErrors from "ext:deno_node/internal/errors.ts";
import internalEventTarget from "ext:deno_node/internal/event_target.mjs";
import internalFsUtils from "ext:deno_node/internal/fs/utils.mjs";
import internalHttp from "ext:deno_node/internal/http.ts";
import internalReadlineUtils from "ext:deno_node/internal/readline/utils.mjs";
import internalStreamsAddAbortSignal from "ext:deno_node/internal/streams/add-abort-signal.mjs";
import internalStreamsBufferList from "ext:deno_node/internal/streams/buffer_list.mjs";
import internalStreamsLazyTransform from "ext:deno_node/internal/streams/lazy_transform.mjs";
import internalStreamsState from "ext:deno_node/internal/streams/state.mjs";
import internalTestBinding from "ext:deno_node/internal/test/binding.ts";
import internalTimers from "ext:deno_node/internal/timers.mjs";
import internalUtil from "ext:deno_node/internal/util.mjs";
import internalUtilInspect from "ext:deno_node/internal/util/inspect.mjs";
import net from "ext:deno_node/net.ts";
import os from "ext:deno_node/os.ts";
import pathPosix from "ext:deno_node/path/posix.ts";
import pathWin32 from "ext:deno_node/path/win32.ts";
import path from "ext:deno_node/path.ts";
import perfHooks from "ext:deno_node/perf_hooks.ts";
import punycode from "ext:deno_node/punycode.ts";
import process from "ext:deno_node/process.ts";
import querystring from "ext:deno_node/querystring.ts";
import readline from "ext:deno_node/readline.ts";
import readlinePromises from "ext:deno_node/readline/promises.ts";
import repl from "ext:deno_node/repl.ts";
import stream from "ext:deno_node/stream.ts";
import streamConsumers from "ext:deno_node/stream/consumers.mjs";
import streamPromises from "ext:deno_node/stream/promises.mjs";
import streamWeb from "ext:deno_node/stream/web.ts";
import stringDecoder from "ext:deno_node/string_decoder.ts";
import sys from "ext:deno_node/sys.ts";
import timers from "ext:deno_node/timers.ts";
import timersPromises from "ext:deno_node/timers/promises.ts";
import tls from "ext:deno_node/tls.ts";
import tty from "ext:deno_node/tty.ts";
import url from "ext:deno_node/url.ts";
import utilTypes from "ext:deno_node/util/types.ts";
import util from "ext:deno_node/util.ts";
import v8 from "ext:deno_node/v8.ts";
import vm from "ext:deno_node/vm.ts";
import workerThreads from "ext:deno_node/worker_threads.ts";
import wasi from "ext:deno_node/wasi.ts";
import zlib from "ext:deno_node/zlib.ts";

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
