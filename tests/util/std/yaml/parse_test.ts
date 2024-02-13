// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { parse, parseAll } from "./parse.ts";
import { assert, assertEquals, assertThrows } from "../assert/mod.ts";
import { DEFAULT_SCHEMA, EXTENDED_SCHEMA } from "./schema/mod.ts";
import { YAMLError } from "./_error.ts";
import { Type } from "./type.ts";

Deno.test({
  name: "`parse` parses single document yaml string",
  fn() {
    const yaml = `
      test: toto
      foo:
        bar: True
        baz: 1
        qux: ~
    `;

    const expected = { test: "toto", foo: { bar: true, baz: 1, qux: null } };

    assertEquals(parse(yaml), expected);
  },
});

Deno.test({
  name: "`parseAll` parses the yaml string with multiple documents",
  fn() {
    const yaml = `
---
id: 1
name: Alice
---
id: 2
name: Bob
---
id: 3
name: Eve
    `;
    const expected = [
      {
        id: 1,
        name: "Alice",
      },
      {
        id: 2,
        name: "Bob",
      },
      {
        id: 3,
        name: "Eve",
      },
    ];
    assertEquals(parseAll(yaml), expected);
  },
});

Deno.test({
  name: "`!!js/*` yaml types are not handled in default schemas while parsing",
  fn() {
    const yaml = `undefined: !!js/undefined ~`;
    assertThrows(() => parse(yaml), YAMLError, "unknown tag !");
  },
});

Deno.test({
  name:
    "`!!js/*` yaml types are correctly handled with extended schema while parsing",
  fn() {
    const yaml = `
      regexp:
        simple: !!js/regexp foobar
        modifiers: !!js/regexp /foobar/mi
      undefined: !!js/undefined ~
    `;

    const expected = {
      regexp: {
        simple: /foobar/,
        modifiers: /foobar/mi,
      },
      undefined: undefined,
    };

    assertEquals(parse(yaml, { schema: EXTENDED_SCHEMA }), expected);
  },
});

Deno.test({
  name: "`!!js/function` yaml type with extended schema throws while parsing",
  fn() {
    const func = function foobar() {
      return "hello world!";
    };

    const yaml = `
function: !!js/function >
${func.toString().split("\n").map((line) => `  ${line}`).join("\n")}
`;

    assertThrows(() => parse(yaml, { schema: EXTENDED_SCHEMA }));
  },
});

Deno.test({
  name: "`!*` yaml user defined types are supported while parsing",
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
    });
    const SPACE_SCHEMA = DEFAULT_SCHEMA.extend({ explicit: [PointYamlType] });

    const yaml = `
      point: !point [1, 2, 3]
    `;

    assertEquals(parse(yaml, { schema: SPACE_SCHEMA }), {
      point: { x: 1, y: 2, z: 3 },
    });
  },
});

Deno.test({
  name: "`parseAll` accepts parse options",
  fn() {
    const yaml = `
---
regexp: !!js/regexp foo
---
regexp: !!js/regexp bar
    `;

    const expected = [
      {
        regexp: /foo/,
      },
      {
        regexp: /bar/,
      },
    ];
    const mockCallback = () => {
      let count = 0;
      const fn = () => {
        count++;
      };
      const callback = {
        calls() {
          return count;
        },
        fn,
      };
      return callback;
    };

    assertEquals(parseAll(yaml, { schema: EXTENDED_SCHEMA }), expected);

    const callback = mockCallback();
    assertEquals(
      parseAll(yaml, callback.fn, { schema: EXTENDED_SCHEMA }),
      undefined,
    );
    assertEquals(callback.calls(), 2);
  },
});

Deno.test({
  name: "parse __proto__",
  async fn() {
    // Tests if the value is set using `Object.defineProperty(target, key, {value})`
    // instead of `target[key] = value` when parsing the object.
    // This makes a difference in behavior when __proto__ is set in Node.js and browsers.
    // Using `Object.defineProperty` avoids prototype pollution in Node.js and browsers.
    // reference: https://github.com/advisories/GHSA-9c47-m6qq-7p4h (CVE-2022-46175)

    const yaml1 = `
__proto__:
  isAdmin: true
    `;

    const yaml2 = `
anchor: &__proto__
  __proto__: 1111
alias_test:
  aaa: *__proto__
merge_test:
  bbb: 2222
  <<: *__proto__
    `;

    const testCode = `
      Object.defineProperty(Object.prototype, "__proto__", {
        set() {
          throw new Error("Don't try to set the value directly to the key __proto__.")
        }
      });
      import { parse } from "${import.meta.resolve("./parse.ts")}";
      parse(\`${yaml1}\`);
      parse(\`${yaml2}\`);
    `;
    const command = new Deno.Command(Deno.execPath(), {
      stdout: "inherit",
      stderr: "inherit",
      args: ["eval", testCode],
    });
    const { success } = await command.output();
    assert(success);
  },
});

Deno.test({
  name: "parse returns `null` when yaml is empty or only comments",
  fn() {
    const expected = null;

    const yaml1 = ``;
    assertEquals(parse(yaml1), expected);
    const yaml2 = ` \n\n `;
    assertEquals(parse(yaml2), expected);
    const yaml3 = `# just a bunch of comments \n # in this file`;
    assertEquals(parse(yaml3), expected);
  },
});

Deno.test({
  name: "parse binary type",
  fn() {
    const yaml = `message: !!binary "SGVsbG8="`;
    assertEquals(parse(yaml), {
      message: new Uint8Array([72, 101, 108, 108, 111]),
    });
  },
});
