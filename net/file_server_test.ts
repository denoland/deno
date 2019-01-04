import { readFile } from "deno";

import {
  test,
  assert,
  assertEqual
} from "../testing/mod.ts";

// Promise to completeResolve when all tests completes
let completeResolve;
export const completePromise = new Promise(res => (completeResolve = res));
let completedTestCount = 0;

function maybeCompleteTests() {
  completedTestCount++;
  // Change this when adding more tests
  if (completedTestCount === 3) {
    completeResolve();
  }
}

export function runTests(serverReadyPromise: Promise<any>) {
  test(async function serveFile() {
    await serverReadyPromise;
    const res = await fetch("http://localhost:4500/azure-pipelines.yml");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEqual(res.headers.get("content-type"), "text/yaml");
    const downloadedFile = await res.text();
    const localFile = new TextDecoder().decode(await readFile("./azure-pipelines.yml"));
    assertEqual(downloadedFile, localFile);
    maybeCompleteTests();
  });

  test(async function serveDirectory() {
    await serverReadyPromise;
    const res = await fetch("http://localhost:4500/");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    const page = await res.text();
    assert(page.includes("azure-pipelines.yml"));
    maybeCompleteTests();
  });

  test(async function serveFallback() {
    await serverReadyPromise;
    const res = await fetch("http://localhost:4500/badfile.txt");
    assert(res.headers.has("access-control-allow-origin"));
    assert(res.headers.has("access-control-allow-headers"));
    assertEqual(res.status, 404);
    maybeCompleteTests();
  });
}
