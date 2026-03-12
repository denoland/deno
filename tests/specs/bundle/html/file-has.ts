const path = Deno.args[0];
const pattern = Deno.args[1];

declare global {
  interface RegExpConstructor {
    escape(string: string): string;
  }
}

const contents = Deno.readTextFileSync(path);

let matcher: RegExp | string = pattern;
if (pattern.startsWith("regex:")) {
  matcher = pattern.slice(6);
  matcher = new RegExp(matcher);
} else {
  matcher = new RegExp(RegExp.escape(pattern));
}

if (matcher.test(contents)) {
  console.log("true");
} else {
  console.log("false");
  console.log("wanted: ", matcher);
  console.log("found: ", contents);
}
