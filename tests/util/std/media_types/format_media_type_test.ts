// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { formatMediaType } from "./mod.ts";

Deno.test({
  name: "media_types - formatMediaType",
  fn() {
    const fixtures = [
      ["noslash", { X: "Y" }, "noslash; x=Y"],
      ["foo bar/baz", undefined, ""],
      ["foo/bar baz", undefined, ""],
      [
        "attachment",
        { filename: "ĄĄŽŽČČŠŠ" },
        "attachment; filename*=utf-8''%C4%84%C4%84%C5%BD%C5%BD%C4%8C%C4%8C%C5%A0%C5%A0",
      ],
      [
        "attachment",
        { filename: "ÁÁÊÊÇÇÎÎ" },
        "attachment; filename*=utf-8''%C3%81%C3%81%C3%8A%C3%8A%C3%87%C3%87%C3%8E%C3%8E",
      ],
      [
        "attachment",
        { filename: "数据统计.png" },
        "attachment; filename*=utf-8''%E6%95%B0%E6%8D%AE%E7%BB%9F%E8%AE%A1.png",
      ],
      ["foo/BAR", undefined, "foo/bar"],
      ["foo/BAR", { "X": "Y" }, "foo/bar; x=Y"],
      ["foo/BAR", { "space": "With space" }, `foo/bar; space="With space"`],
      ["foo/BAR", { "quote": `With "quote` }, `foo/bar; quote="With \\"quote"`],
      [
        "foo/BAR",
        { "bslash": `With \\backslash` },
        `foo/bar; bslash="With \\\\backslash"`,
      ],
      [
        "foo/BAR",
        { "both": `With \\backslash and "quote` },
        `foo/bar; both="With \\\\backslash and \\"quote"`,
      ],
      ["foo/BAR", { "": "empty attribute" }, ""],
      ["foo/BAR", { "bad attribute": "baz" }, ""],
      [
        "foo/BAR",
        { "nonascii": "not an ascii character: ä" },
        "foo/bar; nonascii*=utf-8''not%20an%20ascii%20character%3A%20%C3%A4",
      ],
      [
        "foo/BAR",
        { "ctl": "newline: \n nil: \0" },
        "foo/bar; ctl*=utf-8''newline%3A%20%0A%20nil%3A%20%00",
      ],
      [
        "foo/bar",
        { "a": "av", "b": "bv", "c": "cv" },
        "foo/bar; a=av; b=bv; c=cv",
      ],
      ["foo/bar", { "0": "'", "9": "'" }, "foo/bar; 0='; 9='"],
      ["foo", { "bar": "" }, `foo; bar=""`],
    ] as const;
    for (const [type, param, expected] of fixtures) {
      assertEquals(formatMediaType(type, param), expected);
    }
  },
});
