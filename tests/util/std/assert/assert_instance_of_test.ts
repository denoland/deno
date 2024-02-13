// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertInstanceOf, AssertionError, assertThrows } from "./mod.ts";

Deno.test({
  name: "assertInstanceOf",
  fn() {
    class TestClass1 {}
    class TestClass2 {}
    class TestClass3 {}

    // Regular types
    assertInstanceOf(new Date(), Date);
    assertInstanceOf(new Number(), Number);
    assertInstanceOf(Promise.resolve(), Promise);
    assertInstanceOf(new TestClass1(), TestClass1);

    // Throwing cases
    assertThrows(
      () => assertInstanceOf(new Date(), RegExp),
      AssertionError,
      `Expected object to be an instance of "RegExp" but was "Date".`,
    );
    assertThrows(
      () => assertInstanceOf(5, Date),
      AssertionError,
      `Expected object to be an instance of "Date" but was "number".`,
    );
    assertThrows(
      () => assertInstanceOf(new TestClass1(), TestClass2),
      AssertionError,
      `Expected object to be an instance of "TestClass2" but was "TestClass1".`,
    );

    // Custom message
    assertThrows(
      () => assertInstanceOf(new Date(), RegExp, "Custom message"),
      AssertionError,
      "Custom message",
    );

    // Edge cases
    assertThrows(
      () => assertInstanceOf(5, Number),
      AssertionError,
      `Expected object to be an instance of "Number" but was "number".`,
    );

    let TestClassWithSameName: new () => unknown;
    {
      class TestClass3 {}
      TestClassWithSameName = TestClass3;
    }
    assertThrows(
      () => assertInstanceOf(new TestClassWithSameName(), TestClass3),
      AssertionError,
      `Expected object to be an instance of "TestClass3".`,
    );

    assertThrows(
      () => assertInstanceOf(TestClass1, TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was not an instanced object.`,
    );
    assertThrows(
      () => assertInstanceOf(() => {}, TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was not an instanced object.`,
    );
    assertThrows(
      () => assertInstanceOf(null, TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was "null".`,
    );
    assertThrows(
      () => assertInstanceOf(undefined, TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was "undefined".`,
    );
    assertThrows(
      () => assertInstanceOf({}, TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was "Object".`,
    );
    assertThrows(
      () => assertInstanceOf(Object.create(null), TestClass1),
      AssertionError,
      `Expected object to be an instance of "TestClass1" but was "Object".`,
    );

    // Test TypeScript types functionality, wrapped in a function that never runs
    // deno-lint-ignore no-unused-vars
    function typeScriptTests() {
      class ClassWithProperty {
        property = "prop1";
      }
      const testInstance = new ClassWithProperty() as unknown;

      // @ts-expect-error: `testInstance` is `unknown` so setting its property before `assertInstanceOf` should give a type error.
      testInstance.property = "prop2";

      assertInstanceOf(testInstance, ClassWithProperty);

      // Now `testInstance` should be of type `ClassWithProperty`
      testInstance.property = "prop3";

      let x = 5 as unknown;

      // @ts-expect-error: `x` is `unknown` so adding to it shouldn't work
      x += 5;
      assertInstanceOf(x, Number);

      // @ts-expect-error: `x` is now `Number` rather than `number`, so this should still give a type error.
      x += 5;
    }
  },
});
