import { value } from "package";

// should cause a type error where the type of value is "expected"
const local: "not" = value;
console.log(local);
