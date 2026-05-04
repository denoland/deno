// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
//
// Shared TLS helpers used by both tls.ts and _tls_wrap.js.
// Extracted to avoid circular dependency between those modules.

// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import { core, primordials } from "ext:core/mod.js";
const { codes } = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  isArrayBufferView,
  isUint8Array,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");

const { ArrayIsArray } = primordials;

function convertProtocols(protocols) {
  const lengths = new Array(protocols.length);
  const buffer = Buffer.allocUnsafe(
    protocols.reduce((total, protocol, index) => {
      const length = Buffer.byteLength(protocol);
      if (length > 255) {
        throw new codes.ERR_OUT_OF_RANGE(
          `The byte length of the protocol at index ${index} exceeds the maximum length.`,
          "<= 255",
          length,
          true,
        );
      }
      lengths[index] = length;
      return total + 1 + length;
    }, 0),
  );

  let offset = 0;
  for (let i = 0; i < protocols.length; i++) {
    buffer[offset++] = lengths[i];
    buffer.write(protocols[i], offset);
    offset += lengths[i];
  }

  return buffer;
}

export function convertALPNProtocols(protocols, out) {
  if (ArrayIsArray(protocols)) {
    out.ALPNProtocols = convertProtocols(protocols);
  } else if (isUint8Array(protocols)) {
    out.ALPNProtocols = Buffer.from(protocols);
  } else if (isArrayBufferView(protocols)) {
    out.ALPNProtocols = Buffer.from(
      protocols.buffer.slice(
        protocols.byteOffset,
        protocols.byteOffset + protocols.byteLength,
      ),
    );
  }
}
