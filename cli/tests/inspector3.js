// deno-lint-ignore-file
for (let i = 0; i < 128; i++) {
  console.log(i);
  debugger;
}
await new Promise((res, _) => setTimeout(res, 100));
console.log("done");
