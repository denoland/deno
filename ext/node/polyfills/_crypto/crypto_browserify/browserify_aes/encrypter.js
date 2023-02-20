// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import AuthCipher from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/auth_cipher.js";
import StreamCipher from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/stream_cipher.js";
import Transform from "internal:deno_node/polyfills/_crypto/crypto_browserify/cipher_base.js";
import * as aes from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/aes.js";
import ebtk from "internal:deno_node/polyfills/_crypto/crypto_browserify/evp_bytes_to_key.ts";
import { MODES } from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/modes/mod.js";

function Cipher(mode, key, iv) {
  Transform.call(this);

  this._cache = new Splitter();
  this._cipher = new aes.AES(key);
  this._prev = Buffer.from(iv);
  this._mode = mode;
  this._autopadding = true;
}

Cipher.prototype = Object.create(Transform.prototype, {
  constructor: {
    value: Cipher,
    enumerable: false,
    writable: true,
    configurable: true,
  },
});

Cipher.prototype._update = function (data) {
  this._cache.add(data);
  var chunk;
  var thing;
  var out = [];

  while ((chunk = this._cache.get())) {
    thing = this._mode.encrypt(this, chunk);
    out.push(thing);
  }

  return Buffer.concat(out);
};

var PADDING = Buffer.alloc(16, 0x10);

Cipher.prototype._final = function () {
  var chunk = this._cache.flush();
  if (this._autopadding) {
    chunk = this._mode.encrypt(this, chunk);
    this._cipher.scrub();
    return chunk;
  }

  if (!chunk.equals(PADDING)) {
    this._cipher.scrub();
    throw new Error("data not multiple of block length");
  }
};

Cipher.prototype.setAutoPadding = function (setTo) {
  this._autopadding = !!setTo;
  return this;
};

function Splitter() {
  this.cache = Buffer.allocUnsafe(0);
}

Splitter.prototype.add = function (data) {
  this.cache = Buffer.concat([this.cache, data]);
};

Splitter.prototype.get = function () {
  if (this.cache.length > 15) {
    const out = this.cache.slice(0, 16);
    this.cache = this.cache.slice(16);
    return out;
  }
  return null;
};

Splitter.prototype.flush = function () {
  var len = 16 - this.cache.length;
  var padBuff = Buffer.allocUnsafe(len);

  var i = -1;
  while (++i < len) {
    padBuff.writeUInt8(len, i);
  }

  return Buffer.concat([this.cache, padBuff]);
};

export function createCipheriv(suite, password, iv) {
  var config = MODES[suite.toLowerCase()];
  if (!config) throw new TypeError("invalid suite type");

  if (typeof password === "string") password = Buffer.from(password);
  if (password.length !== config.key / 8) {
    throw new TypeError("invalid key length " + password.length);
  }

  if (typeof iv === "string") iv = Buffer.from(iv);
  if (config.mode !== "GCM" && iv.length !== config.iv) {
    throw new TypeError("invalid iv length " + iv.length);
  }

  if (config.type === "stream") {
    return new StreamCipher(config.module, password, iv);
  } else if (config.type === "auth") {
    return new AuthCipher(config.module, password, iv);
  }

  return new Cipher(config.module, password, iv);
}

export function createCipher(suite, password) {
  var config = MODES[suite.toLowerCase()];
  if (!config) throw new TypeError("invalid suite type");

  var keys = ebtk(password, false, config.key, config.iv);
  return createCipheriv(suite, keys.key, keys.iv);
}
