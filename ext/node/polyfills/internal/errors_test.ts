// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  NodeRangeError,
  NodeSyntaxError,
  NodeTypeError,
  NodeURIError,
} from "./errors.ts";
import { assertEquals } from "../../testing/asserts.ts";

Deno.test("NodeSyntaxError string representation", () => {
  assertEquals(
    String(new NodeSyntaxError("CODE", "MESSAGE")),
    "SyntaxError [CODE]: MESSAGE",
  );
});

Deno.test("NodeRangeError string representation", () => {
  assertEquals(
    String(new NodeRangeError("CODE", "MESSAGE")),
    "RangeError [CODE]: MESSAGE",
  );
});

Deno.test("NodeTypeError string representation", () => {
  assertEquals(
    String(new NodeTypeError("CODE", "MESSAGE")),
    "TypeError [CODE]: MESSAGE",
  );
});

Deno.test("NodeURIError string representation", () => {
  assertEquals(
    String(new NodeURIError("CODE", "MESSAGE")),
    "URIError [CODE]: MESSAGE",
  );
});
