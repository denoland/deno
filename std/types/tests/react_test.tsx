// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// @deno-types="../react.d.ts"
import React from "./react_mock.js";
import { assertEquals } from "../../testing/asserts.ts";

const { test } = Deno;

test({
  name: "JSX can be rendered",
  fn() {
    class Component {
      render() {
        return <div></div>;
      }
    }

    assertEquals(
      new Component().render(),
      `{"type":"div","props":null,"children":[]}`
    );
  }
});
