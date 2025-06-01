import * as url from "url";

export function foobar(): { href: string } {
  return url.pathToFileURL("/foo/bar");
}
