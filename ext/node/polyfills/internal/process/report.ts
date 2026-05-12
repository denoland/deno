// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  StringPrototypeToUpperCase,
  StringPrototypeCharAt,
  StringPrototypeSlice,
  Date,
  DatePrototypeGetTime,
} = primordials;

const { arch, versions } = core.loadExtScript(
  "ext:deno_node/_process/process.ts",
);
const lazyOs = core.createLazyLoader("node:os");

function writeReport(_filename, _err) {
  return "";
}

const todoUndefined = undefined;

// Node only sets `glibcVersionRuntime` / `glibcVersionCompiler` on Linux
// binaries linked against glibc. On musl Linux and non-Linux platforms the
// fields are absent, and tools such as rollup rely on that to detect the
// libc flavor (denoland/deno#33948). Evaluated inside `getReport` because
// `core.build` isn't populated until startup runs `setBuildInfo` after the
// snapshot is restored.
function getGlibcVersions() {
  return core.build.os === "linux" && core.build.env === "gnu"
    ? { glibcVersionRuntime: "2.38", glibcVersionCompiler: "2.38" }
    : {};
}

function getReport(_err) {
  const os = lazyOs();
  const dumpEventTime = new Date();
  return {
    header: {
      reportVersion: 3,
      event: "JavaScript API",
      trigger: "GetReport",
      filename: report.filename, // assumption!
      dumpEventTime,
      dumpEventTimeStamp: DatePrototypeGetTime(dumpEventTime),
      processId: Deno.pid, // I am not sure if it should be Deno.pid or Deno.ppid
      threadId: 0,
      cwd: Deno.cwd(),
      commandLine: ["node"],
      nodejsVersion: `v${versions.node}`,
      ...getGlibcVersions(),
      wordSize: 64,
      arch: arch(),
      platform: Deno.build.os,
      componentVersions: versions,
      release: {
        name: "node",
        headersUrl:
          "https://nodejs.org/download/release/v21.2.0/node-v21.2.0-headers.tar.gz",
        sourceUrl:
          "https://nodejs.org/download/release/v21.2.0/node-v21.2.0.tar.gz",
      },
      osName:
        StringPrototypeToUpperCase(StringPrototypeCharAt(Deno.build.os, 0)) +
        StringPrototypeSlice(Deno.build.os, 1),
      osRelease: todoUndefined,
      osVersion: todoUndefined,
      osMachine: Deno.build.arch,
      cpus: os.cpus(),
      networkInterfaces: os.networkInterfaces(),
      host: os.hostname(),
    },
    javascriptStack: todoUndefined,
    javascriptHeap: todoUndefined,
    nativeStack: todoUndefined,
    resourceUsage: todoUndefined,
    uvthreadResourceUsage: todoUndefined,
    libuv: todoUndefined,
    workers: [],
    environmentVariables: todoUndefined,
    userLimits: todoUndefined,
    sharedObjects: todoUndefined,
  };
}

// https://nodejs.org/api/process.html#processreport
const report = {
  compact: false,
  directory: "",
  filename: "",
  getReport,
  reportOnFatalError: false,
  reportOnSignal: false,
  reportOnUncaughtException: false,
  signal: "SIGUSR2",
  writeReport,
};

return {
  report,
};
})();
