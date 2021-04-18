// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { delay, join, readLines, ROOT_PATH } from "../util.js";
import { assert, ManifestTestOptions, release, runPy } from "./utils.ts";
import { DOMParser } from "https://deno.land/x/deno_dom@v0.1.3-alpha2/deno-dom-wasm.ts";

export async function runWithTestUtil<T>(
  verbose: boolean,
  f: () => Promise<T>,
): Promise<T> {
  const proc = runPy(["wpt", "serve"], {
    stdout: verbose ? "inherit" : "piped",
    stderr: verbose ? "inherit" : "piped",
  });

  const start = performance.now();
  while (true) {
    await delay(1000);
    try {
      const req = await fetch("http://localhost:8000/");
      await req.body?.cancel();
      if (req.status == 200) {
        break;
      }
    } catch (err) {
      // do nothing if this fails
    }
    const passedTime = performance.now() - start;
    if (passedTime > 15000) {
      throw new Error("Timed out while trying to start wpt test util.");
    }
  }

  if (verbose) console.log(`Started wpt test util.`);

  try {
    return await f();
  } finally {
    if (verbose) console.log("Killing wpt test util.");
    proc.kill(2);
    await proc.status();
    proc.close();
  }
}

export interface TestResult {
  cases: TestCaseResult[];
  status: number;
  stderr: string;
}

export interface TestCaseResult {
  name: string;
  passed: boolean;
  status: number;
  message: string | null;
  stack: string | null;
}

export async function runSingleTest(
  url: URL,
  options: ManifestTestOptions,
  reporter: (result: TestCaseResult) => void,
): Promise<TestResult> {
  const bundle = await generateBundle(url);
  const tempFile = await Deno.makeTempFile({
    prefix: "wpt-bundle-",
    suffix: ".js",
  });
  await Deno.writeTextFile(tempFile, bundle);

  const proc = Deno.run({
    cmd: [
      join(ROOT_PATH, `./target/${release ? "release" : "debug"}/deno`),
      "run",
      "-A",
      "--location",
      url.toString(),
      tempFile,
      "[]",
    ],
    env: {
      NO_COLOR: "1",
    },
    stdout: "null",
    stderr: "piped",
  });

  const cases = [];
  let stderr = "";

  const lines = readLines(proc.stderr);
  for await (const line of lines) {
    if (line.startsWith("{")) {
      const data = JSON.parse(line);
      const result = { ...data, passed: data.status == 0 };
      cases.push(result);
      reporter(result);
    } else {
      stderr += line + "\n";
    }
  }

  const { code } = await proc.status();
  return {
    status: code,
    cases,
    stderr,
  };
}

async function generateBundle(location: URL): Promise<string> {
  const res = await fetch(location);
  const body = await res.text();
  const doc = new DOMParser().parseFromString(body, "text/html");
  assert(doc, "document should have been parsed");
  const scripts = doc.getElementsByTagName("script");
  const scriptContents = [];
  for (const script of scripts) {
    const src = script.getAttribute("src");
    if (src === "/resources/testharnessreport.js") {
      scriptContents.push(
        await Deno.readTextFile(
          join(ROOT_PATH, "./tools/wpt/testharnessreport.js"),
        ),
      );
    } else if (src) {
      const res = await fetch(new URL(src, location));
      scriptContents.push(await res.text());
    } else {
      scriptContents.push(script.textContent);
    }
  }
  return scriptContents.join("\n");
}
