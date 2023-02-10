// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

import { DERDecoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/decoders/der.js";

export function PEMDecoder(entity) {
  DERDecoder.call(this, entity);
  this.enc = "pem";
}
// inherits(PEMDecoder, DERDecoder);
PEMDecoder.prototype = Object.create(DERDecoder.prototype, {
  constructor: {
    value: PEMDecoder,
    enumerable: false,
    writable: true,
    configurable: true,
  },
});

PEMDecoder.prototype.decode = function decode(data, options) {
  const lines = data.toString().split(/[\r\n]+/g);

  const label = options.label.toUpperCase();

  const re = /^-----(BEGIN|END) ([^-]+)-----$/;
  let start = -1;
  let end = -1;
  for (let i = 0; i < lines.length; i++) {
    const match = lines[i].match(re);
    if (match === null) {
      continue;
    }

    if (match[2] !== label) {
      continue;
    }

    if (start === -1) {
      if (match[1] !== "BEGIN") {
        break;
      }
      start = i;
    } else {
      if (match[1] !== "END") {
        break;
      }
      end = i;
      break;
    }
  }
  if (start === -1 || end === -1) {
    throw new Error("PEM section not found for: " + label);
  }

  const base64 = lines.slice(start + 1, end).join("");
  // Remove excessive symbols
  base64.replace(/[^a-z0-9+/=]+/gi, "");

  const input = Buffer.from(base64, "base64");
  return DERDecoder.prototype.decode.call(this, input, options);
};
