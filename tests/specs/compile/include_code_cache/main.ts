import { join } from "@std/url/join";

Deno.writeTextFileSync("main-evaluated.txt", "evaluated");
console.log(join);
