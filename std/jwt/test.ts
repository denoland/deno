import { create, decode, Header, Payload, verify } from "./mod.ts";

import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";

const header: Header = {
  alg: "HS256",
  typ: "JWT",
};

const payload: Payload = {
  name: "John Doe",
};

const key = "secret";

Deno.test({
  name: "[jwt] create",
  fn: async function () {
    assertEquals(
      await create("", key),
      "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9..B0lmJDC8zSfMJstPqLdOAWfM265-5Svj0XrACZm8DKa1y6VJA0W7d0VoGGKJo0quKxWUdf1B1ueElNk2Yl_cLw",
    );
    assertEquals(
      await create({}, key),
      "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.e30.dGumW8J3t2BlAwqqoisyWDC6ov2hRtjTAFHzd-Tlr4DUScaHG4OYqTHXLHEzd3hU5wy5xs87vRov6QzZnj410g",
    );
    assertEquals(
      await create({ foo: "bar" }, key),
      "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJmb28iOiJiYXIifQ.WePl7achkd0oGNB8XRF_LJwxlyiPZqpdNgdKpDboAjSTsWq-aOGNynTp8TOv8KjonFym8vwFwppXOLoLXbkIaQ",
    );
    assertEquals(
      await create("null", key),
      "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.bnVsbA.tv7DbhvALc5Eq2sC61Y9IZlG2G15hvJoug9UO6iwmE_UZOLva8EC-9PURg7IIj6f-F9jFWix8vCn9WaAMHR1AA",
    );
    assertEquals(
      await create("[]", key),
      "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.W10.BqmZ-tVI9a-HDx6PpMiBdMq6lzcaqO9sW6pImw-NRajCCmRrVi6IgMhEw7lvOG6sxhteceVMl8_xFRGverJJWw",
    );
  },
});

Deno.test({
  name: "[jwt] verify",
  fn: async function () {
    assertEquals(
      await verify(await create("", key, { header: header }), key, {
        algorithm: "HS256",
      }),
      "",
    );
    assertEquals(
      await verify(
        await create("abc", key, { header: header }),
        key,
        {
          algorithm: "HS256",
        },
      ),
      "abc",
    );

    await assertEquals(
      await verify(await create("null", key), key),
      null,
    );

    await assertEquals(
      await verify(await create("true", key), key),
      true,
    );

    assertEquals(
      await verify(
        await create(payload, key, { header: header }),
        key,
        {
          algorithm: "HS256",
        },
      ),
      payload,
    );
    await assertEquals(
      await verify(await create({}, key), key),
      {},
    );
    await assertEquals(
      await verify(await create("[]", key), key),
      [],
    );
    await assertEquals(
      await verify(await create(`["a", 1, true]`, key), key),
      ["a", 1, true],
    );

    await assertThrowsAsync(
      async () => {
        // payload = { "exp": false }
        await verify(
          "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJleHAiOmZhbHNlfQ.LXb8M9J6ar14CTq7shnqDMWmSsoH_zyIHiD44Rqd6uI",
          key,
        );
      },
      Error,
      "The token is invalid.",
    );

    await assertThrowsAsync(
      async () => {
        await verify("", key);
      },
      Error,
      "The serialization is invalid.",
    );

    await assertThrowsAsync(
      async () => {
        await verify("invalid", key);
      },
      Error,
      "The serialization is invalid.",
    );

    await assertThrowsAsync(
      async () => {
        await verify(
          await create({
            // @ts-ignore */
            exp: "invalid",
          }, key),
          key,
        );
      },
      Error,
      "The token is invalid.",
    );
  },
});

Deno.test({
  name: "[jwt] decode",
  fn: async function () {
    assertEquals(
      decode(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.TVCeFl1nnZWUMQkAQKuSo_I97YeIZAS8T1gOkErT7F8",
      ),
      {
        header: { alg: "HS256", typ: "JWT" },
        payload: {},
        signature:
          "4d509e165d679d959431090040ab92a3f23ded87886404bc4f580e904ad3ec5f",
      },
    );
    assertThrows(
      () => {
        decode("aaa");
      },
      TypeError,
      "The serialization is invalid.",
    );

    assertThrows(
      () => {
        decode("a");
      },
      TypeError,
      "Illegal base64url string!",
    );

    assertThrows(
      () => {
        // "ImEi" === base64url("a")
        decode("ImEi.ImEi.ImEi.ImEi");
      },
      TypeError,
      "The serialization is invalid.",
    );

    const jwt =
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    const header: Header = {
      alg: "HS256",
      typ: "JWT",
    };
    const payload = {
      sub: "1234567890",
      name: "John Doe",
      iat: 1516239022,
    };
    assertEquals(decode(jwt), {
      header,
      payload,
      signature:
        "49f94ac7044948c78a285d904f87f0a4c7897f7e8f3a4eb2255fda750b2cc397",
    });
    assertEquals(await create(payload, "your-256-bit-secret", { header }), jwt);
  },
});

Deno.test({
  name: "[jwt] expired token",
  fn: async function () {
    const payload = {
      iss: "joe",
      jti: "123456789abc",
      exp: 20000,
    };
    const header: Header = {
      alg: "HS256",
      dummy: 100,
    };

    await assertThrowsAsync(
      async () => {
        await verify(await create({ exp: 0 }, key), key);
      },
      Error,
      "The token is expired.",
    );

    await assertThrowsAsync(
      async () => {
        await verify(
          await create(payload, key, { header }),
          key,
          { algorithm: "HS256" },
        );
      },
      Error,
      "The token is expired.",
    );
  },
});

Deno.test({
  name: "[jwt] none algorithm",
  fn: async function () {
    const payload = {
      iss: "joe",
      jti: "123456789abc",
    };
    const header: Header = {
      alg: "none",
      dummy: 100,
    };
    const jwt = await create(payload, key, { header });
    const validatedPayload = await verify(jwt, "keyIsIgnored", {
      algorithm: "none",
    });
    assertEquals(validatedPayload, payload);
  },
});

Deno.test({
  name: "[jwt] HS256 algorithm",
  fn: async function () {
    const header: Header = {
      alg: "HS256",
      typ: "JWT",
    };
    const payload = {
      sub: "1234567890",
      name: "John Doe",
      iat: 1516239022,
    };
    const jwt = await create(payload, key, { header });
    const validatedPayload = await verify(jwt, key, { algorithm: "HS256" });
    assertEquals(
      jwt,
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.XbPfbIHMI6arZ3Y922BhjWgQzWXcXNrz0ogtVhfEd2o",
    );
    assertEquals(validatedPayload, payload);
    assertThrowsAsync(
      async () => {
        const invalidJwt = // jwt with not supported crypto algorithm in alg header:
          "eyJhbGciOiJIUzM4NCIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.bQTnz6AuMJvmXXQsVPrxeQNvzDkimo7VNXxHeSBfClLufmCVZRUuyTwJF311JHuh";
        await verify(invalidJwt, "", {
          algorithm: "HS256",
        });
      },
      Error,
      `The token's algorithm does not match the specified algorithm 'HS256'.`,
    );
  },
});

Deno.test({
  name: "[jwt] HS512 algorithm",
  fn: async function () {
    const header: Header = { alg: "HS512", typ: "JWT" };
    const payload = {
      sub: "1234567890",
      name: "John Doe",
      admin: true,
      iat: 1516239022,
    };
    const jwt = await create(payload, key, { header });
    const validatedPayload = await verify(jwt, key, { algorithm: "HS512" });
    assertEquals(validatedPayload, payload);
  },
});
