// this should still vendor
if (false) {
  await import("./non-existent.js");
}

export class Logger {
  log(text: string) {
    console.log(text);
  }
}
