import { serve } from "./http.ts";
//import { test } from "https://deno.land/x/testing/testing.ts";

const addr = "0.0.0.0:8000";
const s = serve(addr);
console.log(`listening on http://${addr}/`);

const body = new TextEncoder().encode("Hello World\n");

async function main() {
  for await (const req of s) {
    await req.respond({ status: 200, body });
  }
}

main();

/*
test(function basic() {
  console.log("ok");
});
  */
