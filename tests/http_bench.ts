// Used for benchmarking Deno's networking. See tools/http_benchmark.py
import { serve } from "https://deno.land/x/net/http.ts";

const addr = "127.0.0.1:4500";
const s = serve(addr);
console.log(`listening on http://${addr}/`);

const body = (new TextEncoder()).encode("Hello World\n");


async function main() {
  for await (const req of s) {
  await req.respond({ status: 200, body });
  }
}

main();
