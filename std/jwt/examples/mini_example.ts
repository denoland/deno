import { serve } from "./example_deps.ts";
import { encode, decode } from "./example_deps.ts";
import { makeJwt } from "../create.ts";
import { validateJwt } from "../validate.ts";

const jwtInput = {
  header: { typ: "JWT", alg: "HS256" as const },
  payload: { iss: "joe" },
  key: "abc123",
};

console.log("server is listening at 0.0.0.0:8000");
for await (const req of serve("0.0.0.0:8000")) {
  if (req.method === "GET") {
    req.respond({
      body: encode((await makeJwt(jwtInput)) + "\n"),
    });
  } else {
    (
        await validateJwt({
          jwt: decode(await Deno.readAll(req.body)),
          key: "abc123",
          algorithm: "HS256",
        })
      ).isValid
      ? req.respond({ body: encode("Valid JWT\n") })
      : req.respond({ status: 401, body: encode("Invalid JWT\n") });
  }
}
