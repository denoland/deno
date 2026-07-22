// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = __bootstrap;
const {
  op_node_idna_punycode_decode,
  op_node_idna_punycode_encode,
  op_node_idna_punycode_to_ascii,
  op_node_idna_punycode_to_unicode,
} = core.ops;

const { ucs2 } = core.loadExtScript("ext:deno_node/internal/idna.ts");

const version = "2.1.0";

function toASCII(domain) {
  return op_node_idna_punycode_to_ascii(domain);
}

function toUnicode(domain) {
  return op_node_idna_punycode_to_unicode(domain);
}

function decode(domain) {
  return op_node_idna_punycode_decode(domain);
}

function encode(domain) {
  return op_node_idna_punycode_encode(domain);
}

return {
  default: {
    decode,
    encode,
    toASCII,
    toUnicode,
    ucs2,
    version,
  },
  decode,
  encode,
  toASCII,
  toUnicode,
  ucs2,
  version,
};
})();
