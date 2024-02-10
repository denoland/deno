// deno-lint-ignore-file

// check that console methods provided by V8 are available in the inspector
console.timeStamp("foo");
console.profile("foo");
console.profileEnd("foo");

for (let i = 0; i < 128; i++) {
  console.log(i);
  debugger;
}
await new Promise((res, _) => setTimeout(res, 100));
console.log("done");
