// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import { parse } from "../mod.ts";

test(function longOpts() {
  assertEq(parse(["--bool"]), { bool: true, _: [] });
  assertEq(parse(["--pow", "xixxle"]), { pow: "xixxle", _: [] });
  assertEq(parse(["--pow=xixxle"]), { pow: "xixxle", _: [] });
  assertEq(parse(["--host", "localhost", "--port", "555"]), {
    host: "localhost",
    port: 555,
    _: []
  });
  assertEq(parse(["--host=localhost", "--port=555"]), {
    host: "localhost",
    port: 555,
    _: []
  });
});
