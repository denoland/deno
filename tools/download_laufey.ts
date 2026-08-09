#!/usr/bin/env -S deno run --allow-read --allow-write --allow-net --allow-run
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Pre-downloads a pinned laufey desktop backend archive into the shared cache
// layout `deno desktop` expects, so CI (and local test runs) don't re-download
// the backend for every test's fresh `DENO_DIR`.
//
//   deno run -A tools/download_laufey.ts [backend] [cache_dir]
//
// The default backend stays `cef`, except on Windows where it stays `webview`
// because laufey v0.6.1 does not ship a `cef` build for
// `aarch64-pc-windows-msvc`.

const LAUFEY_SUMS_URL = new URL("../cli/laufey_sums.lock", import.meta.url);

function absolute(path: string): string {
  if (path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path)) {
    return path;
  }
  return `${Deno.cwd()}/${path}`;
}

async function exists(path: string): Promise<boolean> {
  try {
    await Deno.stat(path);
    return true;
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      return false;
    }
    throw error;
  }
}

export function defaultBackendFor(os: string): string {
  return os === "windows" ? "webview" : "cef";
}

export function parsePinnedVersion(contents: string): string {
  const match = contents.match(/^# version: v([0-9][^\s]*)$/m);
  if (match == null) {
    throw new Error(
      "cli/laufey_sums.lock is missing a '# version: vX.Y.Z' directive",
    );
  }
  return match[1];
}

export function parsePinnedSha256(
  contents: string,
  archive: string,
): string | null {
  for (const line of contents.split("\n")) {
    const match = line.match(/^([0-9a-fA-F]+)\s+\*?(\S+)$/);
    if (match?.[2] === archive) {
      return match[1];
    }
  }
  return null;
}

export function laufeyTargetForBuild(
  build: Pick<typeof Deno.build, "arch" | "os">,
): string {
  const arch = build.arch;
  if (arch !== "x86_64" && arch !== "aarch64") {
    throw new Error(`unsupported laufey architecture: ${arch}`);
  }

  const os = (() => {
    switch (build.os) {
      case "linux":
        return "unknown-linux-gnu";
      case "darwin":
        return "apple-darwin";
      case "windows":
        return "pc-windows-msvc";
      default:
        throw new Error(`unsupported laufey operating system: ${build.os}`);
    }
  })();

  return `${arch}-${os}`;
}

export function laufeyArchiveName(backend: string, target: string): string {
  const archiveBackend = backend === "raw" ? "winit" : backend;
  const extension = target.includes("windows") ? "zip" : "tar.gz";
  return `laufey-${archiveBackend}-${target}.${extension}`;
}

export function laufeyReleaseUrl(version: string, archive: string): string {
  return `https://github.com/littledivy/laufey/releases/download/v${version}/${archive}`;
}

type DownloadPlan = {
  archive: string;
  markerPath: string;
  parentDir: string;
  sha256: string;
  target: string;
  targetDir: string;
  url: string;
  version: string;
};

export function buildDownloadPlan(
  backend: string,
  cacheDir: string,
  build: Pick<typeof Deno.build, "arch" | "os">,
  lockContents: string,
): DownloadPlan {
  const version = parsePinnedVersion(lockContents);
  const target = laufeyTargetForBuild(build);
  const archive = laufeyArchiveName(backend, target);
  const sha256 = parsePinnedSha256(lockContents, archive);
  if (sha256 == null) {
    throw new Error(
      `no pinned SHA-256 for ${archive} in cli/laufey_sums.lock`,
    );
  }
  const targetDir = `${cacheDir}/${version}/${backend}/${target}`;
  return {
    archive,
    markerPath: `${targetDir}/.downloaded`,
    parentDir: `${cacheDir}/${version}/${backend}`,
    sha256: sha256.toLowerCase(),
    target,
    targetDir,
    url: laufeyReleaseUrl(version, archive),
    version,
  };
}

async function sha256Hex(data: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", Uint8Array.from(data));
  return Array.from(
    new Uint8Array(digest),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("");
}

async function downloadArchive(url: string): Promise<Uint8Array> {
  const response = await fetch(url, {
    headers: {
      "user-agent": `deno-desktop/${Deno.version.deno} (+https://deno.com)`,
    },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(
      `failed to download ${url}: ${response.status} ${response.statusText}`,
    );
  }
  const data = new Uint8Array(await response.arrayBuffer());
  if (data.length === 0) {
    throw new Error(`empty response from ${url}`);
  }
  return data;
}

async function runCommand(command: string, args: string[]): Promise<void> {
  const output = await new Deno.Command(command, {
    args,
    stderr: "inherit",
    stdout: "inherit",
  }).output();
  if (!output.success) {
    throw new Error(`command failed: ${command} ${args.join(" ")}`);
  }
}

async function expandZipWithPowerShell(
  archivePath: string,
  destination: string,
): Promise<void> {
  const quotedArchivePath = archivePath.replaceAll("'", "''");
  const quotedDestination = destination.replaceAll("'", "''");
  const command = `Expand-Archive -LiteralPath '${quotedArchivePath}' ` +
    `-DestinationPath '${quotedDestination}' -Force`;
  for (const shell of ["pwsh", "powershell"]) {
    try {
      await runCommand(shell, ["-NoLogo", "-NoProfile", "-Command", command]);
      return;
    } catch (error) {
      if (error instanceof Deno.errors.NotFound) {
        continue;
      }
    }
  }
  throw new Error("could not find PowerShell to extract laufey zip archive");
}

async function extractArchive(
  archivePath: string,
  archive: string,
  destination: string,
): Promise<void> {
  if (archive.endsWith(".tar.gz")) {
    await runCommand("tar", ["-xzf", archivePath, "-C", destination]);
    return;
  }
  if (archive.endsWith(".zip")) {
    try {
      await runCommand("tar", ["-xf", archivePath, "-C", destination]);
    } catch (error) {
      if (Deno.build.os !== "windows") {
        throw error;
      }
      await expandZipWithPowerShell(archivePath, destination);
    }
    return;
  }
  throw new Error(`unsupported laufey archive format: ${archive}`);
}

async function ensureDownloaded(plan: DownloadPlan): Promise<void> {
  if (await exists(plan.markerPath)) {
    return;
  }

  await Deno.mkdir(plan.parentDir, { recursive: true });
  const staging = await Deno.makeTempDir({
    dir: plan.parentDir,
    prefix: ".staging-",
  });

  let renamed = false;
  try {
    const data = await downloadArchive(plan.url);
    const actual = await sha256Hex(data);
    if (actual !== plan.sha256) {
      throw new Error(
        `checksum mismatch for ${plan.archive} (downloaded from ${plan.url})\n` +
          `  expected: ${plan.sha256}\n` +
          `  actual:   ${actual}`,
      );
    }

    const archivePath = `${staging}/${plan.archive}`;
    await Deno.writeFile(archivePath, data);
    await extractArchive(archivePath, plan.archive, staging);
    await Deno.remove(archivePath);
    await Deno.writeTextFile(`${staging}/.downloaded`, `v${plan.version}\n`);

    if (await exists(plan.markerPath)) {
      return;
    }

    if (await exists(plan.targetDir)) {
      await Deno.remove(plan.targetDir, { recursive: true });
    }

    try {
      await Deno.rename(staging, plan.targetDir);
      renamed = true;
    } catch (error) {
      if (await exists(plan.markerPath)) {
        return;
      }
      throw error;
    }
  } finally {
    if (!renamed) {
      await Deno.remove(staging, { recursive: true }).catch(() => {});
    }
  }
}

async function main() {
  const backend = Deno.args[0] ?? defaultBackendFor(Deno.build.os);
  const cacheDir = absolute(Deno.args[1] ?? "./target/.native_laufey");
  const lockContents = await Deno.readTextFile(LAUFEY_SUMS_URL);
  const plan = buildDownloadPlan(backend, cacheDir, Deno.build, lockContents);

  console.error(`Downloading laufey "${backend}" backend into ${cacheDir}...`);
  await ensureDownloaded(plan);
  console.log(cacheDir);
}

if (import.meta.main) {
  await main();
}
