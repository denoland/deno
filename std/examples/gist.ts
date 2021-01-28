#!/usr/bin/env -S deno run --allow-net --allow-env
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// A program to post files to gist.github.com. Use the following to install it:
// deno install -f --allow-env --allow-read --allow-net=api.github.com https://deno.land/std/examples/gist.ts
import { parse } from "../flags/mod.ts";

function pathBase(p: string): string {
  const parts = p.split("/");
  return parts[parts.length - 1];
}

const token = Deno.env.get("GIST_TOKEN");
if (!token) {
  console.error("GIST_TOKEN environmental variable not set.");
  console.error("Get a token here: https://github.com/settings/tokens");
  Deno.exit(1);
}

const parsedArgs = parse(Deno.args);

if (parsedArgs._.length === 0) {
  console.error(
    "Usage: gist.ts --allow-env --allow-net [-t|--title Example] some_file " +
      "[next_file]",
  );
  Deno.exit(1);
}

const files: Record<string, { content: string }> = {};
for (const filename of parsedArgs._) {
  const base = pathBase(filename as string);
  const content = await Deno.readFile(filename as string);
  const contentStr = new TextDecoder().decode(content);
  files[base] = { content: contentStr };
}

const content = {
  description: parsedArgs.title || parsedArgs.t || "Example",
  public: false,
  files: files,
};
const body = JSON.stringify(content);

const res = await fetch("https://api.github.com/gists", {
  method: "POST",
  headers: [
    ["Content-Type", "application/json"],
    ["User-Agent", "Deno-Gist"],
    ["Authorization", `token ${token}`],
  ],
  body,
});

if (res.ok) {
  const resObj = await res.json();
  console.log("Success");
  console.log(resObj["html_url"]);
} else {
  const err = await res.text();
  console.error("Failure to POST", err);
}
