// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import { MarkdownOptions, parse } from "./markdown.ts";

const FIXTURE_DIR = "./std/convert/fixtures";

const inputs = Deno.readDirSync(FIXTURE_DIR);
const decoder = new TextDecoder();

function parseOptions(str: string): [string, Partial<MarkdownOptions> | undefined] {
  const directiveBlock = /^-{3}\s*\n(.*)-{3}\s*\n/gms.exec(str);
  if (!directiveBlock) {
    return [str, undefined];
  }
  const [match, directives] = directiveBlock;
  const options: Partial<MarkdownOptions> = {};
  for (const directiveLine of directives.split("\n")) {
    if (!directiveLine) {
      continue;
    }
    const directive = /^([^:]+):\s*(.+)$/.exec(directiveLine);
    assert(directive);
    const [, option, rawValue] = directive;
    const valueString = rawValue.trim();
    let value: string | number | boolean | undefined;
    if (valueString.toLowerCase() === "true") {
      value = true;
    } else if (valueString.toLowerCase() === "false") {
      value = false;
    } else {
      const intValue = parseInt(valueString, 10);
      if (intValue || intValue === 0) {
        value = intValue;
      } else {
        if (valueString.charAt(0) === "\"") {
          value = valueString.substring(1, valueString.length - 2);
        }
      }
    }
    if (value) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (options as any)[option] = value;
    }
  }
  return [str.substring(match.length - 1), options];
}

for (const file of inputs) {
  assert(file.name);
  if (file.name.endsWith(".md")) {
    const name = `convert - markdown - ${file.name.replace(/\.md$/, "")}`;
    const [input, options] = parseOptions(
      decoder.decode(Deno.readFileSync(`${FIXTURE_DIR}/${file.name}`))
    );
    const expected = decoder.decode(
      Deno.readFileSync(`${FIXTURE_DIR}/${file.name.replace(/\.md$/, ".html")}`)
    );
    assert(expected);
    test({
      name,
      fn() {
        const actual = parse(input, options);
        assert(actual);
        assertEquals(actual.replace(/\n/g, ""), expected.replace(/\n/g, ""));
      }
    });
  }
}
