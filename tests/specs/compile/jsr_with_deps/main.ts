// this was previously hanging in deno compile and wouldn't work
import { join } from "jsr:@std/url@0.220/join";
import "jsr:@std/url@0.220/normalize";

console.log(join);

// ensure import.meta.resolve works in compile for jsr specifiers
console.log(import.meta.resolve("jsr:@std/url@0.220/join"));
