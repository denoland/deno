// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { isURL } from "./is_url.ts"
import { assert } from "../testing/asserts.ts"

test("isUrl", function(): void {
  assert(isURL("https://example.com"))
  assert(isURL("https://localhost:4000"))
  assert(isURL("https://localhost"))
  assert(isURL("postgres://user:postgres@example.com:5432/database"))
  assert(!isURL("https://"))
  assert(!isURL("example"))
  assert(!isURL("example.com"))
})
