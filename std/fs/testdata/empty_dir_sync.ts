import { emptyDirSync } from "../empty_dir.ts";

try {
  emptyDirSync(Deno.args[0])
  Deno.stdout.write(new TextEncoder().encode("success"))
} catch (err) {
  Deno.stdout.write(new TextEncoder().encode(err.message))
}