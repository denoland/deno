import { a, b, } from "./foobar1.ts";

function main() {
    // @ts-ignore
    console.log("main called", a, b);
}