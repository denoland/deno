// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  decode,
  encode,
  toASCII,
  toUnicode,
  ucs2,
} from "internal:deno_node/polyfills/internal/idna.ts";

export { decode, encode, toASCII, toUnicode, ucs2 };

export default {
  decode,
  encode,
  toASCII,
  toUnicode,
  ucs2,
};
