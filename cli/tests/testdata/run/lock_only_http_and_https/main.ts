import "http://127.0.0.1:4545/run/003_relative_import.ts";
import { a } from "data:application/typescript;base64,ZW51bSBBIHsKICBBLAogIEIsCiAgQywKIH0KIAogZXhwb3J0IGZ1bmN0aW9uIGEoKSB7CiAgIHRocm93IG5ldyBFcnJvcihgSGVsbG8gJHtBLkN9YCk7CiB9CiA=";
import { b } from "./b.ts";

console.log(a);
console.log(b);
