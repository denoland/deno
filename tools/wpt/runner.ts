import { readLines } from "../util.js";
import { ManifestTestOptions, assert, release, runPy } from "./utils.ts";
import { DOMParser } from "https://deno.land/x/deno_dom@v0.1.3-alpha2/deno-dom-wasm.ts";

export async function runWithTestUtil<T>(
  verbose: boolean,
  f: () => Promise<T>
): Promise<T> {
  const proc = runPy(["wpt", "serve"], {
    stdout: verbose ? "inherit" : "piped",
    stderr: verbose ? "inherit" : "piped",
  });

  await new Promise((resolve) => setTimeout(resolve, 5000));

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
  path: string;
  cases: TestCaseResult[];
  status: number;
  stderr: string;
  expectFail: boolean;
}

export interface TestCaseResult {
  name: string;
  passed: boolean;
  status: number;
  message: string | null;
  stack: string | null;
  expectFail: boolean;
}

export async function runSingleTest(
  path: string,
  location: URL,
  options: ManifestTestOptions,
  expectation: boolean | string[],
  reporter: (result: TestCaseResult) => void
): Promise<TestResult> {
  const bundle = await generateBundle(location);
  const tempFile = await Deno.makeTempFile({
    prefix: "wpt-bundle-",
    suffix: ".js",
  });
  await Deno.writeTextFile(tempFile, bundle);

  const proc = Deno.run({
    cmd: [
      `./target/${release ? "release" : "debug"}/deno`,
      "run",
      "-A",
      "--location",
      location.toString(),
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
      const expectFail = Array.isArray(expectation)
        ? expectation.includes(data.name)
        : !expectation;
      const result = { ...data, passed: data.status == 0, expectFail };
      cases.push(result);
      reporter(result);
    } else {
      stderr += line + "\n";
    }
  }

  const { code } = await proc.status();
  return {
    path,
    status: code,
    cases,
    stderr,
    expectFail: expectation === false,
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
        await Deno.readTextFile("./tools/wpt/testharnessreport.js")
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
