import { listenAndServe } from "./http.ts";
import { open, cwd } from "deno";

const addr = "0.0.0.0:4500";
const d = cwd();

listenAndServe(addr, async (req) => {
  const filename = d + "/" + req.url;
  let res;
  try {
    res = { status: 200, body: open(filename) };
  } catch(e) {
    res = { status: 500, body: "bad" };
  }
  req.respond(res);
});

console.log(`HTTP server listening on http://${addr}/`);
