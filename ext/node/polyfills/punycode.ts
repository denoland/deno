// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  op_node_idna_domain_to_ascii,
  op_node_idna_domain_to_unicode,
  op_node_idna_punycode_decode,
  op_node_idna_punycode_encode,
} from "ext:core/ops";

import { ucs2 } from "ext:deno_node/internal/idna.ts";

function toASCII(domain) {
  return op_node_idna_domain_to_ascii(domain);
}

function toUnicode(domain) {
  return op_node_idna_domain_to_unicode(domain);
}

function decode(domain) {
  return op_node_idna_punycode_decode(domain);
}

function encode(domain) {
  return op_node_idna_punycode_encode(domain);
}

export { decode, encode, toASCII, toUnicode, ucs2 };

export default {
  decode,
  encode,
  toASCII,
  toUnicode,
  ucs2,
};
