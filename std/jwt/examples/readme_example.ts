import { serve } from "./example_deps.ts";
import { validateJwt } from "../validate.ts";
import { makeJwt, setExpiration, Jose, Payload } from "../create.ts";

const key = "your-secret";
const payload: Payload = {
  iss: "joe",
  exp: setExpiration(60),
};
const header: Jose = {
  alg: "HS256",
  typ: "JWT",
};

console.log("server is listening at 0.0.0.0:8000");
for await (const req of serve("0.0.0.0:8000")) {
  if (req.method === "GET") {
    req.respond({ body: (await makeJwt({ header, payload, key })) + "\n" });
  } else {
    const jwt = new TextDecoder().decode(await Deno.readAll(req.body));
    (await validateJwt({ jwt, key, algorithm: "HS256" })).isValid
      ? req.respond({ body: "Valid JWT\n" })
      : req.respond({ body: "Invalid JWT\n", status: 401 });
  }
}
