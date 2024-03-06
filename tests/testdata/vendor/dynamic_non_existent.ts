// this should still vendor
// deno-lint-ignore no-constant-condition
if (false) {
  await import("./non-existent.js");
}

export class Logger {
  log(text: string) {
    console.log(text);
  }
}
