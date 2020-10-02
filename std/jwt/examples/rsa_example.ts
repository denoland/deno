import { serve } from "./example_deps.ts";
import { validateJwt } from "../validate.ts";
import { makeJwt, Jose, Payload } from "../create.ts";

const publicKey = Deno.readTextFileSync("./public.pem");
const privateKey = Deno.readTextFileSync("./private.pem");
const payload: Payload = {
  sub: "1234567890",
  name: "John Doe",
  admin: true,
  iat: 1516239022,
};
const header: Jose = {
  alg: "RS256",
  typ: "JWT",
};

console.log("server is listening at 0.0.0.0:8000");
for await (const req of serve("0.0.0.0:8000")) {
  if (req.method === "GET") {
    req.respond({
      body: (await makeJwt({ header, payload, key: privateKey })) + "\n",
    });
  } else {
    const jwt = new TextDecoder().decode(await Deno.readAll(req.body));
    (await validateJwt({ jwt, key: publicKey, algorithm: "RS256" })).isValid
      ? req.respond({ body: "Valid JWT\n" })
      : req.respond({ body: "Invalid JWT\n", status: 401 });
  }
}
