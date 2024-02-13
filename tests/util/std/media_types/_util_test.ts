// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { consumeMediaParam, consumeToken, consumeValue } from "./_util.ts";

Deno.test({
  name: "media_types::util - consumeToken()",
  fn() {
    const fixtures = [
      ["foo bar", "foo", " bar"],
      ["bar", "bar", ""],
      ["", "", ""],
      [" foo", "", " foo"],
    ] as const;
    for (const [fixture, token, rest] of fixtures) {
      assertEquals(consumeToken(fixture), [token, rest]);
    }
  },
});

Deno.test({
  name: "media_types::util - consumeValue()",
  fn() {
    const fixtures = [
      ["foo bar", "foo", " bar"],
      ["bar", "bar", ""],
      [" bar ", "", " bar "],
      [`"My value"end`, "My value", "end"],
      [`"My value" end`, "My value", " end"],
      [`"\\\\" rest`, "\\", " rest"],
      [`"My \\" value"end`, 'My " value', "end"],
      [`"\\" rest`, "", `"\\" rest`],
      [`"C:\\dev\\go\\robots.txt"`, `C:\\dev\\go\\robots.txt`, ""],
      [
        `"C:\\新建文件夹\\中文第二次测试.mp4"`,
        `C:\\新建文件夹\\中文第二次测试.mp4`,
        "",
      ],
    ] as const;
    for (const [fixture, value, rest] of fixtures) {
      assertEquals(consumeValue(fixture), [value, rest]);
    }
  },
});

Deno.test({
  name: "media_types::util - consumeMediaParam()",
  fn() {
    const fixtures = [
      [" ; foo=bar", "foo", "bar", ""],
      ["; foo=bar", "foo", "bar", ""],
      [";foo=bar", "foo", "bar", ""],
      [";FOO=bar", "foo", "bar", ""],
      [`;foo="bar"`, "foo", "bar", ""],
      [`;foo="bar"; `, "foo", "bar", "; "],
      [`;foo="bar"; foo=baz`, "foo", "bar", "; foo=baz"],
      [` ; boundary=----CUT;`, "boundary", "----CUT", ";"],
      [
        ` ; key=value;  blah="value";name="foo" `,
        "key",
        "value",
        `;  blah="value";name="foo" `,
      ],
      [`;  blah="value";name="foo" `, "blah", "value", `;name="foo" `],
      [`;name="foo" `, "name", "foo", ` `],
    ] as const;
    for (const [fixture, key, value, rest] of fixtures) {
      assertEquals(consumeMediaParam(fixture), [key, value, rest]);
    }
  },
});
