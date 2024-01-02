// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { ucs2 } from "ext:deno_node/internal/idna.ts";

const { ops } = globalThis.__bootstrap.core;

function toASCII(domain) {
  return ops.op_node_idna_domain_to_ascii(domain);
}

function toUnicode(domain) {
  return ops.op_node_idna_domain_to_unicode(domain);
}

function decode(domain) {
  return ops.op_node_idna_punycode_decode(domain);
}

function encode(domain) {
  return ops.op_node_idna_punycode_encode(domain);
}

export { decode, encode, toASCII, toUnicode, ucs2 };

export default {
  decode,
  encode,
  toASCII,
  toUnicode,
  ucs2,
};
