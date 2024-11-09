import * as a from "./a.cts";

await Deno.stdout.write(new TextEncoder().encode(a.add(1, 2)));
