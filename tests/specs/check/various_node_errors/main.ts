import * as test from "package/package.json";
import * as test2 from "#not-an-import";
import * as test3 from "#non-existent";
import * as test4 from "package2/dir"; // not allowed dir import
import * as test5 from "package3"; // invalid package target

console.log(test, test2, test3, test4, test5);
