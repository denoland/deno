import { serve } from "./http.ts";
//import { test } from "https://deno.land/x/testing/testing.ts";

const addr = "0.0.0.0:8000";
const s = serve(addr);
console.log(`listening on http://${addr}/`);

async function main() {
  for await (const req of s) {
    console.log("Req", req);
    req.respond({ body: "Hello World\n" });
  }
}

main();

/*
test(function basic() {
  console.log("ok");
});
  */
