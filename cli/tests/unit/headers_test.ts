// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  unitTest,
} from "./test_util.ts";
const {
  inspectArgs,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

unitTest(function headersHasCorrectNameProp(): void {
  assertEquals(Headers.name, "Headers");
});

// Logic heavily copied from web-platform-tests, make
// sure pass mostly header basic test
// ref: https://github.com/web-platform-tests/wpt/blob/7c50c216081d6ea3c9afe553ee7b64534020a1b2/fetch/api/headers/headers-basic.html
unitTest(function newHeaderTest(): void {
  new Headers();
  new Headers(undefined);
  new Headers({});
  try {
    // deno-lint-ignore no-explicit-any
    new Headers(null as any);
  } catch (e) {
    assertEquals(
      e.message,
      "Failed to construct 'Headers'; The provided value was not valid",
    );
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
for (const name in headerDict) {
  headerSeq.push([name, headerDict[name]]);
}

unitTest(function newHeaderWithSequence(): void {
  const headers = new Headers(headerSeq);
  for (const name in headerDict) {
    assertEquals(headers.get(name), String(headerDict[name]));
  }
  assertEquals(headers.get("length"), null);
});

unitTest(function newHeaderWithRecord(): void {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assertEquals(headers.get(name), String(headerDict[name]));
  }
});

unitTest(function newHeaderWithHeadersInstance(): void {
  const headers = new Headers(headerDict);
  const headers2 = new Headers(headers);
  for (const name in headerDict) {
    assertEquals(headers2.get(name), String(headerDict[name]));
  }
});

unitTest(function headerAppendSuccess(): void {
  const headers = new Headers();
  for (const name in headerDict) {
    headers.append(name, headerDict[name]);
    assertEquals(headers.get(name), String(headerDict[name]));
  }
});

unitTest(function headerSetSuccess(): void {
  const headers = new Headers();
  for (const name in headerDict) {
    headers.set(name, headerDict[name]);
    assertEquals(headers.get(name), String(headerDict[name]));
  }
});

unitTest(function headerHasSuccess(): void {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assert(headers.has(name), "headers has name " + name);
    assert(
      !headers.has("nameNotInHeaders"),
      "headers do not have header: nameNotInHeaders",
    );
  }
});

unitTest(function headerDeleteSuccess(): void {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assert(headers.has(name), "headers have a header: " + name);
    headers.delete(name);
    assert(!headers.has(name), "headers do not have anymore a header: " + name);
  }
});

unitTest(function headerGetSuccess(): void {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assertEquals(headers.get(name), String(headerDict[name]));
    assertEquals(headers.get("nameNotInHeaders"), null);
  }
});

unitTest(function headerEntriesSuccess(): void {
  const headers = new Headers(headerDict);
  const iterators = headers.entries();
  for (const it of iterators) {
    const key = it[0];
    const value = it[1];
    assert(headers.has(key));
    assertEquals(value, headers.get(key));
  }
});

unitTest(function headerKeysSuccess(): void {
  const headers = new Headers(headerDict);
  const iterators = headers.keys();
  for (const it of iterators) {
    assert(headers.has(it));
  }
});

