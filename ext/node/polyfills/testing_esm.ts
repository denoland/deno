// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
import { Readable } from "node:stream";
const mod = core.loadExtScript("ext:deno_node/testing.ts");

// Install the real implementation of `test.run`. Deno does not run tests
// programmatically through `node:test`, so `run()` returns an empty
// `TestsStream`-compatible Readable that ends immediately.
mod.setRunImpl((_options) => Readable.from([]));

export const {
  run,
  test,
  suite,
  it,
  describe,
  before,
  after,
  beforeEach,
  afterEach,
  mock,
} = mod;

export default mod.default;
