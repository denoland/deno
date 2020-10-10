import {
  create,
  Header,
  isTokenObject,
  parse,
  setExpiration,
  verify,
} from "./mod.ts";

import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";

const header: Header = {
  alg: "HS256",
  typ: "JWT",
};
const payload = {
  name: "John Doe",
};
const signature = "abc";
const exp = setExpiration(new Date("2035-07-01"));
const key = "your-secret";

Deno.test("[jwt] isTokenObject", function (): void {
  assert(isTokenObject({ header, payload, signature }));
  assert(
    isTokenObject({ header, payload: "payloadAsString", signature }),
  );
  assert(
    isTokenObject(
      {
        header,
        payload: { exp },
        signature,
      },
    ),
  );

  // @ts-ignore */
  assertEquals(isTokenObject("invalid"), false);
  // @ts-ignore */
  assertEquals(isTokenObject({ header: "invalid" }), false);
  // @ts-ignore */
  assertEquals(isTokenObject({ signature: 123 }), false);
});

Deno.test("[jwt] create", function (): void {
  const key = "secret";
  assertEquals(
    create("", key),
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9..B0lmJDC8zSfMJstPqLdOAWfM265-5Svj0XrACZm8DKa1y6VJA0W7d0VoGGKJo0quKxWUdf1B1ueElNk2Yl_cLw",
  );
  assertEquals(
    create({}, key),
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.e30.dGumW8J3t2BlAwqqoisyWDC6ov2hRtjTAFHzd-Tlr4DUScaHG4OYqTHXLHEzd3hU5wy5xs87vRov6QzZnj410g",
  );
  assertEquals(
    create({ foo: "bar" }, key),
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.eyJmb28iOiJiYXIifQ.WePl7achkd0oGNB8XRF_LJwxlyiPZqpdNgdKpDboAjSTsWq-aOGNynTp8TOv8KjonFym8vwFwppXOLoLXbkIaQ",
  );
  assertEquals(
    create(null, key),
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.bnVsbA.tv7DbhvALc5Eq2sC61Y9IZlG2G15hvJoug9UO6iwmE_UZOLva8EC-9PURg7IIj6f-F9jFWix8vCn9WaAMHR1AA",
  );
  assertEquals(
    create([], key),
    "eyJhbGciOiJIUzUxMiIsInR5cCI6IkpXVCJ9.W10.BqmZ-tVI9a-HDx6PpMiBdMq6lzcaqO9sW6pImw-NRajCCmRrVi6IgMhEw7lvOG6sxhteceVMl8_xFRGverJJWw",
  );
});

Deno.test("[jwt] verify", function (): void {
  assertThrows(
    () => {
      // @ts-ignore */
      const jwt = create("", key, { header: "invalid" });
      verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );

  assertThrows(
    () => {
      // @ts-ignore */
      const jwt = create("", key, { header: { alg: "invalidAlg" } });
      verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );
  assertThrows(() => {
    // @ts-ignore */
    const jwt = create({ header: {}, payload });
    verify(jwt, key, { algorithm: "HS512" });
  }, Error);
  assertThrows(
    () => {
      // @ts-ignore */
      const jwt = create({ header, payload });
      verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
  );
  assertThrows(
    () => {
      // @ts-ignore */
      const jwt = create({ exp: 100 }, key, { header });
      verify(jwt, key, { algorithm: "HS512" });
    },
    Error,
    "jwt is expired",
  );
});

Deno.test("[jwt] setExpiration", function (): void {
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});

Deno.test("[jwt] parse", function (): void {
  assertEquals(
    parse(
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.TVCeFl1nnZWUMQkAQKuSo_I97YeIZAS8T1gOkErT7F8",
    ),
    {
      header: { alg: "HS256", typ: "JWT" },
      payload: {},
      signature:
        "4d509e165d679d959431090040ab92a3f23ded87886404bc4f580e904ad3ec5f",
    },
  );
  // "ImEi" === base64url("a")
  assertThrows(() => {
    // SyntaxError: Unexpected end of JSON input
    parse("aaa");
  }, SyntaxError);
  assertThrows(() => {
    // SyntaxError: Unexpected end of JSON input
    parse("ImEi..ImEi");
  }, SyntaxError);
  assertThrows((): void => {
    // TypeError: Illegal base64url string!
    parse("a");
  }, TypeError);
  assertThrows((): void => {
    // TypeError: invalid serialization
    parse("ImEi.ImEi.ImEi.ImEi");
  }, TypeError);

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
  assertEquals(parse(jwt), {
    header,
    payload,
    signature:
      "49f94ac7044948c78a285d904f87f0a4c7897f7e8f3a4eb2255fda750b2cc397",
  });
  assertEquals(
    create(payload, "your-256-bit-secret", { header }),
    jwt,
  );
});

Deno.test("[jwt] expired token", function (): void {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
    exp: setExpiration(-20000),
  };
  const header: Header = {
    alg: "HS256",
    dummy: 100,
  };
  const jwt = create(payload, key, { header });

  try {
    verify(jwt, key, { algorithm: "HS256" });
  } catch (error) {
    assertEquals(error.message, "jwt is expired");
  }
});

Deno.test("[jwt] none algorithm", function (): void {
  const payload = {
    iss: "joe",
    jti: "123456789abc",
  };
  const header: Header = {
    alg: "none",
    dummy: 100,
  };
  const jwt = create(payload, key, { header });
  const validatedPayload = verify(
    jwt,
    "keyIsIgnored",
    {
      algorithm: "none",
    },
  );
  assertEquals(validatedPayload, payload);
});

Deno.test("[jwt] HS256 algorithm", function (): void {
  const header: Header = {
    alg: "HS256",
    typ: "JWT",
  };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    iat: 1516239022,
  };
  const jwt = create(payload, key, { header });
  const validatedPayload = verify(jwt, key, { algorithm: "HS256" });
  assertEquals(
    jwt,
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SARsBE5x_ua2ye823r2zKpQNaew3Daq8riKz5A4h3o4",
  );
  assertEquals(validatedPayload, payload);
  try {
    const invalidJwt = // jwt with not supported crypto algorithm in alg header:
      "eyJhbGciOiJIUzM4NCIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWUsImlhdCI6MTUxNjIzOTAyMn0.bQTnz6AuMJvmXXQsVPrxeQNvzDkimo7VNXxHeSBfClLufmCVZRUuyTwJF311JHuh";
    verify(invalidJwt, "", {
      algorithm: "HS256",
    });
  } catch (error) {
    assertEquals(error.message, "algorithms do not match");
  }
});

Deno.test("[jwt] HS512 algorithm", function (): void {
  const header: Header = { alg: "HS512", typ: "JWT" };
  const payload = {
    sub: "1234567890",
    name: "John Doe",
    admin: true,
    iat: 1516239022,
  };
  const jwt = create(payload, key, { header });
  const validatedPayload = verify(jwt, key, { algorithm: "HS512" });
  assertEquals(validatedPayload, payload);
});
