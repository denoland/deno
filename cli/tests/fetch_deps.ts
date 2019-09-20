// Run ./tools/http_server.py too in order for this test to run.
import { assert } from "../js/deps/https/deno.land/std/testing/asserts.ts";

// TODO Top level await https://github.com/denoland/deno/issues/471
async function main(): Promise<void> {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  const deps = Object.keys(json.devDependencies);
  console.log("Deno JS Deps");
  console.log(deps.map((d): string => `* ${d}`).join("\n"));
  assert(deps.includes("typescript"));
}

main();
