// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

import * as aes from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/aes.js";
import Transform from "internal:deno_node/polyfills/_crypto/crypto_browserify/cipher_base.js";

export function StreamCipher(mode, key, iv, decrypt) {
  Transform.call(this);

  this._cipher = new aes.AES(key);
  this._prev = Buffer.from(iv);
  this._cache = Buffer.allocUnsafe(0);
  this._secCache = Buffer.allocUnsafe(0);
  this._decrypt = decrypt;
  this._mode = mode;
}

// StreamCipher inherits Transform
StreamCipher.prototype = Object.create(Transform.prototype, {
  constructor: {
    value: StreamCipher,
    enumerable: false,
    writable: true,
    configurable: true,
  },
});

StreamCipher.prototype._update = function (chunk) {
  return this._mode.encrypt(this, chunk, this._decrypt);
};

StreamCipher.prototype._final = function () {
  this._cipher.scrub();
};

export default StreamCipher;
