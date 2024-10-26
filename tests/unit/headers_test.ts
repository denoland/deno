// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "./test_util.ts";
const {
  inspectArgs,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

Deno.test(function headersHasCorrectNameProp() {
  assertEquals(Headers.name, "Headers");
});

// Logic heavily copied from web-platform-tests, make
// sure pass mostly header basic test
// ref: https://github.com/web-platform-tests/wpt/blob/7c50c216081d6ea3c9afe553ee7b64534020a1b2/fetch/api/headers/headers-basic.html
Deno.test(function newHeaderTest() {
  new Headers();
  new Headers(undefined);
  new Headers({});
  try {
    // deno-lint-ignore no-explicit-any
    new Headers(null as any);
  } catch (e) {
    assert(e instanceof TypeError);
  }
});

const headerDict: Record<string, string> = {
  name1: "value1",
  name2: "value2",
  name3: "value3",
  // deno-lint-ignore no-explicit-any
  name4: undefined as any,
  "Content-Type": "value4",
};
// deno-lint-ignore no-explicit-any
const headerSeq: any[] = [];
for (const [name, value] of Object.entries(headerDict)) {
  headerSeq.push([name, value]);
}

Deno.test(function newHeaderWithSequence() {
  const headers = new Headers(headerSeq);
  for (const [name, value] of Object.entries(headerDict)) {
    assertEquals(headers.get(name), String(value));
  }
  assertEquals(headers.get("length"), null);
});

Deno.test(function newHeaderWithRecord() {
  const headers = new Headers(headerDict);
  for (const [name, value] of Object.entries(headerDict)) {
    assertEquals(headers.get(name), String(value));
  }
});

Deno.test(function newHeaderWithHeadersInstance() {
  const headers = new Headers(headerDict);
  const headers2 = new Headers(headers);
  for (const [name, value] of Object.entries(headerDict)) {
    assertEquals(headers2.get(name), String(value));
  }
});

Deno.test(function headerAppendSuccess() {
  const headers = new Headers();
  for (const [name, value] of Object.entries(headerDict)) {
    headers.append(name, value);
    assertEquals(headers.get(name), String(value));
  }
});

Deno.test(function headerSetSuccess() {
  const headers = new Headers();
  for (const [name, value] of Object.entries(headerDict)) {
    headers.set(name, value);
    assertEquals(headers.get(name), String(value));
  }
});

Deno.test(function headerHasSuccess() {
  const headers = new Headers(headerDict);
  for (const name of Object.keys(headerDict)) {
    assert(headers.has(name), "headers has name " + name);
    assert(
      !headers.has("nameNotInHeaders"),
      "headers do not have header: nameNotInHeaders",
    );
  }
});

Deno.test(function headerDeleteSuccess() {
  const headers = new Headers(headerDict);
  for (const name of Object.keys(headerDict)) {
    assert(headers.has(name), "headers have a header: " + name);
    headers.delete(name);
    assert(!headers.has(name), "headers do not have anymore a header: " + name);
  }
});

Deno.test(function headerGetSuccess() {
  const headers = new Headers(headerDict);
  for (const [name, value] of Object.entries(headerDict)) {
    assertEquals(headers.get(name), String(value));
    assertEquals(headers.get("nameNotInHeaders"), null);
  }
});

Deno.test(function headerEntriesSuccess() {
  const headers = new Headers(headerDict);
  const iterators = headers.entries();
  for (const it of iterators) {
    const key = it[0];
    const value = it[1];
    assert(headers.has(key));
    assertEquals(value, headers.get(key));
  }
});

Deno.test(function headerKeysSuccess() {
  const headers = new Headers(headerDict);
  const iterators = headers.keys();
  for (const it of iterators) {
    assert(headers.has(it));
  }
});

Deno.test(function headerValuesSuccess() {
  const headers = new Headers(headerDict);
  const iterators = headers.values();
  const entries = headers.entries();
  const values = [];
  for (const pair of entries) {
    values.push(pair[1]);
  }
  for (const it of iterators) {
    assert(values.includes(it));
  }
});

const headerEntriesDict: Record<string, string> = {
  name1: "value1",
  Name2: "value2",
  name: "value3",
  "content-Type": "value4",
  "Content-Typ": "value5",
  "Content-Types": "value6",
};

Deno.test(function headerForEachSuccess() {
  const headers = new Headers(headerEntriesDict);
  const keys = Object.keys(headerEntriesDict);
  keys.forEach((key) => {
    const value = headerEntriesDict[key];
    const newkey = key.toLowerCase();
    headerEntriesDict[newkey] = value;
  });
  let callNum = 0;
  headers.forEach((value, key, container) => {
    assertEquals(headers, container);
    assertEquals(value, headerEntriesDict[key]);
    callNum++;
  });
  assertEquals(callNum, keys.length);
});

Deno.test(function headerSymbolIteratorSuccess() {
  assert(Symbol.iterator in Headers.prototype);
  const headers = new Headers(headerEntriesDict);
  for (const header of headers) {
    const key = header[0];
    const value = header[1];
    assert(headers.has(key));
    assertEquals(value, headers.get(key));
  }
});

Deno.test(function headerTypesAvailable() {
  function newHeaders(): Headers {
    return new Headers();
  }
  const headers = newHeaders();
  assert(headers instanceof Headers);
});

// Modified from https://github.com/bitinn/node-fetch/blob/7d3293200a91ad52b5ca7962f9d6fd1c04983edb/test/test.js#L2001-L2014
// Copyright (c) 2016 David Frank. MIT License.
Deno.test(function headerIllegalReject() {
  let errorCount = 0;
  try {
    new Headers({ "He y": "ok" });
  } catch (_e) {
    errorCount++;
  }
  try {
    new Headers({ "Hé-y": "ok" });
  } catch (_e) {
    errorCount++;
  }
  try {
    new Headers({ "He-y": "ăk" });
  } catch (_e) {
    errorCount++;
  }
  const headers = new Headers();
  try {
    headers.append("Hé-y", "ok");
  } catch (_e) {
    errorCount++;
  }
  try {
    headers.delete("Hé-y");
  } catch (_e) {
    errorCount++;
  }
  try {
    headers.get("Hé-y");
  } catch (_e) {
    errorCount++;
  }
  try {
    headers.has("Hé-y");
  } catch (_e) {
    errorCount++;
  }
  try {
    headers.set("Hé-y", "ok");
  } catch (_e) {
    errorCount++;
  }
  try {
    headers.set("", "ok");
  } catch (_e) {
    errorCount++;
  }
  assertEquals(errorCount, 9);
  // 'o k' is valid value but invalid name
  new Headers({ "He-y": "o k" });
});

// If pair does not contain exactly two items,then throw a TypeError.
Deno.test(function headerParamsShouldThrowTypeError() {
  let hasThrown = 0;

  try {
    new Headers(([["1"]] as unknown) as Array<[string, string]>);
    hasThrown = 1;
  } catch (err) {
    if (err instanceof TypeError) {
      hasThrown = 2;
    } else {
      hasThrown = 3;
    }
  }

  assertEquals(hasThrown, 2);
});

Deno.test(function headerParamsArgumentsCheck() {
  const methodRequireOneParam = ["delete", "get", "has", "forEach"] as const;

  const methodRequireTwoParams = ["append", "set"] as const;

  methodRequireOneParam.forEach((method) => {
    const headers = new Headers();
    let hasThrown = 0;
    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]();
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
  });

  methodRequireTwoParams.forEach((method) => {
    const headers = new Headers();
    let hasThrown = 0;

    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]();
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);

    hasThrown = 0;
    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]("foo");
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
  });
});

