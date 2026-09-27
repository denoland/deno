// Regression test: --deny-import written as an IP literal must block module
// loads whose hostname resolves to the denied IP. The import permission check
// runs on the specifier before DNS resolution, so on its own it only sees
// "localhost", which matches no deny rule.
import { add } from "http://localhost:4545/add.ts";

console.log(add(1, 2));
