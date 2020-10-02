import { makeJwt, setExpiration } from "../create.ts";
import { validateJwt } from "../validate.ts";

const payload = {
  iss: "joe",
  jti: "123456789abc",
  exp: setExpiration(new Date("July 26, 2040 22:43:00")),
};
const header = {
  alg: "HS256" as const,
  crit: ["dummy"],
  dummy: 100,
};
const critHandlers = {
  dummy(value: any) {
    console.log(`dummy works: ${value}`);
    return value * 2;
  },
};
const key = "abc123";

const jwt = await makeJwt({ header, payload, key });
console.log("JWT:", jwt);
const validatedJwt = await validateJwt({
  jwt,
  key,
  critHandlers,
  algorithm: ["HS256", "HS512"],
});
if (validatedJwt.isValid) console.log("JWT is valid!\n", validatedJwt);
else console.log("JWT is invalid!\n", validatedJwt);