Deno.test(function headersInitMultiple() {
  const headers = new Headers([
    ["Set-Cookie", "foo=bar"],
    ["Set-Cookie", "bar=baz"],
    ["X-Deno", "foo"],
    ["X-Deno", "bar"],
  ]);
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=bar"],
    ["set-cookie", "bar=baz"],
    ["x-deno", "foo, bar"],
  ]);
});

Deno.test(function headerInitWithPrototypePollution() {
  const originalExec = RegExp.prototype.exec;
  try {
    RegExp.prototype.exec = () => {
      throw Error();
    };
    new Headers([
      ["X-Deno", "foo"],
      ["X-Deno", "bar"],
    ]);
  } finally {
    RegExp.prototype.exec = originalExec;
  }
});

Deno.test(function headersAppendMultiple() {
  const headers = new Headers([
    ["Set-Cookie", "foo=bar"],
    ["X-Deno", "foo"],
  ]);
  headers.append("set-Cookie", "bar=baz");
  headers.append("x-Deno", "bar");
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=bar"],
    ["set-cookie", "bar=baz"],
    ["x-deno", "foo, bar"],
  ]);
});

Deno.test(function headersAppendDuplicateSetCookieKey() {
  const headers = new Headers([["Set-Cookie", "foo=bar"]]);
  headers.append("set-Cookie", "foo=baz");
  headers.append("Set-cookie", "baz=bar");
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=bar"],
    ["set-cookie", "foo=baz"],
    ["set-cookie", "baz=bar"],
  ]);
});

Deno.test(function headersGetSetCookie() {
  const headers = new Headers([
    ["Set-Cookie", "foo=bar"],
    ["set-Cookie", "bar=qat"],
  ]);
  assertEquals(headers.get("SET-COOKIE"), "foo=bar, bar=qat");
});

Deno.test(function toStringShouldBeWebCompatibility() {
  const headers = new Headers();
  assertEquals(headers.toString(), "[object Headers]");
});

function stringify(...args: unknown[]): string {
  return inspectArgs(args).replace(/\n$/, "");
}

Deno.test(function customInspectReturnsCorrectHeadersFormat() {
  const blankHeaders = new Headers();
  assertEquals(stringify(blankHeaders), "Headers {}");
  const singleHeader = new Headers([["Content-Type", "application/json"]]);
  assertEquals(
    stringify(singleHeader),
    `Headers { "content-type": "application/json" }`,
  );
  const multiParamHeader = new Headers([
    ["Content-Type", "application/json"],
    ["Content-Length", "1337"],
  ]);
  assertEquals(
    stringify(multiParamHeader),
    `Headers { "content-length": "1337", "content-type": "application/json" }`,
  );
});

Deno.test(function invalidHeadersFlaky() {
  assertThrows(
    () => new Headers([["x", "\u0000x"]]),
    TypeError,
    'Invalid header value: "\u0000x"',
  );
  assertThrows(
    () => new Headers([["x", "\u0000x"]]),
    TypeError,
    'Invalid header value: "\u0000x"',
  );
});
