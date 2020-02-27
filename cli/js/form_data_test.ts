// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function formDataHasCorrectNameProp(): void {
  assert.equals(FormData.name, "FormData");
});

test(function formDataParamsAppendSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  assert.equals(formData.get("a"), "true");
});

test(function formDataParamsDeleteSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assert.equals(formData.get("b"), "false");
  formData.delete("b");
  assert.equals(formData.get("a"), "true");
  assert.equals(formData.get("b"), null);
});

test(function formDataParamsGetAllSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assert.equals(formData.getAll("a"), ["true", "null"]);
  assert.equals(formData.getAll("b"), ["false"]);
  assert.equals(formData.getAll("c"), []);
});

test(function formDataParamsGetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  // @ts-ignore
  formData.append("d", undefined);
  // @ts-ignore
  formData.append("e", null);
  assert.equals(formData.get("a"), "true");
  assert.equals(formData.get("b"), "false");
  assert.equals(formData.get("c"), null);
  assert.equals(formData.get("d"), "undefined");
  assert.equals(formData.get("e"), "null");
});

test(function formDataParamsHasSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assert(formData.has("a"));
  assert(formData.has("b"));
  assert(!formData.has("c"));
});

test(function formDataParamsSetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assert.equals(formData.getAll("a"), ["true", "null"]);
  assert.equals(formData.getAll("b"), ["false"]);
  formData.set("a", "false");
  assert.equals(formData.getAll("a"), ["false"]);
  // @ts-ignore
  formData.set("d", undefined);
  assert.equals(formData.get("d"), "undefined");
  // @ts-ignore
  formData.set("e", null);
  assert.equals(formData.get("e"), "null");
});

test(function formDataSetEmptyBlobSuccess(): void {
  const formData = new FormData();
  formData.set("a", new Blob([]), "blank.txt");
  formData.get("a");
  /* TODO Fix this test.
  assert(file instanceof File);
  if (typeof file !== "string") {
    assert.equals(file.name, "blank.txt");
  }
  */
});

test(function formDataParamsForEachSuccess(): void {
  const init = [
    ["a", "54"],
    ["b", "true"]
  ];
  const formData = new FormData();
  for (const [name, value] of init) {
    formData.append(name, value);
  }
  let callNum = 0;
  formData.forEach((value, key, parent): void => {
    assert.equals(formData, parent);
    assert.equals(value, init[callNum][1]);
    assert.equals(key, init[callNum][0]);
    callNum++;
  });
  assert.equals(callNum, init.length);
});

test(function formDataParamsArgumentsCheck(): void {
  const methodRequireOneParam = [
    "delete",
    "getAll",
    "get",
    "has",
    "forEach"
  ] as const;

  const methodRequireTwoParams = ["append", "set"] as const;

  methodRequireOneParam.forEach((method): void => {
    const formData = new FormData();
    let hasThrown = 0;
    let errMsg = "";
    try {
      // @ts-ignore
      formData[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assert.equals(hasThrown, 2);
    assert.equals(
      errMsg,
      `FormData.${method} requires at least 1 argument, but only 0 present`
    );
  });

  methodRequireTwoParams.forEach((method: string): void => {
    const formData = new FormData();
    let hasThrown = 0;
    let errMsg = "";

    try {
      // @ts-ignore
      formData[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assert.equals(hasThrown, 2);
    assert.equals(
      errMsg,
      `FormData.${method} requires at least 2 arguments, but only 0 present`
    );

    hasThrown = 0;
    errMsg = "";
    try {
      // @ts-ignore
      formData[method]("foo");
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assert.equals(hasThrown, 2);
    assert.equals(
      errMsg,
      `FormData.${method} requires at least 2 arguments, but only 1 present`
    );
  });
});

test(function toStringShouldBeWebCompatibility(): void {
  const formData = new FormData();
  assert.equals(formData.toString(), "[object FormData]");
});
