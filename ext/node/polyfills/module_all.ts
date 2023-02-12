// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import _httpAgent from "./_http_agent.mjs";
import _httpOutgoing from "./_http_outgoing.ts";
import _streamDuplex from "./internal/streams/duplex.mjs";
import _streamPassthrough from "./internal/streams/passthrough.mjs";
import _streamReadable from "./internal/streams/readable.mjs";
import _streamTransform from "./internal/streams/transform.mjs";
import _streamWritable from "./internal/streams/writable.mjs";
import assert from "./assert.ts";
import assertStrict from "./assert/strict.ts";
import asyncHooks from "./async_hooks.ts";
import buffer from "./buffer.ts";
import childProcess from "./child_process.ts";
import cluster from "./cluster.ts";
import console from "./console.ts";
import constants from "./constants.ts";
import crypto from "./crypto.ts";
import dgram from "./dgram.ts";
import diagnosticsChannel from "./diagnostics_channel.ts";
import dns from "./dns.ts";
import dnsPromises from "./dns/promises.ts";
import domain from "./domain.ts";
import events from "./events.ts";
import fs from "./fs.ts";
import fsPromises from "./fs/promises.ts";
import http from "./http.ts";
import http2 from "./http2.ts";
import https from "./https.ts";
import inspector from "./inspector.ts";
import internalCp from "./internal/child_process.ts";
import internalCryptoCertificate from "./internal/crypto/certificate.ts";
import internalCryptoCipher from "./internal/crypto/cipher.ts";
import internalCryptoDiffiehellman from "./internal/crypto/diffiehellman.ts";
import internalCryptoHash from "./internal/crypto/hash.ts";
import internalCryptoHkdf from "./internal/crypto/hkdf.ts";
import internalCryptoKeygen from "./internal/crypto/keygen.ts";
import internalCryptoKeys from "./internal/crypto/keys.ts";
import internalCryptoPbkdf2 from "./internal/crypto/pbkdf2.ts";
import internalCryptoRandom from "./internal/crypto/random.ts";
import internalCryptoScrypt from "./internal/crypto/scrypt.ts";
import internalCryptoSig from "./internal/crypto/sig.ts";
import internalCryptoUtil from "./internal/crypto/util.ts";
import internalCryptoX509 from "./internal/crypto/x509.ts";
import internalDgram from "./internal/dgram.ts";
import internalDnsPromises from "./internal/dns/promises.ts";
import internalErrors from "./internal/errors.ts";
import internalEventTarget from "./internal/event_target.mjs";
import internalFsUtils from "./internal/fs/utils.mjs";
import internalHttp from "./internal/http.ts";
import internalReadlineUtils from "./internal/readline/utils.mjs";
import internalStreamsAddAbortSignal from "./internal/streams/add-abort-signal.mjs";
import internalStreamsBufferList from "./internal/streams/buffer_list.mjs";
import internalStreamsLazyTransform from "./internal/streams/lazy_transform.mjs";
import internalStreamsState from "./internal/streams/state.mjs";
import internalTestBinding from "./internal/test/binding.ts";
import internalTimers from "./internal/timers.mjs";
import internalUtil from "./internal/util.mjs";
import internalUtilInspect from "./internal/util/inspect.mjs";
import net from "./net.ts";
import os from "./os.ts";
import pathPosix from "./path/posix.ts";
import pathWin32 from "./path/win32.ts";
import path from "./path.ts";
import perfHooks from "./perf_hooks.ts";
import punycode from "./punycode.ts";
import process from "./process.ts";
import querystring from "./querystring.ts";
import readline from "./readline.ts";
import readlinePromises from "./readline/promises.ts";
import repl from "./repl.ts";
import stream from "./stream.ts";
import streamConsumers from "./stream/consumers.mjs";
import streamPromises from "./stream/promises.mjs";
import streamWeb from "./stream/web.ts";
import stringDecoder from "./string_decoder.ts";
import sys from "./sys.ts";
import timers from "./timers.ts";
import timersPromises from "./timers/promises.ts";
import tls from "./tls.ts";
import tty from "./tty.ts";
import url from "./url.ts";
import utilTypes from "./util/types.ts";
import util from "./util.ts";
import v8 from "./v8.ts";
import vm from "./vm.ts";
import workerThreads from "./worker_threads.ts";
import wasi from "./wasi.ts";
import zlib from "./zlib.ts";

// Canonical mapping of supported modules
export default {
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
