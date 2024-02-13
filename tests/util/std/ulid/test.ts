// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2023 Yoshiya Hinosawa. All rights reserved. MIT license.
// Copyright 2017 Alizain Feerasta. All rights reserved. MIT license.

import { FakeTime } from "../testing/time.ts";
import {
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "../assert/mod.ts";

import { decodeTime, monotonicUlid, ulid } from "./mod.ts";
import {
  encodeRandom,
  encodeTime,
  ENCODING,
  ENCODING_LEN,
  incrementBase32,
  monotonicFactory,
} from "./_util.ts";

Deno.test("increment base32", async (t) => {
  await t.step("increments correctly", () => {
    assertEquals("A109D", incrementBase32("A109C"));
  });

  await t.step("carries correctly", () => {
    assertEquals("A1Z00", incrementBase32("A1YZZ"));
  });

  await t.step("double increments correctly", () => {
    assertEquals(
      "A1Z01",
      incrementBase32(incrementBase32("A1YZZ")),
    );
  });

  await t.step("throws when it cannot increment", () => {
    assertThrows(() => {
      incrementBase32("ZZZ");
    });
  });

  await t.step("throws when given an invalid string", () => {
    assertThrows(() => {
      incrementBase32("./_--=-`,.");
    });
  });
});

Deno.test("encodeTime", async (t) => {
  await t.step("should return expected encoded result", () => {
    assertEquals("01ARYZ6S41", encodeTime(1469918176385, 10));
  });

  await t.step("should change length properly", () => {
    assertEquals("0001AS99AA60", encodeTime(1470264322240, 12));
  });

  await t.step("should truncate time if not enough length", () => {
    assertEquals("AS4Y1E11", encodeTime(1470118279201, 8));
  });

  await t.step("should throw an error", async (t) => {
    await t.step("if time greater than (2 ^ 48) - 1", () => {
      assertThrows(() => {
        encodeTime(Math.pow(2, 48), 8);
      }, Error);
    });

    await t.step("if time is not a number", () => {
      assertThrows(() => {
        // deno-lint-ignore no-explicit-any
        encodeTime("test" as any, 3);
      }, Error);
    });

    await t.step("if time is infinity", () => {
      assertThrows(() => {
        encodeTime(Infinity);
      }, Error);
    });

    await t.step("if time is negative", () => {
      assertThrows(() => {
        encodeTime(-1);
      }, Error);
    });

    await t.step("if time is a float", () => {
      assertThrows(() => {
        encodeTime(100.1);
      }, Error);
    });
  });
});

Deno.test("encodeRandom", async (t) => {
  await t.step("should return correct length", () => {
    assertEquals(12, encodeRandom(12).length);
  });
});

Deno.test("decodeTime", async (t) => {
  await t.step("should return correct timestamp", () => {
    const timestamp = Date.now();
    const id = ulid(timestamp);
    assertEquals(timestamp, decodeTime(id));
  });

  await t.step("should accept the maximum allowed timestamp", () => {
    assertEquals(
      281474976710655,
      decodeTime("7ZZZZZZZZZZZZZZZZZZZZZZZZZ"),
    );
  });

  await t.step("should reject", async (t) => {
    await t.step("malformed strings of incorrect length", () => {
      assertThrows(() => {
        decodeTime("FFFF");
      }, Error);
    });

    await t.step("strings with timestamps that are too high", () => {
      assertThrows(() => {
        decodeTime("80000000000000000000000000");
      }, Error);
    });

    await t.step("invalid character", () => {
      assertThrows(() => {
        decodeTime("&1ARZ3NDEKTSV4RRFFQ69G5FAV");
      }, Error);
    });
  });
});

Deno.test("ulid", async (t) => {
  await t.step("should return correct length", () => {
    assertEquals(26, ulid().length);
  });

  await t.step(
    "should return expected encoded time component result",
    () => {
      assertEquals("01ARYZ6S41", ulid(1469918176385).substring(0, 10));
    },
  );
});

Deno.test("monotonicity", async (t) => {
  function encodeRandom(len: number): string {
    let str = "";
    const randomBytes = new Array(len).fill(30);
    for (let i = 0; i < len; i++) {
      str += ENCODING[randomBytes[i] % ENCODING_LEN];
    }
    return str;
  }

  await t.step("without seedTime", async (t) => {
    const stubbedUlid = monotonicFactory(encodeRandom);

    const time = new FakeTime(1469918176385);

    await t.step("first call", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYYY", stubbedUlid());
    });

    await t.step("second call", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYYZ", stubbedUlid());
    });

    await t.step("third call", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYZ0", stubbedUlid());
    });

    await t.step("fourth call", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYZ1", stubbedUlid());
    });

    time.restore();
  });

  await t.step("with seedTime", async (t) => {
    const stubbedUlid = monotonicFactory(encodeRandom);

    await t.step("first call", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYYY", stubbedUlid(1469918176385));
    });

    await t.step("second call with the same", () => {
      assertStrictEquals(
        "01ARYZ6S41YYYYYYYYYYYYYYYZ",
        stubbedUlid(1469918176385),
      );
    });

    await t.step("third call with less than", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYZ0", stubbedUlid(100000000));
    });

    await t.step("fourth call with even more less than", () => {
      assertEquals("01ARYZ6S41YYYYYYYYYYYYYYZ1", stubbedUlid(10000));
    });

    await t.step("fifth call with 1 greater than", () => {
      assertEquals("01ARYZ6S42YYYYYYYYYYYYYYYY", stubbedUlid(1469918176386));
    });
  });

  await t.step("seed time is decoded correctly", () => {
    assertEquals(
      decodeTime(monotonicUlid(1000)),
      1000,
    );
    assertEquals(
      decodeTime(monotonicUlid(1469918176386)),
      1469918176386,
    );
  });
});
