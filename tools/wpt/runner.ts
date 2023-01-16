// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { delay, join, ROOT_PATH, TextLineStream, toFileUrl } from "../util.js";
import { assert, denoBinary, ManifestTestOptions, runPy } from "./utils.ts";
import { DOMParser } from "https://deno.land/x/deno_dom@v0.1.3-alpha2/deno-dom-wasm.ts";

export async function runWithTestUtil<T>(
  verbose: boolean,
  f: () => Promise<T>,
): Promise<T> {
  const proc = runPy([
    "wpt",
    "serve",
    "--config",
    "../../tools/wpt/config.json",
  ], {
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
    } catch (_err) {
      // do nothing if this fails
    }
    const passedTime = performance.now() - start;
    if (passedTime > 15000) {
      proc.kill("SIGINT");
      await proc.status;
      throw new Error("Timed out while trying to start wpt test util.");
    }
  }

  if (verbose) console.log(`Started wpt test util.`);

  try {
    return await f();
  } finally {
    if (verbose) console.log("Killing wpt test util.");
    proc.kill("SIGINT");
    await proc.status;
  }
}

export interface TestResult {
  cases: TestCaseResult[];
  harnessStatus: TestHarnessStatus | null;
  duration: number;
  status: number;
  stderr: string;
}

export interface TestHarnessStatus {
  status: number;
  message: string | null;
  stack: string | null;
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
  _options: ManifestTestOptions,
  reporter: (result: TestCaseResult) => void,
  inspectBrk: boolean,
): Promise<TestResult> {
  const bundle = await generateBundle(url);
  const tempFile = await Deno.makeTempFile({
    prefix: "wpt-bundle-",
    suffix: ".js",
  });

  try {
    await Deno.writeTextFile(tempFile, bundle);

    const startTime = new Date().getTime();

    const args = [
      "run",
      "-A",
      "--unstable",
    ];

    if (inspectBrk) {
      args.push("--inspect-brk");
    }

    args.push(
      "--enable-testing-features-do-not-use",
      "--location",
      url.toString(),
      "--cert",
      join(ROOT_PATH, `./tools/wpt/certs/cacert.pem`),
      tempFile,
      "[]",
    );

    const proc = new Deno.Command(denoBinary(), {
      args,
      env: {
        NO_COLOR: "1",
      },
      stdout: "null",
      stderr: "piped",
    }).spawn();

    const cases = [];
    let stderr = "";

    let harnessStatus = null;

    const lines = proc.stderr.pipeThrough(new TextDecoderStream()).pipeThrough(
      new TextLineStream(),
    );
    for await (const line of lines) {
      if (line.startsWith("{")) {
        const data = JSON.parse(line);
        const result = { ...data, passed: data.status == 0 };
        cases.push(result);
        reporter(result);
      } else if (line.startsWith("#$#$#{")) {
        harnessStatus = JSON.parse(line.slice(5));
      } else {
        stderr += line + "\n";
        console.error(line);
      }
    }

    const duration = new Date().getTime() - startTime;

    const { code } = await proc.status;
    return {
      status: code,
      harnessStatus,
      duration,
      cases,
      stderr,
    };
  } finally {
    await Deno.remove(tempFile);
  }
}

async function generateBundle(location: URL): Promise<string> {
  const res = await fetch(location);
  const body = await res.text();
  const doc = new DOMParser().parseFromString(body, "text/html");
  assert(doc, "document should have been parsed");
  const scripts = doc.getElementsByTagName("script");
  const scriptContents = [];
  let inlineScriptCount = 0;
  for (const script of scripts) {
    const src = script.getAttribute("src");
    if (src === "/resources/testharnessreport.js") {
      const url = toFileUrl(
        join(ROOT_PATH, "./tools/wpt/testharnessreport.js"),
      );
      const contents = await Deno.readTextFile(url);
      scriptContents.push([url.href, contents]);
    } else if (src) {
      const url = new URL(src, location);
      const res = await fetch(url);
      if (res.ok) {
        const contents = await res.text();
        scriptContents.push([url.href, contents]);
      }
    } else {
      const url = new URL(`#${inlineScriptCount}`, location);
      inlineScriptCount++;
      scriptContents.push([url.href, script.textContent]);
    }
  }

  return scriptContents.map(([url, contents]) => `
(function() {
  const [_,err] = Deno.core.evalContext(${JSON.stringify(contents)}, ${
    JSON.stringify(url)
  });
  if (err !== null) {
    throw err?.thrown;
  }
})();`).join("\n");
}
