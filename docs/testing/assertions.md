## Assertions

To help developers write tests the Deno standard library comes with a built in
[assertions module](https://deno.land/std@$STD_VERSION/testing/asserts.ts) which
can be imported from `https://deno.land/std@$STD_VERSION/testing/asserts.ts`.

```js
import { assert } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";

Deno.test("Hello Test", () => {
  assert("Hello");
});
```

The assertions module provides 10 assertions:

- `assert(expr: unknown, msg = ""): asserts expr`
- `assertEquals(actual: unknown, expected: unknown, msg?: string): void`
- `assertExists(actual: unknown,msg?: string): void`
- `assertNotEquals(actual: unknown, expected: unknown, msg?: string): void`
- `assertStrictEquals(actual: unknown, expected: unknown, msg?: string): void`
- `assertStringIncludes(actual: string, expected: string, msg?: string): void`
- `assertArrayIncludes(actual: unknown[], expected: unknown[], msg?: string): void`
- `assertMatch(actual: string, expected: RegExp, msg?: string): void`
- `assertNotMatch(actual: string, expected: RegExp, msg?: string): void`
- `assertObjectMatch( actual: Record<PropertyKey, unknown>, expected: Record<PropertyKey, unknown>): void`
- `assertThrows(fn: () => void, ErrorClass?: Constructor, msgIncludes = "", msg?: string): Error`
- `assertThrowsAsync(fn: () => Promise<void>, ErrorClass?: Constructor, msgIncludes = "", msg?: string): Promise<Error>`

### Assert

The assert method is a simple 'truthy' assertion and can be used to assert any
value which can be inferred as true.

```js
Deno.test("Test Assert", () => {
  assert(1);
  assert("Hello");
  assert(true);
});
```

### Exists

The `assertExists` can be used to check if a value is not `null` or `undefined`.

```js
assertExists("Denosaurus");
Deno.test("Test Assert Exists", () => {
  assertExists("Denosaurus");
  assertExists(false);
  assertExists(0);
});
```

### Equality

There are three equality assertions available, `assertEquals()`,
`assertNotEquals()` and `assertStrictEquals()`.

The `assertEquals()` and `assertNotEquals()` methods provide a general equality
check and are capable of asserting equality between primitive types and objects.

```js
Deno.test("Test Assert Equals", () => {
  assertEquals(1, 1);
  assertEquals("Hello", "Hello");
  assertEquals(true, true);
  assertEquals(undefined, undefined);
  assertEquals(null, null);
  assertEquals(new Date(), new Date());
  assertEquals(new RegExp("abc"), new RegExp("abc"));

  class Foo {}
  const foo1 = new Foo();
  const foo2 = new Foo();

  assertEquals(foo1, foo2);
});

Deno.test("Test Assert Not Equals", () => {
  assertNotEquals(1, 2);
  assertNotEquals("Hello", "World");
  assertNotEquals(true, false);
  assertNotEquals(undefined, "");
  assertNotEquals(new Date(), Date.now());
  assertNotEquals(new RegExp("abc"), new RegExp("def"));
});
```

By contrast `assertStrictEquals()` provides a simpler, stricter equality check
based on the `===` operator. As a result it will not assert two instances of
identical objects as they won't be referentially the same.

```js
Deno.test("Test Assert Strict Equals", () => {
  assertStrictEquals(1, 1);
  assertStrictEquals("Hello", "Hello");
  assertStrictEquals(true, true);
  assertStrictEquals(undefined, undefined);
});
```

The `assertStrictEquals()` assertion is best used when you wish to make a
precise check against two primitive types.

### Contains

There are two methods available to assert a value contains a value,
`assertStringIncludes()` and `assertArrayIncludes()`.

The `assertStringIncludes()` assertion does a simple includes check on a string
to see if it contains the expected string.

```js
Deno.test("Test Assert String Contains", () => {
  assertStringIncludes("Hello World", "Hello");
});
```

The `assertArrayIncludes()` assertion is slightly more advanced and can find
both a value within an array and an array of values within an array.

```js
Deno.test("Test Assert Array Contains", () => {
  assertArrayIncludes([1, 2, 3], [1]);
  assertArrayIncludes([1, 2, 3], [1, 2]);
  assertArrayIncludes(Array.from("Hello World"), Array.from("Hello"));
});
```

### Regex

You can assert regular expressions via `assertMatch()` and `assertNotMatch()`
assertions.

```js
Deno.test("Test Assert Match", () => {
  assertMatch("abcdefghi", new RegExp("def"));

  const basicUrl = new RegExp("^https?://[a-z.]+.com$");
  assertMatch("https://www.google.com", basicUrl);
  assertMatch("http://facebook.com", basicUrl);
});

Deno.test("Test Assert Not Match", () => {
  assertNotMatch("abcdefghi", new RegExp("jkl"));

  const basicUrl = new RegExp("^https?://[a-z.]+.com$");
  assertNotMatch("https://deno.land/", basicUrl);
});
```

### Object

Use `assertObjectMatch` to check that a JavaScript object matches a subset of
the properties of an object.

```js
// Simple subset
assertObjectMatch(
  { foo: true, bar: false },
  {
    foo: true,
  },
);
```

### Throws

There are two ways to assert whether something throws an error in Deno,
`assertThrows()` and `assertThrowsAsync()`. Both assertions allow you to check
an
[Error](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error)
has been thrown, the type of error thrown and what the message was.

The difference between the two assertions is `assertThrows()` accepts a standard
function and `assertThrowsAsync()` accepts a function which returns a
[Promise](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise).

The `assertThrows()` assertion will check an error has been thrown, and
optionally will check the thrown error is of the correct type, and assert the
error message is as expected.

```js
Deno.test("Test Assert Throws", () => {
  assertThrows(
    () => {
      throw new Error("Panic!");
    },
    Error,
    "Panic!",
  );
});
```

The `assertThrowsAsync()` assertion is a little more complicated, mainly because
it deals with Promises. But basically it will catch thrown errors or rejections
in Promises. You can also optionally check for the error type and error message.

```js
Deno.test("Test Assert Throws Async", () => {
  assertThrowsAsync(
    () => {
      return new Promise(() => {
        throw new Error("Panic! Threw Error");
      });
    },
    Error,
    "Panic! Threw Error",
  );

  assertThrowsAsync(
    () => {
      return Promise.reject(new Error("Panic! Reject Error"));
    },
    Error,
    "Panic! Reject Error",
  );
});
```

### Custom Messages

Each of Deno's built in assertions allow you to overwrite the standard CLI error
message if you wish. For instance this example will output "Values Don't Match!"
rather than the standard CLI error message.

```js
Deno.test("Test Assert Equal Fail Custom Message", () => {
  assertEquals(1, 2, "Values Don't Match!");
});
```
