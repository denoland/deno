// deno-lint-ignore-file prefer-const no-unused-vars no-explicit-any
import type * as types from "./types.ts";
import * as common from "./common.ts";
import * as ourselves from "./mod.ts";
// import * as denoflate from "https://deno.land/x/denoflate@1.2.1/mod.ts";

const ESBUILD_VERSION: string = "0.25.4";

export let version = ESBUILD_VERSION;

export let build: typeof types.build = (options: types.BuildOptions) =>
  ensureServiceIsRunning().then((service) => service.build(options));

export const context: typeof types.context = (options: types.BuildOptions) =>
  ensureServiceIsRunning().then((service) => service.context(options));

export const formatMessages: typeof types.formatMessages = (
  messages,
  options,
) =>
  ensureServiceIsRunning().then((service) =>
    service.formatMessages(messages, options)
  );

export const analyzeMetafile: typeof types.analyzeMetafile = (
  metafile,
  options,
) =>
  ensureServiceIsRunning().then((service) =>
    service.analyzeMetafile(metafile, options)
  );

export const stop = async () => {
  if (stopService) await stopService();
};

let initializeWasCalled = false;

export const initialize: typeof types.initialize = async (options) => {
  options = common.validateInitializeOptions(options || {});
  if (options.wasmURL) {
    throw new Error(`The "wasmURL" option only works in the browser`);
  }
  if (options.wasmModule) {
    throw new Error(`The "wasmModule" option only works in the browser`);
  }
  if (options.worker) {
    throw new Error(`The "worker" option only works in the browser`);
  }
  if (initializeWasCalled) {
    throw new Error('Cannot call "initialize" more than once');
  }
  await ensureServiceIsRunning();
  initializeWasCalled = true;
};

async function installFromNPM(name: string, subpath: string): Promise<string> {
  const { finalPath, finalDir } = getCachePath(name);
  try {
    await Deno.stat(finalPath);
    return finalPath;
  } catch (e) {
    //
  }

  const npmRegistry = Deno.env.get("NPM_CONFIG_REGISTRY") ||
    "https://registry.npmjs.org";
  const url = `${npmRegistry}/${name}/-/${
    name.replace("@esbuild/", "")
  }-${version}.tgz`;
  const buffer = await fetch(url).then((r) => r.arrayBuffer());
  const executable = extractFileFromTarGzip(new Uint8Array(buffer), subpath);

  await Deno.mkdir(finalDir, {
    recursive: true,
    mode: 0o700, // https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
  });

  await Deno.writeFile(finalPath, executable, { mode: 0o755 });
  return finalPath;
}

function getCachePath(name: string): { finalPath: string; finalDir: string } {
  let baseDir: string | undefined;

  switch (Deno.build.os) {
    case "darwin":
      baseDir = Deno.env.get("HOME");
      if (baseDir) baseDir += "/Library/Caches";
      break;

    case "windows":
      baseDir = Deno.env.get("LOCALAPPDATA");
      if (!baseDir) {
        baseDir = Deno.env.get("USERPROFILE");
        if (baseDir) baseDir += "/AppData/Local";
      }
      if (baseDir) baseDir += "/Cache";
      break;

    case "linux": {
      // https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
      const xdg = Deno.env.get("XDG_CACHE_HOME");
      if (xdg && xdg[0] === "/") baseDir = xdg;
      break;
    }
  }

  if (!baseDir) {
    baseDir = Deno.env.get("HOME");
    if (baseDir) baseDir += "/.cache";
  }

  if (!baseDir) throw new Error("Failed to find cache directory");
  const finalDir = baseDir + `/esbuild/bin`;
  const finalPath = finalDir + `/${name.replace("/", "-")}@${version}`;
  return { finalPath, finalDir };
}

function extractFileFromTarGzip(buffer: Uint8Array, file: string): Uint8Array {
  try {
    buffer = denoflate.gunzip(buffer);
  } catch (err: any) {
    throw new Error(
      `Invalid gzip data in archive: ${err && err.message || err}`,
    );
  }
  let str = (i: number, n: number) =>
    String.fromCharCode(...buffer.subarray(i, i + n)).replace(/\0.*$/, "");
  let offset = 0;
  file = `package/${file}`;
  while (offset < buffer.length) {
    let name = str(offset, 100);
    let size = parseInt(str(offset + 124, 12), 8);
    offset += 512;
    if (!isNaN(size)) {
      if (name === file) return buffer.subarray(offset, offset + size);
      offset += (size + 511) & ~511;
    }
  }
  throw new Error(`Could not find ${JSON.stringify(file)} in archive`);
}

async function install(): Promise<string> {
  const overridePath = Deno.env.get("ESBUILD_BINARY_PATH");
  if (overridePath) return overridePath;

  const platformKey = Deno.build.target;
  const knownWindowsPackages: Record<string, string> = {
    "x86_64-pc-windows-msvc": "@esbuild/win32-x64",
  };
  const knownUnixlikePackages: Record<string, string> = {
    // These are the only platforms that Deno supports
    "aarch64-apple-darwin": "@esbuild/darwin-arm64",
    "aarch64-unknown-linux-gnu": "@esbuild/linux-arm64",
    "x86_64-apple-darwin": "@esbuild/darwin-x64",
    "x86_64-unknown-linux-gnu": "@esbuild/linux-x64",

    // These platforms are not supported by Deno
    "aarch64-linux-android": "@esbuild/android-arm64",
    "x86_64-unknown-freebsd": "@esbuild/freebsd-x64",
    "x86_64-alpine-linux-musl": "@esbuild/linux-x64",
  };

  // Pick a package to install
  if (platformKey in knownWindowsPackages) {
    return await installFromNPM(
      knownWindowsPackages[platformKey],
      "esbuild.exe",
    );
  } else if (platformKey in knownUnixlikePackages) {
    return await installFromNPM(
      knownUnixlikePackages[platformKey],
      "bin/esbuild",
    );
  } else {
    throw new Error(`Unsupported platform: ${platformKey}`);
  }
}

