// Copyright 2018-2026 the Deno authors. MIT license.

import {
  buildDownloadPlan,
  defaultBackendFor,
  laufeyArchiveName,
  laufeyTargetForBuild,
  parsePinnedSha256,
  parsePinnedVersion,
} from "./download_laufey.ts";

function assertEquals<T>(actual: T, expected: T, message?: string) {
  if (actual !== expected) {
    throw new Error(
      `${
        message ?? "values are not equal"
      }\nexpected: ${expected}\nactual:   ${actual}`,
    );
  }
}

function assertThrows(fn: () => unknown, expectedText: string) {
  try {
    fn();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.includes(expectedText)) {
      return;
    }
    throw new Error(
      `expected error containing '${expectedText}', got '${message}'`,
    );
  }
  throw new Error(`expected error containing '${expectedText}'`);
}

Deno.test("parsePinnedVersion reads lockfile directive", () => {
  assertEquals(
    parsePinnedVersion("# comment\n# version: v0.6.1\n"),
    "0.6.1",
  );
});

Deno.test("parsePinnedVersion rejects missing directive", () => {
  assertThrows(
    () =>
      parsePinnedVersion(
        "95b7dad3  laufey-cef-x86_64-unknown-linux-gnu.tar.gz",
      ),
    "missing a '# version: vX.Y.Z' directive",
  );
});

Deno.test("parsePinnedSha256 supports GNU sha256sum formats", () => {
  const contents = [
    "abc123  laufey-cef-x86_64-unknown-linux-gnu.tar.gz",
    "def456  *laufey-webview-x86_64-pc-windows-msvc.zip",
  ].join("\n");
  assertEquals(
    parsePinnedSha256(
      contents,
      "laufey-cef-x86_64-unknown-linux-gnu.tar.gz",
    ),
    "abc123",
  );
  assertEquals(
    parsePinnedSha256(
      contents,
      "laufey-webview-x86_64-pc-windows-msvc.zip",
    ),
    "def456",
  );
});

Deno.test("laufeyTargetForBuild maps supported targets", () => {
  assertEquals(
    laufeyTargetForBuild({ arch: "x86_64", os: "linux" }),
    "x86_64-unknown-linux-gnu",
  );
  assertEquals(
    laufeyTargetForBuild({ arch: "aarch64", os: "darwin" }),
    "aarch64-apple-darwin",
  );
  assertEquals(
    laufeyTargetForBuild({ arch: "x86_64", os: "windows" }),
    "x86_64-pc-windows-msvc",
  );
});

Deno.test("archive naming keeps raw cache key but uses winit upstream name", () => {
  assertEquals(defaultBackendFor("windows"), "webview");
  assertEquals(defaultBackendFor("linux"), "cef");
  assertEquals(
    laufeyArchiveName("raw", "x86_64-unknown-linux-gnu"),
    "laufey-winit-x86_64-unknown-linux-gnu.tar.gz",
  );
  assertEquals(
    laufeyArchiveName("webview", "x86_64-pc-windows-msvc"),
    "laufey-webview-x86_64-pc-windows-msvc.zip",
  );
});

Deno.test("buildDownloadPlan matches resolver cache layout", () => {
  const plan = buildDownloadPlan(
    "raw",
    "/tmp/native_laufey",
    { arch: "x86_64", os: "linux" },
    [
      "# version: v0.6.1",
      "b0db0c0892181481976da48291ff5befe1cdf81d6e1e2598d6c78b9ed2c616f5  laufey-winit-x86_64-unknown-linux-gnu.tar.gz",
    ].join("\n"),
  );

  assertEquals(plan.version, "0.6.1");
  assertEquals(plan.target, "x86_64-unknown-linux-gnu");
  assertEquals(plan.archive, "laufey-winit-x86_64-unknown-linux-gnu.tar.gz");
  assertEquals(
    plan.url,
    "https://github.com/littledivy/laufey/releases/download/v0.6.1/laufey-winit-x86_64-unknown-linux-gnu.tar.gz",
  );
  assertEquals(
    plan.targetDir,
    "/tmp/native_laufey/0.6.1/raw/x86_64-unknown-linux-gnu",
  );
  assertEquals(
    plan.markerPath,
    "/tmp/native_laufey/0.6.1/raw/x86_64-unknown-linux-gnu/.downloaded",
  );
});
