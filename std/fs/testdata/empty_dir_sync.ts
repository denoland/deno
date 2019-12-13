import { emptyDirSync } from "../empty_dir.ts";

try {
  emptyDirSync(Deno.args[1])
  Deno.stdout.write(new TextEncoder().encode("success"))
} catch (err) {
  Deno.stdout.write(new TextEncoder().encode(err.message))
}