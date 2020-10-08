import { assertEquals, assertThrows } from "../testing/asserts.ts";
import { validate, isExpired } from "./validation.ts";
import { setExpiration } from "./mod.ts";

const header = {
  alg: "HS256",
  typ: "JWT",
};
const payload = {
  name: "John Doe",
};
const signature = "abc";
const exp = setExpiration(new Date("2035-07-01"));

Deno.test("[jwt] validate", async function (): Promise<void> {
  assertEquals(validate({ header, payload, signature }, "HS256"), {
    header,
    payload,
    signature,
  });
  assertEquals(
    validate({ header, payload: "payloadAsString", signature }, "HS256"),
    {
      header,
      payload: "payloadAsString",
      signature,
    }
  );
  assertEquals(
    validate(
      {
        header,
        payload: { exp },
        signature,
      },
      "HS256"
    ),
    { header, payload: { exp }, signature }
  );

  assertThrows(() => {
    validate({ header: { alg: "invalidAlg" }, payload, signature }, "HS256");
  }, Error);
  assertThrows(() => {
    validate({ header: {}, payload, signature }, "HS256");
  }, Error);
  assertThrows(() => {
    validate({ header, payload, signature: 111 }, "HS256");
  }, Error);
  assertThrows(() => {
    validate({ header, payload: { exp: 100 }, signature }, "HS256");
  }, Error);
});

Deno.test("[jwt] isExpired", function (): void {
  // A specific date:
  const t1 = setExpiration(new Date("2020-01-01"));
  const t2 = setExpiration(new Date("2099-01-01"));
  // Ten seconds from now:
  const t3 = setExpiration(10);
  // One hour from now:
  const t4 = setExpiration(60 * 60);
  //  1 second from now:
  const t5 = setExpiration(1);
  //  1 second earlier:
  const t6 = setExpiration(-1);
  assertEquals(isExpired(t1), true);
  assertEquals(isExpired(t2), false);
  assertEquals(10, t3 - Math.round(Date.now() / 1000));
  assertEquals(isExpired(t4), false);
  assertEquals(isExpired(t5), false);
  assertEquals(isExpired(t6), true);
  // add leeway:
  assertEquals(isExpired(t6, 1500), false);
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});
