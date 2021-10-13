// todo(dsherret): use ./subdir/a.ts once fixtures are restored
export { a as test1 } from "./file_tests-fixture16_2.ts";
export { a as test2 } from "./file_tests-fixture16_2.ts";
import { a } from "./file_tests-fixture16_2.ts";

console.log(a);
