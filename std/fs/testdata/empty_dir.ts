import { emptyDir } from "../empty_dir.ts";

emptyDir(Deno.args[1])
  .then(() => {
    Deno.stdout.write(new TextEncoder().encode("success"))
  })
  .catch((err) => {
    Deno.stdout.write(new TextEncoder().encode(err.message))
  })