// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, runIfMain } from "../testing/mod.ts";
import { assert } from "../testing/asserts.ts";
import { isRemoteUrl } from "./mod.ts";

// TODO(ry) Many installer tests were removed in order to get deno_std to merge
// into the deno repo. Bring them back.
// https://github.com/denoland/deno_std/blob/98784c305c653b1c507b4b25be82ecf40f188305/installer/test.ts

test(function testIsRemoteUrl(): void {
  assert(isRemoteUrl("https://deno.land/std/http/file_server.ts"));
  assert(isRemoteUrl("http://deno.land/std/http/file_server.ts"));
  assert(!isRemoteUrl("file:///dev/deno_std/http/file_server.ts"));
  assert(!isRemoteUrl("./dev/deno_std/http/file_server.ts"));
});

runIfMain(import.meta);