interface Service {
  build: typeof types.build;
  context: typeof types.context;
  formatMessages: typeof types.formatMessages;
  analyzeMetafile: typeof types.analyzeMetafile;
}

let defaultWD = Deno.cwd();
let longLivedService: Promise<Service> | undefined;
let stopService: (() => Promise<void>) | undefined;

// Declare a common subprocess API for the two implementations below
type SpawnFn = (cmd: string, options: {
  args: string[];
  stdin: "piped" | "inherit";
  stdout: "piped" | "inherit";
  stderr: "inherit";
}) => {
  write(bytes: Uint8Array): void;
  read(): Promise<Uint8Array | null>;
  close(): Promise<void> | void;
  status(): Promise<{ code: number }>;
};

// Deno ≥1.40
const spawnNew: SpawnFn = (cmd, { args, stdin, stdout, stderr }) => {
  const child = new Deno.Command(cmd, {
    args,
    cwd: defaultWD,
    stdin,
    stdout,
    stderr,
  }).spawn();
  // Note: Need to check for "piped" in Deno ≥1.31.0 to avoid a crash
  const writer = stdin === "piped" ? child.stdin.getWriter() : null;
  const reader = stdout === "piped" ? child.stdout.getReader() : null;
  return {
    write: writer ? (bytes) => writer.write(bytes) : () => Promise.resolve(),
    read: reader
      ? () => reader.read().then((x) => x.value || null)
      : () => Promise.resolve(null),
    close: async () => {
      // We can't call "kill()" because it doesn't seem to work. Tests will
      // still fail with "A child process was opened during the test, but not
      // closed during the test" even though we kill the child process.
      //
      // And we can't call both "writer.close()" and "kill()" because then
      // there's a race as the child process exits when stdin is closed, and
      // "kill()" fails when the child process has already been killed.
      //
      // So instead we just call "writer.close()" and then hope that this
      // causes the child process to exit. It won't work if the stdin consumer
      // thread in the child process is hung or busy, but that may be the best
      // we can do.
      //
      // See this for more info: https://github.com/evanw/esbuild/pull/3611
      if (writer) await writer.close();
      if (reader) await reader.cancel();

      // Wait for the process to exit. The new "kill()" API doesn't flag the
      // process as having exited because processes can technically ignore the
      // kill signal. Without this, Deno will fail tests that use esbuild with
      // an error because the test spawned a process but didn't wait for it.
      await child.status;
    },
    status: () => child.status,
  };
};

// This is a shim for "Deno.run" for newer versions of Deno
const spawn: SpawnFn = spawnNew;

const ensureServiceIsRunning = (): Promise<Service> => {
  if (!longLivedService) {
    longLivedService = (async (): Promise<Service> => {
      const binPath = await install();
      const isTTY = Deno.stderr.isTerminal();
      const child = spawn(binPath, {
        args: [`--service=${version}`],
        stdin: "piped",
        stdout: "piped",
        stderr: "inherit",
      });

      stopService = async () => {
        // Close all resources related to the subprocess.
        await child.close();
        initializeWasCalled = false;
        longLivedService = undefined;
        stopService = undefined;
      };

      const { readFromStdout, afterClose, service } = common.createChannel({
        writeToStdin(bytes) {
          child.write(bytes);
        },
        isSync: false,
        hasFS: true,
        esbuild: ourselves,
      });

      const readMoreStdout = () =>
        child.read().then((buffer) => {
          if (buffer === null) {
            afterClose(null);
          } else {
            readFromStdout(buffer);
            readMoreStdout();
          }
        }).catch((e) => {
          if (
            e instanceof Deno.errors.Interrupted ||
            e instanceof Deno.errors.BadResource
          ) {
            // ignore the error if read was interrupted (stdout was closed)
            afterClose(e);
          } else {
            throw e;
          }
        });
      readMoreStdout();

      return {
        build: (options: types.BuildOptions) =>
          new Promise<types.BuildResult>((resolve, reject) => {
            service.buildOrContext({
              callName: "build",
              refs: null,
              options,
              isTTY,
              defaultWD,
              callback: (err, res) =>
                err ? reject(err) : resolve(res as types.BuildResult),
            });
          }),

        context: (options: types.BuildOptions) =>
          new Promise<types.BuildContext>((resolve, reject) =>
            service.buildOrContext({
              callName: "context",
              refs: null,
              options,
              isTTY,
              defaultWD,
              callback: (err, res) =>
                err ? reject(err) : resolve(res as types.BuildContext),
            })
          ),
        formatMessages: (messages, options) =>
          new Promise((resolve, reject) =>
            service.formatMessages({
              callName: "formatMessages",
              refs: null,
              messages,
              options,
              callback: (err, res) => err ? reject(err) : resolve(res!),
            })
          ),

        analyzeMetafile: (metafile, options) =>
          new Promise((resolve, reject) =>
            service.analyzeMetafile({
              callName: "analyzeMetafile",
              refs: null,
              metafile: typeof metafile === "string"
                ? metafile
                : JSON.stringify(metafile),
              options,
              callback: (err, res) => err ? reject(err) : resolve(res!),
            })
          ),
      };
    })();
  }
  return longLivedService;
};
