import * as qs from "./querystring.ts";
import { test } from "../testing/mod.ts";
import { assertEquals, assertThrows, assert } from "../testing/asserts.ts";

test(function qsBase() {
  function createWithNoPrototype(properties) {
    const noProto = Object.create(null);
    properties.forEach(property => {
      noProto[property.key] = property.value;
    });
    return noProto;
  }
  // Folding block, commented to pass gjslint
  // {{{
  // [ wonkyQS, canonicalQS, obj ]
  const qsTestCases = [
    [
      "__proto__=1",
      "__proto__=1",
      createWithNoPrototype([{ key: "__proto__", value: "1" }])
    ],
    [
      "__defineGetter__=asdf",
      "__defineGetter__=asdf",
      JSON.parse('{"__defineGetter__":"asdf"}')
    ],
    [
      "foo=918854443121279438895193",
      "foo=918854443121279438895193",
      { foo: "918854443121279438895193" }
    ],
    ["foo=bar", "foo=bar", { foo: "bar" }],
    ["foo=bar&foo=quux", "foo=bar&foo=quux", { foo: ["bar", "quux"] }],
    ["foo=1&bar=2", "foo=1&bar=2", { foo: "1", bar: "2" }],
    [
      "my+weird+field=q1%212%22%27w%245%267%2Fz8%29%3F",
      "my%20weird%20field=q1!2%22'w%245%267%2Fz8)%3F",
      { "my weird field": "q1!2\"'w$5&7/z8)?" }
    ],
    ["foo%3Dbaz=bar", "foo%3Dbaz=bar", { "foo=baz": "bar" }],
    ["foo=baz=bar", "foo=baz%3Dbar", { foo: "baz=bar" }],
    [
      "str=foo&arr=1&arr=2&arr=3&somenull=&undef=",
      "str=foo&arr=1&arr=2&arr=3&somenull=&undef=",
      { str: "foo", arr: ["1", "2", "3"], somenull: "", undef: "" }
    ],
    [" foo = bar ", "%20foo%20=%20bar%20", { " foo ": " bar " }],
    ["foo=%zx", "foo=%25zx", { foo: "%zx" }],
    ["foo=%EF%BF%BD", "foo=%EF%BF%BD", { foo: "\ufffd" }],
    // See: https://github.com/joyent/node/issues/1707
    [
      "hasOwnProperty=x&toString=foo&valueOf=bar&__defineGetter__=baz",
      "hasOwnProperty=x&toString=foo&valueOf=bar&__defineGetter__=baz",
      {
        hasOwnProperty: "x",
        toString: "foo",
        valueOf: "bar",
        __defineGetter__: "baz"
      }
    ],
    // See: https://github.com/joyent/node/issues/3058
    ["foo&bar=baz", "foo=&bar=baz", { foo: "", bar: "baz" }],
    ["a=b&c&d=e", "a=b&c=&d=e", { a: "b", c: "", d: "e" }],
    ["a=b&c=&d=e", "a=b&c=&d=e", { a: "b", c: "", d: "e" }],
    ["a=b&=c&d=e", "a=b&=c&d=e", { a: "b", "": "c", d: "e" }],
    ["a=b&=&c=d", "a=b&=&c=d", { a: "b", "": "", c: "d" }],
    ["&&foo=bar&&", "foo=bar", { foo: "bar" }],
    ["&", "", {}],
    ["&&&&", "", {}],
    ["&=&", "=", { "": "" }],
    ["&=&=", "=&=", { "": ["", ""] }],
    ["=", "=", { "": "" }],
    ["+", "%20=", { " ": "" }],
    ["+=", "%20=", { " ": "" }],
    ["+&", "%20=", { " ": "" }],
    ["=+", "=%20", { "": " " }],
    ["+=&", "%20=", { " ": "" }],
    ["a&&b", "a=&b=", { a: "", b: "" }],
    ["a=a&&b=b", "a=a&b=b", { a: "a", b: "b" }],
    ["&a", "a=", { a: "" }],
    ["&=", "=", { "": "" }],
    ["a&a&", "a=&a=", { a: ["", ""] }],
    ["a&a&a&", "a=&a=&a=", { a: ["", "", ""] }],
    ["a&a&a&a&", "a=&a=&a=&a=", { a: ["", "", "", ""] }],
    ["a=&a=value&a=", "a=&a=value&a=", { a: ["", "value", ""] }],
    ["foo+bar=baz+quux", "foo%20bar=baz%20quux", { "foo bar": "baz quux" }],
    ["+foo=+bar", "%20foo=%20bar", { " foo": " bar" }],
    ["a+", "a%20=", { "a ": "" }],
    ["=a+", "=a%20", { "": "a " }],
    ["a+&", "a%20=", { "a ": "" }],
    ["=a+&", "=a%20", { "": "a " }],
    ["%20+", "%20%20=", { "  ": "" }],
    ["=%20+", "=%20%20", { "": "  " }],
    ["%20+&", "%20%20=", { "  ": "" }],
    ["=%20+&", "=%20%20", { "": "  " }],
    [null, "", {}],
    [undefined, "", {}]
  ];

  // [ wonkyQS, canonicalQS, obj ]
  const qsColonTestCases = [
    ["foo:bar", "foo:bar", { foo: "bar" }],
    ["foo:bar;foo:quux", "foo:bar;foo:quux", { foo: ["bar", "quux"] }],
    [
      "foo:1&bar:2;baz:quux",
      "foo:1%26bar%3A2;baz:quux",
      { foo: "1&bar:2", baz: "quux" }
    ],
    ["foo%3Abaz:bar", "foo%3Abaz:bar", { "foo:baz": "bar" }],
    ["foo:baz:bar", "foo:baz%3Abar", { foo: "baz:bar" }]
  ];

  // [wonkyObj, qs, canonicalObj]
  function extendedFunction() {}
  extendedFunction.prototype = { a: "b" };
  const qsWeirdObjects = [
    // eslint-disable-next-line node-core/no-unescaped-regexp-dot
    [{ regexp: /./g }, "regexp=", { regexp: "" }],
    // eslint-disable-next-line node-core/no-unescaped-regexp-dot
    [{ regexp: new RegExp(".", "g") }, "regexp=", { regexp: "" }],
    [{ fn: () => {} }, "fn=", { fn: "" }],
    [{ fn: new Function("") }, "fn=", { fn: "" }],
    [{ math: Math }, "math=", { math: "" }],
    [{ e: extendedFunction }, "e=", { e: "" }],
    [{ d: new Date() }, "d=", { d: "" }],
    [{ d: Date }, "d=", { d: "" }],
    [
      { f: new Boolean(false), t: new Boolean(true) },
      "f=&t=",
      { f: "", t: "" }
    ],
    [{ f: false, t: true }, "f=false&t=true", { f: "false", t: "true" }],
    [{ n: null }, "n=", { n: "" }],
    [{ nan: NaN }, "nan=", { nan: "" }],
    [{ inf: Infinity }, "inf=", { inf: "" }],
    [{ a: [], b: [] }, "", {}]
  ];
  // }}}

  const qsNoMungeTestCases = [
    ["", {}],
    ["foo=bar&foo=baz", { foo: ["bar", "baz"] }],
    ["blah=burp", { blah: "burp" }],
    ["a=!-._~'()*", { a: "!-._~'()*" }],
    ["a=abcdefghijklmnopqrstuvwxyz", { a: "abcdefghijklmnopqrstuvwxyz" }],
    ["a=ABCDEFGHIJKLMNOPQRSTUVWXYZ", { a: "ABCDEFGHIJKLMNOPQRSTUVWXYZ" }],
    ["a=0123456789", { a: "0123456789" }],
    ["gragh=1&gragh=3&goo=2", { gragh: ["1", "3"], goo: "2" }],
    [
      "frappucino=muffin&goat%5B%5D=scone&pond=moose",
      { frappucino: "muffin", "goat[]": "scone", pond: "moose" }
    ],
    ["trololol=yes&lololo=no", { trololol: "yes", lololo: "no" }]
  ];

  const qsUnescapeTestCases = [
    ["there is nothing to unescape here", "there is nothing to unescape here"],
    [
      "there%20are%20several%20spaces%20that%20need%20to%20be%20unescaped",
      "there are several spaces that need to be unescaped"
    ],
    [
      "there%2Qare%0-fake%escaped values in%%%%this%9Hstring",
      "there%2Qare%0-fake%escaped values in%%%%this%9Hstring"
    ],
    [
      "%20%21%22%23%24%25%26%27%28%29%2A%2B%2C%2D%2E%2F%30%31%32%33%34%35%36%37",
      " !\"#$%&'()*+,-./01234567"
    ]
  ];

  assertEquals(
    qs.parse("id=918854443121279438895193").id,
    "918854443121279438895193"
  );

  // TODO: wait for `util.inspect`
  function check(actual: unknown, expected: unknown, input?: unknown) {
    assertEquals(actual, expected);
  }

  // Test that the canonical qs is parsed properly.
  qsTestCases.forEach(testCase => {
    check(qs.parse(testCase[0]), testCase[2], testCase[0]);
  });

  // Test that the colon test cases can do the same
  qsColonTestCases.forEach(testCase => {
    check(qs.parse(testCase[0] as any, ";", ":"), testCase[2], testCase[0]);
  });

  // Test the weird objects, that they get parsed properly
  qsWeirdObjects.forEach(testCase => {
    check(qs.parse(testCase[1] as any), testCase[2], testCase[1]);
  });

  qsNoMungeTestCases.forEach(testCase => {
    assertEquals(qs.stringify(testCase[1], "&", "="), testCase[0]);
  });

  // Test the nested qs-in-qs case
  {
    const f = qs.parse("a=b&q=x%3Dy%26y%3Dz");
    check(
      f,
      createWithNoPrototype([
        { key: "a", value: "b" },
        { key: "q", value: "x=y&y=z" }
      ])
    );

    f.q = qs.parse(f.q as any) as any;
    const expectedInternal = createWithNoPrototype([
      { key: "x", value: "y" },
      { key: "y", value: "z" }
    ]);
    check(f.q, expectedInternal);
  }

  // nested in colon
  {
    const f = qs.parse("a:b;q:x%3Ay%3By%3Az", ";", ":");
    check(
      f,
      createWithNoPrototype([
        { key: "a", value: "b" },
        { key: "q", value: "x:y;y:z" }
      ])
    );
    f.q = qs.parse(f.q as any, ";", ":") as any;
    const expectedInternal = createWithNoPrototype([
      { key: "x", value: "y" },
      { key: "y", value: "z" }
    ]);
    check(f.q, expectedInternal);
  }

  // Now test stringifying

  // basic
  qsTestCases.forEach(testCase => {
    assertEquals(qs.stringify(testCase[2]), testCase[1]);
  });

  qsColonTestCases.forEach(testCase => {
    assertEquals(qs.stringify(testCase[2] as any, ";", ":"), testCase[1]);
  });

  qsWeirdObjects.forEach(testCase => {
    assertEquals(qs.stringify(testCase[0] as any), testCase[1]);
  });

  // Invalid surrogate pair throws URIError
  assertThrows(
    () => qs.stringify({ foo: "\udc00" }),
    URIError,
    "URI malformed",
    "URI malformed"
  );

  // Coerce numbers to string
  assertEquals(qs.stringify({ foo: 0 }), "foo=0");
  assertEquals(qs.stringify({ foo: -0 }), "foo=0");
  assertEquals(qs.stringify({ foo: 3 }), "foo=3");
  assertEquals(qs.stringify({ foo: -72.42 }), "foo=-72.42");
  assertEquals(qs.stringify({ foo: NaN }), "foo=");
  assertEquals(qs.stringify({ foo: Infinity }), "foo=");

  // nested
  {
    const f = qs.stringify({
      a: "b",
      q: qs.stringify({
        x: "y",
        y: "z"
      })
    });
    assertEquals(f, "a=b&q=x%3Dy%26y%3Dz");
  }

  qs.parse(undefined); // Should not throw.

  // nested in colon
  {
    const f = qs.stringify(
      {
        a: "b",
        q: qs.stringify(
          {
            x: "y",
            y: "z"
          },
          ";",
          ":"
        )
      },
      ";",
      ":"
    );
    assertEquals(f, "a:b;q:x%3Ay%3By%3Az");
  }

  // empty string
  assertEquals(qs.stringify(), "");
  assertEquals(qs.stringify(0 as any), "");
  assertEquals(qs.stringify([] as any), "");
  assertEquals(qs.stringify(null), "");
  assertEquals(qs.stringify(true as any), "");

  check(qs.parse(undefined), {});

  // empty sep
  check(qs.parse("a", [] as any), { a: "" });

  // empty eq
  check(qs.parse("a", null, [] as any), { "": "a" });

  // Test limiting
  assertEquals(
    Object.keys(qs.parse("a=1&b=1&c=1", null, null, { maxKeys: 1 })).length,
    1
  );

  // Test limiting with a case that starts from `&`
  assertEquals(
    Object.keys(qs.parse("&a", null, null, { maxKeys: 1 })).length,
    0
  );

  // Test removing limit
  {
    function testUnlimitedKeys() {
      const query = {};

      for (let i = 0; i < 2000; i++) query[i] = i;

      const url = qs.stringify(query);

      assertEquals(
        Object.keys(qs.parse(url, null, null, { maxKeys: 0 })).length,
        2000
      );
    }

    testUnlimitedKeys();
  }

  {
    const b = qs.unescapeBuffer(
      "%d3%f2Ug%1f6v%24%5e%98%cb" + "%0d%ac%a2%2f%9d%eb%d8%a2%e6"
    );
    // <Buffer d3 f2 55 67 1f 36 76 24 5e 98 cb 0d ac a2 2f 9d eb d8 a2 e6>
    assertEquals(b[0], 0xd3);
    assertEquals(b[1], 0xf2);
    assertEquals(b[2], 0x55);
    assertEquals(b[3], 0x67);
    assertEquals(b[4], 0x1f);
    assertEquals(b[5], 0x36);
    assertEquals(b[6], 0x76);
    assertEquals(b[7], 0x24);
    assertEquals(b[8], 0x5e);
    assertEquals(b[9], 0x98);
    assertEquals(b[10], 0xcb);
    assertEquals(b[11], 0x0d);
    assertEquals(b[12], 0xac);
    assertEquals(b[13], 0xa2);
    assertEquals(b[14], 0x2f);
    assertEquals(b[15], 0x9d);
    assertEquals(b[16], 0xeb);
    assertEquals(b[17], 0xd8);
    assertEquals(b[18], 0xa2);
    assertEquals(b[19], 0xe6);
  }

  assertEquals(qs.unescapeBuffer("a+b", true).toString(), "a b");
  assertEquals(qs.unescapeBuffer("a+b").toString(), "a+b");
  assertEquals(qs.unescapeBuffer("a%").toString(), "a%");
  assertEquals(qs.unescapeBuffer("a%2").toString(), "a%2");
  assertEquals(qs.unescapeBuffer("a%20").toString(), "a ");
  assertEquals(qs.unescapeBuffer("a%2g").toString(), "a%2g");
  assertEquals(qs.unescapeBuffer("a%%").toString(), "a%%");

  // Test invalid encoded string
  check(qs.parse("%\u0100=%\u0101"), { "%Ā": "%ā" });

  // Test custom decode
  {
    function demoDecode(str) {
      return str + str;
    }

    check(
      qs.parse("a=a&b=b&c=c", null, null, { decodeURIComponent: demoDecode }),
      { aa: "aa", bb: "bb", cc: "cc" }
    );
    check(
      qs.parse("a=a&b=b&c=c", null, "==", { decodeURIComponent: str => str }),
      { "a=a": "", "b=b": "", "c=c": "" }
    );
  }

  // Test QueryString.unescape
  {
    function errDecode(str) {
      throw new Error("To jump to the catch scope");
    }

    check(
      qs.parse("a=a", null, null, { decodeURIComponent: errDecode as any }),
      {
        a: "a"
      }
    );
  }

  // Test custom encode
  {
    function demoEncode(str) {
      return str[0];
    }

    const obj = { aa: "aa", bb: "bb", cc: "cc" };
    assertEquals(
      qs.stringify(obj, null, null, { encodeURIComponent: demoEncode }),
      "a=a&b=b&c=c"
    );
  }

  // Test QueryString.unescapeBuffer
  qsUnescapeTestCases.forEach(testCase => {
    assertEquals(qs.unescape(testCase[0]), testCase[1]);
    // assertEquals(qs.unescapeBuffer(testCase[0]).toString(), testCase[1]);// TODO
  });

  // Test separator and "equals" parsing order
  check(qs.parse("foo&bar", "&", "&"), { foo: "", bar: "" });
});

