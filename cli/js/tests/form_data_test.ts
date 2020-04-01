// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(function formDataHasCorrectNameProp(): void {
  assertEquals(FormData.name, "FormData");
});

unitTest(function formDataParamsAppendSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  assertEquals(formData.get("a"), "true");
});

unitTest(function formDataParamsDeleteSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assertEquals(formData.get("b"), "false");
  formData.delete("b");
  assertEquals(formData.get("a"), "true");
  assertEquals(formData.get("b"), null);
});

unitTest(function formDataParamsGetAllSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEquals(formData.getAll("a"), ["true", "null"]);
  assertEquals(formData.getAll("b"), ["false"]);
  assertEquals(formData.getAll("c"), []);
});

unitTest(function formDataParamsGetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  // @ts-ignore
  formData.append("d", undefined);
  // @ts-ignore
  formData.append("e", null);
  assertEquals(formData.get("a"), "true");
  assertEquals(formData.get("b"), "false");
  assertEquals(formData.get("c"), null);
  assertEquals(formData.get("d"), "undefined");
  assertEquals(formData.get("e"), "null");
});

unitTest(function formDataParamsHasSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assert(formData.has("a"));
  assert(formData.has("b"));
  assert(!formData.has("c"));
});

unitTest(function formDataParamsSetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEquals(formData.getAll("a"), ["true", "null"]);
  assertEquals(formData.getAll("b"), ["false"]);
  formData.set("a", "false");
  assertEquals(formData.getAll("a"), ["false"]);
  // @ts-ignore
  formData.set("d", undefined);
  assertEquals(formData.get("d"), "undefined");
  // @ts-ignore
  formData.set("e", null);
  assertEquals(formData.get("e"), "null");
});

unitTest(function formDataSetEmptyBlobSuccess(): void {
  const formData = new FormData();
  formData.set("a", new Blob([]), "blank.txt");
  formData.get("a");
  /* TODO Fix this test.
  assert(file instanceof File);
  if (typeof file !== "string") {
    assertEquals(file.name, "blank.txt");
  }
  */
});

unitTest(function formDataParamsForEachSuccess(): void {
  const init = [
    ["a", "54"],
    ["b", "true"],
  ];
  const formData = new FormData();
  for (const [name, value] of init) {
    formData.append(name, value);
  }
  let callNum = 0;
  formData.forEach((value, key, parent): void => {
    assertEquals(formData, parent);
    assertEquals(value, init[callNum][1]);
    assertEquals(key, init[callNum][0]);
    callNum++;
  });
  assertEquals(callNum, init.length);
});

unitTest(function formDataParamsArgumentsCheck(): void {
  const methodRequireOneParam = [
    "delete",
    "getAll",
    "get",
    "has",
    "forEach",
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
    assertEquals(hasThrown, 2);
    assertEquals(
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
    assertEquals(hasThrown, 2);
    assertEquals(
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
    assertEquals(hasThrown, 2);
    assertEquals(
      errMsg,
      `FormData.${method} requires at least 2 arguments, but only 1 present`
    );
  });
});

unitTest(function toStringShouldBeWebCompatibility(): void {
  const formData = new FormData();
  assertEquals(formData.toString(), "[object FormData]");
});
