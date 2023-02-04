// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

// deno-lint-ignore no-explicit-any
function testFirstArgument(arg1: any[], expectedSize: number) {
  const file = new File(arg1, "name");
  assert(file instanceof File);
  assertEquals(file.name, "name");
  assertEquals(file.size, expectedSize);
  assertEquals(file.type, "");
}

Deno.test(function fileEmptyFileBits() {
  testFirstArgument([], 0);
});

Deno.test(function fileStringFileBits() {
  testFirstArgument(["bits"], 4);
});

Deno.test(function fileUnicodeStringFileBits() {
  testFirstArgument(["𝓽𝓮𝔁𝓽"], 16);
});

Deno.test(function fileStringObjectFileBits() {
  testFirstArgument([new String("string object")], 13);
});

Deno.test(function fileEmptyBlobFileBits() {
  testFirstArgument([new Blob()], 0);
});

Deno.test(function fileBlobFileBits() {
  testFirstArgument([new Blob(["bits"])], 4);
});

Deno.test(function fileEmptyFileFileBits() {
  testFirstArgument([new File([], "world.txt")], 0);
});

Deno.test(function fileFileFileBits() {
  testFirstArgument([new File(["bits"], "world.txt")], 4);
});

Deno.test(function fileArrayBufferFileBits() {
  testFirstArgument([new ArrayBuffer(8)], 8);
});

Deno.test(function fileTypedArrayFileBits() {
  testFirstArgument([new Uint8Array([0x50, 0x41, 0x53, 0x53])], 4);
});

Deno.test(function fileVariousFileBits() {
  testFirstArgument(
    [
      "bits",
      new Blob(["bits"]),
      new Blob(),
      new Uint8Array([0x50, 0x41]),
      new Uint16Array([0x5353]),
      new Uint32Array([0x53534150]),
    ],
    16,
  );
});

Deno.test(function fileNumberInFileBits() {
  testFirstArgument([12], 2);
});

Deno.test(function fileArrayInFileBits() {
  testFirstArgument([[1, 2, 3]], 5);
});

Deno.test(function fileObjectInFileBits() {
  // "[object Object]"
  testFirstArgument([{}], 15);
});

// deno-lint-ignore no-explicit-any
function testSecondArgument(arg2: any, expectedFileName: string) {
  const file = new File(["bits"], arg2);
  assert(file instanceof File);
  assertEquals(file.name, expectedFileName);
}

Deno.test(function fileUsingFileName() {
  testSecondArgument("dummy", "dummy");
});

Deno.test(function fileUsingNullFileName() {
  testSecondArgument(null, "null");
});

Deno.test(function fileUsingNumberFileName() {
  testSecondArgument(1, "1");
});

Deno.test(function fileUsingEmptyStringFileName() {
  testSecondArgument("", "");
});

Deno.test(function fileConstructorOptionsValidation() {
  // deno-lint-ignore ban-types
  type AnyFunction = Function;
  const assert = {
    strictEqual: assertEquals,
    throws(fn: AnyFunction, expected: AnyFunction) {
      try {
        fn();
        throw new Error("Missing expected exception");
      } catch (e) {
        if (e instanceof expected) {
          return;
        }

        throw new Error("Wrong type of error", { cause: e });
      }
    },
  };

  function mustCall(fn: AnyFunction, nb = 1) {
    const timeout = setTimeout(() => {
      if (nb !== 0) throw new Error(`Expected ${nb} more calls`);
    }, 999);
    return function (this: unknown) {
      nb--;
      if (nb === 0) clearTimeout(timeout);
      else if (nb < 0) {
        throw new Error("Function has been called more times than expected");
      }
      return Reflect.apply(fn, this, arguments);
    };
  }

  [undefined, null, Object.create(null), { lastModified: undefined }, {
    get lastModified() {
      return undefined;
    },
  }].forEach((options) => {
    assert.strictEqual(
      new File([], "", options).lastModified,
      new File([], "").lastModified,
    );
  });

  Reflect.defineProperty(Object.prototype, "get", {
    // @ts-ignore __proto__ null is important here to avoid prototype pollution.
    __proto__: null,
    configurable: true,
    get() {
      throw new Error();
    },
  });
  Reflect.defineProperty(Object.prototype, "lastModified", {
    // @ts-ignore __proto__ null is important here to avoid prototype pollution.
    __proto__: null,
    configurable: true,
    get: mustCall(() => 3, 7),
  });

  [{}, [], () => {}, Number, new Number(), new String(), new Boolean()].forEach(
    (options) => {
      // @ts-ignore We want to test an options object that doesn't meet the typical types.
      assert.strictEqual(new File([], "", options).lastModified, 3);
    },
  );
  [0, "", true, Symbol(), 0n].forEach((options) => {
    // @ts-ignore We want to test an options object that doesn't meet the typical types.
    assert.throws(() => new File([], "", options), TypeError);
  });

  // @ts-ignore cleaning up.
  delete Object.prototype.lastModified;
});