test(function qsMulticharSeparator() {
  assertEquals(qs.parse("foo=>bar&&bar=>baz", "&&", "=>"), {
    foo: "bar",
    bar: "baz"
  });

  assertEquals(
    qs.stringify({ foo: "bar", bar: "baz" }, "&&", "=>"),
    "foo=>bar&&bar=>baz"
  );

  assertEquals(qs.parse("foo==>bar, bar==>baz", ", ", "==>"), {
    foo: "bar",
    bar: "baz"
  });

  assertEquals(
    qs.stringify({ foo: "bar", bar: "baz" }, ", ", "==>"),
    "foo==>bar, bar==>baz"
  );
});

test(function qsEscape() {
  assertEquals(qs.escape(5 as any), "5");
  assertEquals(qs.escape("test"), "test");
  assertEquals(qs.escape({} as any), "%5Bobject%20Object%5D");
  assertEquals(qs.escape([5, 10] as any), "5%2C10");
  assertEquals(qs.escape("Ŋōđĕ"), "%C5%8A%C5%8D%C4%91%C4%95");
  assertEquals(qs.escape("testŊōđĕ"), "test%C5%8A%C5%8D%C4%91%C4%95");
  assertEquals(
    qs.escape(`${String.fromCharCode(0xd800 + 1)}test`),
    "%F0%90%91%B4est"
  );

  assertThrows(
    () => qs.escape(String.fromCharCode(0xd800 + 1)),
    URIError,
    "URI malformed",
    "URI malformed"
  );
  assertThrows(() => qs.escape({ toString: 5 } as any), TypeError);
  assertThrows(() => qs.escape(Symbol("test") as any), TypeError);

  assertEquals(
    qs.escape({ test: 5, toString: () => "test", valueOf: () => 10 } as any),
    "test"
  );
  assertEquals(
    qs.escape({ toString: 5, valueOf: () => "test" } as any),
    "test"
  );
});

