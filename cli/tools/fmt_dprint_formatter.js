// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console -- this worker reports formatting status.

// Worker module for the in-process WASM backend of `deno fmt` (see
// cli/tools/fmt_dprint.rs). It is loaded as an ES module into an embedded
// MainWorker - the same process as `deno fmt`, NOT a `deno run` subprocess -
// and formats batches of files using the dprint WASM plugins loaded through
// `@dprint/formatter`. The plugins run on the embedded runtime's V8
// WebAssembly, loaded once, so there is no per-file process spawn.
//
// Rust calls the exported `run(jobsJson)` function and awaits its result. Each
// job has the shape:
//
//   {
//     "check": boolean,        // check instead of write
//     "quiet": boolean,        // suppress non-error output
//     "global": { ... },       // dprint global config (lineWidth, ...)
//     "plugins": { name: cfg },// per-plugin config keyed by plugin name
//     "files": [ "abs/path" ]  // absolute file paths to format
//   }

import { createContext } from "npm:@dprint/formatter@0.5.1";
import * as pluginTypescript from "npm:@dprint/typescript@0.96.1";
import * as pluginJson from "npm:@dprint/json@0.21.3";
import * as pluginMarkdown from "npm:@dprint/markdown@0.22.1";
import * as pluginToml from "npm:@dprint/toml@0.7.0";
import * as pluginSql from "npm:@dprint/sql@0.3.0";
import * as pluginJupyter from "npm:@dprint/jupyter@0.2.3";

// Plugins distributed as npm packages exposing the WASM bytes. These are
// vendorable, so they work in a hermetic (offline) environment once cached.
const NPM_PLUGINS = {
  typescript: pluginTypescript,
  json: pluginJson,
  markdown: pluginMarkdown,
  toml: pluginToml,
  sql: pluginSql,
  jupyter: pluginJupyter,
};

// Plugins not published as dprint npm packages, loaded from plugins.dprint.dev.
// TODO(prototype): vendor these so the WASM backend is fully offline.
const URL_PLUGINS = {
  malva: "https://plugins.dprint.dev/g-plane/malva-v0.16.0.wasm",
  markup: "https://plugins.dprint.dev/g-plane/markup_fmt-v0.27.2.wasm",
  yaml: "https://plugins.dprint.dev/g-plane/pretty_yaml-v0.6.0.wasm",
};

function loadNpmBytes(mod) {
  // Most @dprint/* packages expose getPath() to the WASM file; some (e.g.
  // @dprint/sql) expose getBuffer() returning the bytes directly.
  return typeof mod.getBuffer === "function"
    ? mod.getBuffer()
    : Deno.readFileSync(mod.getPath());
}

async function createFormatterContext(job) {
  const pluginConfig = job.plugins ?? {};
  const ctx = createContext(job.global ?? {});
  for (const [name, mod] of Object.entries(NPM_PLUGINS)) {
    ctx.addPlugin(loadNpmBytes(mod), pluginConfig[name] ?? {});
  }
  for (const [name, url] of Object.entries(URL_PLUGINS)) {
    const response = await fetch(url);
    await ctx.addPluginStreaming(response, pluginConfig[name] ?? {});
  }
  return ctx;
}

function formatBatch(ctx, job) {
  let notFormatted = 0;
  let formattedCount = 0;
  let errored = false;
  for (const filePath of job.files) {
    const fileText = Deno.readTextFileSync(filePath);
    let output;
    try {
      output = ctx.formatText({ filePath, fileText });
    } catch (err) {
      console.error(`Error formatting ${filePath}: ${err?.message ?? err}`);
      errored = true;
      continue;
    }
    if (output === fileText) {
      continue;
    }
    if (job.check) {
      notFormatted++;
      if (!job.quiet) {
        console.error(`Not formatted: ${filePath}`);
      }
    } else {
      Deno.writeTextFileSync(filePath, output);
      formattedCount++;
    }
  }

  if (job.check) {
    if (notFormatted > 0) {
      errored = true;
      if (!job.quiet) {
        console.error(`error: Found ${notFormatted} not formatted file(s)`);
      }
    }
  } else if (formattedCount > 0 && !job.quiet) {
    console.error(`Formatted ${formattedCount} file(s)`);
  }
  return errored;
}

// Entry point invoked from Rust. Returns `{ failed }`.
export async function run(jobsJson) {
  const jobs = JSON.parse(jobsJson);
  let failed = false;
  for (const job of jobs) {
    const ctx = await createFormatterContext(job);
    if (formatBatch(ctx, job)) {
      failed = true;
    }
  }
  return { failed };
}
