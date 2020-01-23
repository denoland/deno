import { exists } from "../exists.ts";

exists(Deno.args[0])
  .then(isExist => {
    Deno.stdout.write(new TextEncoder().encode(isExist ? 'exist' :'not exist'))
  })
  .catch((err) => {
    Deno.stdout.write(new TextEncoder().encode(err.message))
  })