test(function qsMaxKeysNonFinite() {
  function createManyParams(count) {
    let str = "";

    if (count === 0) {
      return str;
    }

    str += "0=0";

    for (let i = 1; i < count; i++) {
      const n = i.toString(36);
      str += `&${n}=${n}`;
    }

    return str;
  }

  const count = 10000;
  const originalMaxLength = 1000;
  const params = createManyParams(count);

  // thealphanerd
  // 27def4f introduced a change to parse that would cause Infinity
  // to be passed to String.prototype.split as an argument for limit
  // In this instance split will always return an empty array
  // this test confirms that the output of parse is the expected length
  // when passed Infinity as the argument for maxKeys
  const resultInfinity = qs.parse(params, undefined, undefined, {
    maxKeys: Infinity
  });
  const resultNaN = qs.parse(params, undefined, undefined, {
    maxKeys: NaN
  });
  const resultInfinityString = qs.parse(params, undefined, undefined, {
    maxKeys: "Infinity" as any
  });
  const resultNaNString = qs.parse(params, undefined, undefined, {
    maxKeys: "NaN" as any
  });

  // Non Finite maxKeys should return the length of input
  assertEquals(Object.keys(resultInfinity).length, count);
  assertEquals(Object.keys(resultNaN).length, count);
  // Strings maxKeys should return the maxLength
  // defined by parses internals
  assertEquals(Object.keys(resultInfinityString).length, originalMaxLength);
  assertEquals(Object.keys(resultNaNString).length, originalMaxLength);
});
