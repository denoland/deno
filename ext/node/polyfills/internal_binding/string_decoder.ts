// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore no-unused-vars
import type { Encodings } from "ext:deno_node/internal_binding/_node.ts";

/**
 * * {@linkcode Encodings.ASCII} = "ascii"
 * * {@linkcode Encodings.BASE64} = "base64"
 * * {@linkcode Encodings.BASE64URL} = "base64url"
 * * {@linkcode Encodings.BUFFER} = "buffer"
 * * {@linkcode Encodings.HEX} = "hex"
 * * {@linkcode Encodings.LATIN1} = "latin1"
 * * {@linkcode Encodings.UCS2} = "utf16le"
 * * {@linkcode Encodings.UTF8} = "utf8"
 */
const encodings = [
  "ascii",
  "utf8",
  "base64",
  "utf16le",
  "latin1",
  "hex",
  "buffer",
  "base64url",
] as const;

export default { encodings };
export { encodings };
