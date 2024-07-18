// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "@std/assert/mod.ts";
import { createReadStream } from "node:fs";

Deno.test("fetch node stream", async () => {
  const file = createReadStream("tests/testdata/assets/fixture.json");

  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: file,
  });

  assertEquals(
    await response.text(),
    await Deno.readTextFile("tests/testdata/assets/fixture.json"),
  );
});
