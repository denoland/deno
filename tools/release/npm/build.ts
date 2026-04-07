#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
// NOTICE: This deployment/npm folder was lifted from https://github.com/dprint/dprint/blob/0ba79811cc96d2dee8e0cf766a8c8c0fc44879c2/deployment/npm/
// with permission (Copyright 2019-2023 David Sherret)
import $ from "jsr:@david/dax@^0.42.0";
// @ts-types="npm:@types/decompress@4.2.7"
import decompress from "npm:decompress@4.2.1";
import { parseArgs } from "@std/cli/parse-args";

interface Package {
  zipFileName: string;
  os: "win32" | "darwin" | "linux";
  cpu: "x64" | "arm64";
  libc?: "glibc" | "musl";
}

const args = parseArgs(Deno.args, {
  boolean: ["publish", "publish-only"],
});
const packages: Package[] = [{
  zipFileName: "deno-x86_64-pc-windows-msvc.zip",
  os: "win32",
  cpu: "x64",
}, {
  zipFileName: "deno-aarch64-pc-windows-msvc.zip",
  os: "win32",
  cpu: "arm64",
}, {
  zipFileName: "deno-x86_64-apple-darwin.zip",
  os: "darwin",
  cpu: "x64",
}, {
  zipFileName: "deno-aarch64-apple-darwin.zip",
  os: "darwin",
  cpu: "arm64",
}, {
  zipFileName: "deno-x86_64-unknown-linux-gnu.zip",
  os: "linux",
  cpu: "x64",
  libc: "glibc",
}, {
  zipFileName: "deno-aarch64-unknown-linux-gnu.zip",
  os: "linux",
  cpu: "arm64",
  libc: "glibc",
}];

const markdownText = `# Deno

[Deno](https://deno.com)
([/ˈdiːnoʊ/](https://ipa-reader.com/?text=%CB%88di%CB%90no%CA%8A), pronounced
\`dee-no\`) is a JavaScript, TypeScript, and WebAssembly runtime with secure
defaults and a great developer experience. It's built on [V8](https://v8.dev/),
[Rust](https://www.rust-lang.org/), and [Tokio](https://tokio.rs/).

Learn more about the Deno runtime
[in the documentation](https://docs.deno.com/runtime/manual).
`;

const currentDir = $.path(import.meta.dirname!);
const rootDir = currentDir.parentOrThrow().parentOrThrow().parentOrThrow();
const outputDir = currentDir.join("./dist");
const scopeDir = outputDir.join("@deno");
const denoDir = outputDir.join("deno");
const version = resolveVersion();

$.logStep(`Publishing ${version}...`);

if (!args["publish-only"]) {
  await $`rm -rf ${outputDir}`;
  await $`mkdir -p ${denoDir} ${scopeDir}`;

  // setup Deno packages
  {
    $.logStep(`Setting up deno ${version}...`);
    const pkgJson = {
      "name": "deno",
      "version": version,
      "description": "A modern runtime for JavaScript and TypeScript.",
      "bin": "bin.cjs",
      "repository": {
        "type": "git",
        "url": "git+https://github.com/denoland/deno.git",
      },
      "keywords": [
        "runtime",
        "typescript",
      ],
      "author": "the Deno authors",
      "license": "MIT",
      "bugs": {
        "url": "https://github.com/denoland/deno/issues",
      },
      "homepage": "https://deno.com",
      // for yarn berry (https://github.com/dprint/dprint/issues/686)
      "preferUnplugged": true,
      "scripts": {
        "postinstall": "node ./install.cjs",
      },
      optionalDependencies: packages
        .map((pkg) => `@deno/${getPackageNameNoScope(pkg)}`)
        .reduce((obj, pkgName) => ({ ...obj, [pkgName]: version }), {}),
    };
    currentDir.join("bin.cjs").copyFileToDirSync(denoDir);
    currentDir.join("install_api.cjs").copyFileToDirSync(denoDir);
    currentDir.join("install.cjs").copyFileToDirSync(denoDir);
    denoDir.join("package.json").writeJsonPrettySync(pkgJson);
    rootDir.join("LICENSE.md").copyFileSync(denoDir.join("LICENSE"));
    denoDir.join("README.md").writeTextSync(markdownText);
    // ensure the test files don't get published
    denoDir.join(".npmignore").writeTextSync("deno\ndeno.exe\n");

    // setup each binary package
    for (const pkg of packages) {
      const pkgName = getPackageNameNoScope(pkg);
      $.logStep(`Setting up @deno/${pkgName}...`);
      const pkgDir = scopeDir.join(pkgName);
      const zipPath = pkgDir.join("output.zip");

      await $`mkdir -p ${pkgDir}`;

      // download and extract the zip file
      const zipUrl =
        `https://github.com/denoland/deno/releases/download/v${version}/${pkg.zipFileName}`;
      await $.request(zipUrl).showProgress().pipeToPath(zipPath);
      await decompress(zipPath.toString(), pkgDir.toString());
      zipPath.removeSync();

      // create the package.json and readme
      pkgDir.join("README.md").writeTextSync(
        `# @denoland/${pkgName}\n\n${pkgName} distribution of [Deno](https://deno.land).\n`,
      );
      pkgDir.join("package.json").writeJsonPrettySync({
        "name": `@deno/${pkgName}`,
        "version": version,
        "description": `${pkgName} distribution of Deno`,
        "repository": {
          "type": "git",
          "url": "git+https://github.com/denoland/deno.git",
        },
        // force yarn to unpack
        "preferUnplugged": true,
        "author": "David Sherret",
        "license": "MIT",
        "bugs": {
          "url": "https://github.com/denoland/deno/issues",
        },
        "homepage": "https://deno.land",
        "os": [pkg.os],
        "cpu": [pkg.cpu],
        libc: pkg.libc == null ? undefined : [pkg.libc],
      });
    }
  }
}

// publish if necessary
if (args.publish || args["publish-only"]) {
  for (const pkg of packages) {
    const pkgName = getPackageNameNoScope(pkg);
    $.logStep(`Publishing @deno/${pkgName}...`);
    if (await checkPackagePublished(`@deno/${pkgName}`)) {
      $.logLight("  Already published.");
      continue;
    }
    const pkgDir = scopeDir.join(pkgName);
    // ensure the binary is executable in the tarball
    if (pkg.os !== "win32") {
      await $`chmod +x ${pkgDir.join("deno")}`;
    }
    await $`cd ${pkgDir} && npm publish --provenance --access public`;
  }

  $.logStep(`Publishing deno...`);
  await $`cd ${denoDir} && npm publish --provenance --access public`;
}

function getPackageNameNoScope(name: Package) {
  const libc = name.libc == null ? "" : `-${name.libc}`;
  return `${name.os}-${name.cpu}${libc}`;
}

function resolveVersion() {
  const firstArg = args._[0];
  if (
    firstArg != null &&
    typeof firstArg === "string" &&
    firstArg.trim().length > 0
  ) {
    return firstArg;
  }
  const version = (rootDir.join("cli/Cargo.toml").readTextSync().match(
    /version = "(.*?)"/,
  ))?.[1];
  if (version == null) {
    throw new Error("Could not resolve version.");
  }
  return version;
}

async function checkPackagePublished(pkgName: string) {
  const result = await $`npm info ${pkgName}@${version}`.quiet().noThrow();
  return result.code === 0;
}
