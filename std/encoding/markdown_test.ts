// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import { dirname, join, extname } from "../path/mod.ts";
import { parse as yamlParse } from "../encoding/yaml/parse.ts";

import { MarkdownOptions, parse } from "./markdown.ts";

const FIXTURE_DIR = join(
  dirname(import.meta.url.replace("file://", "")),
  "testdata",
  "markdown"
);

let inputs = [];
if (Deno.build.os !== "win") {
  inputs = Deno.readDirSync(FIXTURE_DIR).filter(
    info => extname(info.name || "") === ".md"
  );
}

const decoder = new TextDecoder();

function parseOptions(
  str: string
): [string, Partial<MarkdownOptions> | undefined] {
  const directiveBlock = /^-{3}\s*\n(.*?)-{3}\s*\n/gms.exec(str);
  if (!directiveBlock) {
    return [str, undefined];
  }
  const [match, directives] = directiveBlock;
  return [
    str.substring(match.length - 1),
    yamlParse(directives) as Partial<MarkdownOptions>
  ];
}

for (const { name: fileName } of inputs) {
  assert(fileName);
  const fixture = decoder.decode(
    Deno.readFileSync(join(FIXTURE_DIR, fileName))
  );
  const [input, options] = parseOptions(fixture);
  const expected = decoder.decode(
    Deno.readFileSync(join(FIXTURE_DIR, fileName.replace(/\.md$/, ".html")))
  );
  assert(expected);
  test({
    name: `[markdown] ${fileName.replace(/\.md$/, "")}`,
    fn() {
      const actual = parse(input, options);
      assert(actual);
      assertEquals(actual.replace(/\n/g, ""), expected.replace(/\n/g, ""));
    }
  });
}
