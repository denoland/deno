// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// @deno-types="../react.d.ts"
import React from "./react_mock.js";
// @deno-types="../react-dom.d.ts"
import ReactDOM from "./react-dom_mock.js";

import { assertEquals } from "../../testing/asserts.ts";

const { test } = Deno;

test({
  name: "ReactDOM is typed to render",
  fn() {
    assertEquals(
      ReactDOM.render(<div />, null),
      '"{\\"type\\":\\"div\\",\\"props\\":null,\\"children\\":[]}"'
    );
  }
});
