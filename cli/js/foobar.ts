import { a, b, } from "./foobar1.ts";

function foo() {
    // @ts-ignore
    console.log(Deno.core.dispatch(0));
}

foo();

function main() {
    // @ts-ignore
    console.log("main called", a, b);
}