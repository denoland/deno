// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertThrows } from "./test_util.ts";

unitTest(function streamReadableHwmError() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const invalidHwm: any[] = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new ReadableStream<number>(undefined, { highWaterMark });
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:",
    );
  }

  assertThrows(() => {
    new ReadableStream<number>(
      undefined,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      { highWaterMark: Symbol("hwk") as any },
    );
  }, TypeError);
});

unitTest(function streamWriteableHwmError() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const invalidHwm: any[] = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new WritableStream(
          undefined,
          new CountQueuingStrategy({ highWaterMark }),
        );
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:",
    );
  }

  assertThrows(() => {
    new WritableStream(
      undefined,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      new CountQueuingStrategy({ highWaterMark: Symbol("hwmk") as any }),
    );
  }, TypeError);
});

unitTest(function streamTransformHwmError() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const invalidHwm: any[] = [NaN, Number("NaN"), {}, -1, "two"];
  for (const highWaterMark of invalidHwm) {
    assertThrows(
      () => {
        new TransformStream(undefined, undefined, { highWaterMark });
      },
      RangeError,
      "highWaterMark must be a positive number or Infinity.  Received:",
    );
  }

  assertThrows(() => {
    new TransformStream(
      undefined,
      undefined,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      { highWaterMark: Symbol("hwmk") as any },
    );
  }, TypeError);
});
