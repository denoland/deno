// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { stringify } from "./stringify.ts";
import { YAMLError } from "./_error.ts";
import { DEFAULT_SCHEMA, EXTENDED_SCHEMA } from "./schema/mod.ts";
import { Type } from "./type.ts";

Deno.test({
  name: "stringified correctly",
  fn() {
    const FIXTURE = {
      foo: {
        bar: true,
        test: [
          "a",
          "b",
          {
            a: false,
          },
          {
            a: false,
          },
        ],
      },
      test: "foobar",
      binary: new Uint8Array([72, 101, 108, 108, 111]),
    };

    const ASSERTS = `foo:
  bar: true
  test:
    - a
    - b
    - a: false
    - a: false
test: foobar
binary: !<tag:yaml.org,2002:binary> SGVsbG8=
`;

    assertEquals(stringify(FIXTURE), ASSERTS);
  },
});

Deno.test({
  name:
    "`!!js/*` yaml types are not handled in default schemas while stringifying",
  fn() {
    const object = { undefined: undefined };
    assertThrows(
      () => stringify(object),
      YAMLError,
      "unacceptable kind of an object to dump",
    );
  },
});

Deno.test({
  name:
    "`!!js/*` yaml types are correctly handled with extended schema while stringifying",
  fn() {
    const object = {
      regexp: {
        simple: /foobar/,
        modifiers: /foobar/im,
      },
      undefined: undefined,
    };

    const expected = `regexp:
  simple: !<tag:yaml.org,2002:js/regexp> /foobar/
  modifiers: !<tag:yaml.org,2002:js/regexp> /foobar/im
undefined: !<tag:yaml.org,2002:js/undefined> ''
`;

    assertEquals(stringify(object, { schema: EXTENDED_SCHEMA }), expected);
  },
});

Deno.test({
  name: "`!!js/function` yaml with extended schema throws while stringifying",
  fn() {
    const func = function foobar() {
      return "hello world!";
    };

    assertThrows(
      () => stringify({ function: func }, { schema: EXTENDED_SCHEMA }),
    );
  },
});

Deno.test({
  name: "`!*` yaml user defined types are supported while stringifying",
  fn() {
    const PointYamlType = new Type("!point", {
      kind: "sequence",
      resolve(data) {
        return data !== null && data?.length === 3;
      },
      construct(data) {
        const [x, y, z] = data;
        return { x, y, z };
      },
      predicate(object: unknown) {
        return !!(object && typeof object === "object" && "x" in object &&
          "y" in object && "z" in object);
      },
      represent(point) {
        return [point.x, point.y, point.z];
      },
    });
    const SPACE_SCHEMA = DEFAULT_SCHEMA.extend({ explicit: [PointYamlType] });

    const object = {
      point: { x: 1, y: 2, z: 3 },
    };

    const expected = `point: !<!point>${" "}
  - 1
  - 2
  - 3
`;

    assertEquals(stringify(object, { schema: SPACE_SCHEMA }), expected);
  },
});
