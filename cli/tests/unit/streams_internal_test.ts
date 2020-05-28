// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertThrows } from "./test_util.ts";

unitTest(function streamReadableHwmError() {
  const invalidHwm = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new ReadableStream<number>(
          undefined,
          // @ts-ignore
          { highWaterMark }
        );
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:"
    );
  }

  assertThrows(() => {
    new ReadableStream<number>(
      undefined,
      // @ts-ignore
      { highWaterMark: Symbol("hwk") }
    );
  }, TypeError);
});

unitTest(function streamWriteableHwmError() {
  const invalidHwm = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new WritableStream(
          undefined,
          // @ts-ignore
          new CountQueuingStrategy({ highWaterMark })
        );
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:"
    );
  }

  assertThrows(() => {
    new WritableStream(
      undefined,
      // @ts-ignore
      new CountQueuingStrategy({ highWaterMark: Symbol("hwmk") })
    );
  }, TypeError);
});

unitTest(function streamTransformHwmError() {
  const invalidHwm = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new TransformStream(
          undefined,
          undefined,
          // @ts-ignore
          { highWaterMark }
        );
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:"
    );
  }

  assertThrows(() => {
    new TransformStream(
      undefined,
      undefined,
      // @ts-ignore
      { highWaterMark: Symbol("hwmk") }
    );
  }, TypeError);
});
