#!/usr/bin/env -S deno --allow-net --allow-env
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

const { args, env, exit, readFile } = Deno;
import { parse } from "https://deno.land/std/flags/mod.ts";

function pathBase(p: string): string {
  const parts = p.split("/");
  return parts[parts.length - 1];
}

async function main(): Promise<void> {
  const token = env()["GIST_TOKEN"];
  if (!token) {
    console.error("GIST_TOKEN environmental variable not set.");
    console.error("Get a token here: https://github.com/settings/tokens");
    exit(1);
  }

  const parsedArgs = parse(args.slice(1));

  if (parsedArgs._.length === 0) {
    console.error(
      "Usage: gist.ts --allow-env --allow-net [-t|--title Example] some_file " +
        "[next_file]"
    );
    exit(1);
  }

  const files = {};
  for (const filename of parsedArgs._) {
    const base = pathBase(filename);
    const content = await readFile(filename);
    const contentStr = new TextDecoder().decode(content);
    files[base] = { content: contentStr };
  }

  const content = {
    description: parsedArgs.title || parsedArgs.t || "Example",
    public: false,
    files: files
  };
  const body = JSON.stringify(content);

  const res = await fetch("https://api.github.com/gists", {
    method: "POST",
    headers: [
      ["Content-Type", "application/json"],
      ["User-Agent", "Deno-Gist"],
      ["Authorization", `token ${token}`]
    ],
    body
  });

  if (res.ok) {
    const resObj = await res.json();
    console.log("Success");
    console.log(resObj["html_url"]);
  } else {
    const err = await res.text();
    console.error("Failure to POST", err);
  }
}

main();
