// console.log intentionally misspelled to trigger a type error
consol.log("hello world!");

// the following error should be ignored and not output to the console
// @ts-ignore
const foo = new Foo();
