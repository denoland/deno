// Copyright 2018-2025 the Deno authors. MIT license.
import { Encodings } from "ext:deno_node/internal_binding/_node.ts";

const encodings = [];
encodings[Encodings.ASCII] = "ascii";
encodings[Encodings.BASE64] = "base64";
encodings[Encodings.BASE64URL] = "base64url";
encodings[Encodings.BUFFER] = "buffer";
encodings[Encodings.HEX] = "hex";
encodings[Encodings.LATIN1] = "latin1";
encodings[Encodings.UCS2] = "utf16le";
encodings[Encodings.UTF8] = "utf8";

export default { encodings };
export { encodings };
