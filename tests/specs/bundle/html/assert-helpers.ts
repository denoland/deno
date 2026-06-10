export function assertFileContains(path: string, pattern: string | RegExp) {
  const contents = Deno.readTextFileSync(path);
  let matcher: (s: string) => boolean;
  if (typeof pattern === "string") {
    matcher = (s) => s.includes(pattern);
  } else {
    matcher = (s) => pattern.test(s);
  }
  if (!matcher(contents)) {
    let message = "";
    message += "file does not contain the pattern: " + path + "\n";
    message += "wanted: " + pattern + "\n";
    message += "found: " + contents + "\n";
    throw new Error(message);
  }
}

export function assertFileDoesNotContain(
  path: string,
  pattern: string | RegExp,
) {
  try {
    assertFileContains(path, pattern);
  } catch (_e) {
    return;
  }
  let message = "";
  message += "file contains the pattern: " + path + "\n";
  message += "did not want: " + pattern + "\n";
  message += "found: " + Deno.readTextFileSync(path) + "\n";
  throw new Error(message);
}
