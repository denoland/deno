// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import bignum from "internal:deno_node/polyfills/_crypto/crypto_browserify/bn.js/bn.js";
import { Node } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/base/node.js";
import {
  DecoderBuffer,
  EncoderBuffer,
} from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/base/buffer.js";
import { Reporter } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/base/reporter.js";
import { DEREncoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/encoders/der.js";
import { PEMEncoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/encoders/pem.js";
import { DERDecoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/decoders/der.js";
import { PEMDecoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/decoders/pem.js";
import * as der from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/constants/der.js";

export const base = {
  DecoderBuffer,
  EncoderBuffer,
  Node,
  Reporter,
};
export const encoders = { der: DEREncoder, pem: PEMEncoder };
export const decoders = { der: DERDecoder, pem: PEMDecoder };
export const constants = { der };
export { bignum };

export function define(name, body) {
  return new Entity(name, body);
}

function Entity(name, body) {
  this.name = name;
  this.body = body;

  this.decoders = {};
  this.encoders = {};
}

Entity.prototype._createNamed = function createNamed(Base) {
  const name = this.name;

  function Generated(entity) {
    this._initNamed(entity, name);
  }
  // inherits(Generated, Base);
  Generated.prototype = Object.create(Base.prototype, {
    constructor: {
      value: Generated,
      enumerable: false,
      writable: true,
      configurable: true,
    },
  });
  Generated.prototype._initNamed = function _initNamed(entity, name) {
    Base.call(this, entity, name);
  };
  return new Generated(this);
};

Entity.prototype._getDecoder = function _getDecoder(enc) {
  enc = enc || "der";
  // Lazily create decoder
  // deno-lint-ignore no-prototype-builtins
  if (!this.decoders.hasOwnProperty(enc)) {
    this.decoders[enc] = this._createNamed(decoders[enc]);
  }
  return this.decoders[enc];
};

Entity.prototype.decode = function decode(data, enc, options) {
  return this._getDecoder(enc).decode(data, options);
};

Entity.prototype._getEncoder = function _getEncoder(enc) {
  enc = enc || "der";
  // Lazily create encoder
  // deno-lint-ignore no-prototype-builtins
  if (!this.encoders.hasOwnProperty(enc)) {
    this.encoders[enc] = this._createNamed(encoders[enc]);
  }
  return this.encoders[enc];
};

Entity.prototype.encode = function encode(data, enc, /* internal */ reporter) {
  return this._getEncoder(enc).encode(data, reporter);
};

export default {
  base,
  bignum,
  constants,
  decoders,
  define,
  encoders,
};
