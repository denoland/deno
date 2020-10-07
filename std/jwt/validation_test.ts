import { assertEquals, assertThrows } from "../testing/asserts.ts";
import { validate } from "./validation.ts";
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

Deno.test("[jwt] validate TokenObject", async function (): Promise<void> {
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
