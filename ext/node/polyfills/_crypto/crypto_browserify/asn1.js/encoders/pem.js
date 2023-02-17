// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Fedor Indutny. All rights reserved. MIT license.

import { DEREncoder } from "internal:deno_node/polyfills/_crypto/crypto_browserify/asn1.js/encoders/der.js";

export function PEMEncoder(entity) {
  DEREncoder.call(this, entity);
  this.enc = "pem";
}
// inherits(PEMEncoder, DEREncoder);
PEMEncoder.prototype = Object.create(DEREncoder.prototype, {
  constructor: {
    value: PEMEncoder,
    enumerable: false,
    writable: true,
    configurable: true,
  },
});

PEMEncoder.prototype.encode = function encode(data, options) {
  const buf = DEREncoder.prototype.encode.call(this, data);

  const p = buf.toString("base64");
  const out = ["-----BEGIN " + options.label + "-----"];
  for (let i = 0; i < p.length; i += 64) {
    out.push(p.slice(i, i + 64));
  }
  out.push("-----END " + options.label + "-----");
  return out.join("\n");
};
