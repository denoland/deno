import { join } from "node:path";
import { pathToFileURL } from "node:url";

const mode = Deno.args[0];
const specifier = pathToFileURL(
  join(Deno.cwd(), "allowed", `${mode}.cjs`),
).href;

try {
  const module = await import(specifier);
  if (mode === "reexport" && module.value === "from-inner") {
    console.log(module.value);
  } else {
    console.log("unexpected result");
    Deno.exit(1);
  }
} catch (err) {
  const message = err instanceof Error
    ? `${err.message}\n${err.stack ?? ""}`
    : String(err);
  if (
    mode === "denied" &&
    message.includes("Requires read access") &&
    !message.includes("DENO_STANDALONE_CJS_SOURCE_CANARY")
  ) {
    console.log("blocked");
  } else {
    console.log("unexpected error");
    Deno.exit(1);
  }
}