unitTest(function headerValuesSuccess(): void {
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

unitTest(function headerForEachSuccess(): void {
  const headers = new Headers(headerEntriesDict);
  const keys = Object.keys(headerEntriesDict);
  keys.forEach((key): void => {
    const value = headerEntriesDict[key];
    const newkey = key.toLowerCase();
    headerEntriesDict[newkey] = value;
  });
  let callNum = 0;
  headers.forEach((value, key, container): void => {
    assertEquals(headers, container);
    assertEquals(value, headerEntriesDict[key]);
    callNum++;
  });
  assertEquals(callNum, keys.length);
});

unitTest(function headerSymbolIteratorSuccess(): void {
  assert(Symbol.iterator in Headers.prototype);
  const headers = new Headers(headerEntriesDict);
  for (const header of headers) {
    const key = header[0];
    const value = header[1];
    assert(headers.has(key));
    assertEquals(value, headers.get(key));
  }
});

unitTest(function headerTypesAvailable(): void {
  function newHeaders(): Headers {
    return new Headers();
  }
  const headers = newHeaders();
  assert(headers instanceof Headers);
});

// Modified from https://github.com/bitinn/node-fetch/blob/7d3293200a91ad52b5ca7962f9d6fd1c04983edb/test/test.js#L2001-L2014
// Copyright (c) 2016 David Frank. MIT License.
unitTest(function headerIllegalReject(): void {
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
unitTest(function headerParamsShouldThrowTypeError(): void {
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

unitTest(function headerParamsArgumentsCheck(): void {
  const methodRequireOneParam = ["delete", "get", "has", "forEach"] as const;

  const methodRequireTwoParams = ["append", "set"] as const;

  methodRequireOneParam.forEach((method): void => {
    const headers = new Headers();
    let hasThrown = 0;
    let errMsg = "";
    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
    assertStringIncludes(
      errMsg,
      `${method} requires at least 1 argument, but only 0 present`,
    );
  });

  methodRequireTwoParams.forEach((method): void => {
    const headers = new Headers();
    let hasThrown = 0;
    let errMsg = "";

    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
    assertStringIncludes(
      errMsg,
      `${method} requires at least 2 arguments, but only 0 present`,
    );

    hasThrown = 0;
    errMsg = "";
    try {
      // deno-lint-ignore no-explicit-any
      (headers as any)[method]("foo");
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
    assertStringIncludes(
      errMsg,
      `${method} requires at least 2 arguments, but only 1 present`,
    );
  });
});

unitTest(function headersInitMultiple(): void {
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

unitTest(function headersAppendMultiple(): void {
  const headers = new Headers([
    ["Set-Cookie", "foo=bar"],
    ["X-Deno", "foo"],
  ]);
  headers.append("set-Cookie", "bar=baz");
  headers.append("x-Deno", "bar");
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=bar"],
    ["x-deno", "foo, bar"],
    ["set-cookie", "bar=baz"],
  ]);
});

unitTest(function headersAppendDuplicateSetCookieKey(): void {
  const headers = new Headers([["Set-Cookie", "foo=bar"]]);
  headers.append("set-Cookie", "foo=baz");
  headers.append("Set-cookie", "baz=bar");
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=baz"],
    ["set-cookie", "baz=bar"],
  ]);
});

unitTest(function headersSetDuplicateCookieKey(): void {
  const headers = new Headers([["Set-Cookie", "foo=bar"]]);
  headers.set("set-Cookie", "foo=baz");
  headers.set("set-cookie", "bar=qat");
  const actual = [...headers];
  assertEquals(actual, [
    ["set-cookie", "foo=baz"],
    ["set-cookie", "bar=qat"],
  ]);
});

unitTest(function headersGetSetCookie(): void {
  const headers = new Headers([
    ["Set-Cookie", "foo=bar"],
    ["set-Cookie", "bar=qat"],
  ]);
  assertEquals(headers.get("SET-COOKIE"), "foo=bar, bar=qat");
});

unitTest(function toStringShouldBeWebCompatibility(): void {
  const headers = new Headers();
  assertEquals(headers.toString(), "[object Headers]");
});

function stringify(...args: unknown[]): string {
  return inspectArgs(args).replace(/\n$/, "");
}

unitTest(function customInspectReturnsCorrectHeadersFormat(): void {
  const blankHeaders = new Headers();
  assertEquals(stringify(blankHeaders), "Headers {}");
  const singleHeader = new Headers([["Content-Type", "application/json"]]);
  assertEquals(
    stringify(singleHeader),
    "Headers { content-type: application/json }",
  );
  const multiParamHeader = new Headers([
    ["Content-Type", "application/json"],
    ["Content-Length", "1337"],
  ]);
  assertEquals(
    stringify(multiParamHeader),
    "Headers { content-type: application/json, content-length: 1337 }",
  );
});
