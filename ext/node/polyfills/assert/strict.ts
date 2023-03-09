// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { strict } from "ext:deno_node/assert.ts";

export {
  AssertionError,
  deepEqual,
  deepStrictEqual,
  doesNotMatch,
  doesNotReject,
  doesNotThrow,
  equal,
  fail,
  ifError,
  match,
  notDeepEqual,
  notDeepStrictEqual,
  notEqual,
  notStrictEqual,
  ok,
  rejects,
  strictEqual,
  throws,
} from "ext:deno_node/assert.ts";

export { strict };
export default strict;
