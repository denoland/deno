import { decode, serve } from "./example_deps.ts";
import { create, setExpiration, verify } from "../mod.ts";
import type { Header, Payload } from "../mod.ts";

const key = "your-secret";
const payload: Payload = {
  iss: "joe",
  exp: setExpiration(60),
};
const header: Header = {
  alg: "HS256",
  typ: "JWT",
};

console.log("server is listening at 0.0.0.0:8000");
for await (const req of serve("0.0.0.0:8000")) {
  if (req.method === "GET") {
    req.respond({ body: (await create({ header, payload, key })) + "\n" });
  } else {
    const jwt = decode(await Deno.readAll(req.body));
    req.respond({
      body: JSON.stringify(await verify({ jwt, key, algorithm: "HS256" })),
    });
  }
}
