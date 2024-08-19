#!/usr/bin/env -S deno run --allow-read --allow-env --allow-sys --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { Node, Project, ts } from "npm:ts-morph@22.0.0";
import { join, ROOT_PATH } from "./util.js";

const libs = [
  join(ROOT_PATH, "ext/cache/lib.deno_cache.d.ts"),
  join(ROOT_PATH, "ext/console/lib.deno_console.d.ts"),
  join(ROOT_PATH, "ext/url/lib.deno_url.d.ts"),
  join(ROOT_PATH, "ext/web/lib.deno_web.d.ts"),
  join(ROOT_PATH, "ext/fetch/lib.deno_fetch.d.ts"),
  join(ROOT_PATH, "ext/websocket/lib.deno_websocket.d.ts"),
  join(ROOT_PATH, "ext/webstorage/lib.deno_webstorage.d.ts"),
  join(ROOT_PATH, "ext/canvas/lib.deno_canvas.d.ts"),
  join(ROOT_PATH, "ext/crypto/lib.deno_crypto.d.ts"),
  join(ROOT_PATH, "ext/net/lib.deno_net.d.ts"),
  join(ROOT_PATH, "cli/tsc/dts/lib.deno.ns.d.ts"),
  join(ROOT_PATH, "cli/tsc/dts/lib.deno.shared_globals.d.ts"),
  join(ROOT_PATH, "cli/tsc/dts/lib.deno.window.d.ts"),
];

const unstableLibs = [
  join(ROOT_PATH, "ext/broadcast_channel/lib.deno_broadcast_channel.d.ts"),
  join(ROOT_PATH, "cli/tsc/dts/lib.deno_webgpu.d.ts"),
  join(ROOT_PATH, "cli/tsc/dts/lib.deno.unstable.d.ts"),
];

const errors = [];

const project = new Project();
project.addSourceFilesAtPaths(libs);
const unstableFiles = project.addSourceFilesAtPaths(unstableLibs);

for (const file of project.getSourceFiles()) {
  for (
    const node of file.getDescendants().filter((descendant) =>
      Node.isExportable(descendant)
    )
  ) {
    if (
      node.getKind() === ts.SyntaxKind.ModuleDeclaration &&
      node.getName() === "Deno"
    ) {
      continue;
    }

    const parent = node.getFirstAncestorByKind(ts.SyntaxKind.ModuleDeclaration);

    if (parent) {
      if (!node.isExported()) {
        errors.push(getErrorPrefix(node) + "export keyword");
        continue;
      }
    } else if (!node.hasDeclareKeyword()) {
      errors.push(getErrorPrefix(node) + "declare keyword");
      continue;
    }

    const jsDoc = node.getFirstChildIfKind(ts.SyntaxKind.JSDoc);
    if (!jsDoc) {
      errors.push(getErrorPrefix(node) + "JSDoc comment");
      continue;
    }

    const tags = jsDoc.getTags();

    if (!tags.find((tag) => tag.getTagName() === "category")) {
      errors.push(getErrorPrefix(node) + "JSDoc @category tag");
      continue;
    }

    if (unstableFiles.includes(file)) {
      if (!tags.find((tag) => tag.getTagName() === "experimental")) {
        errors.push(getErrorPrefix(node) + "JSDoc @experimental tag");
      }
    }
  }
}

if (errors.length > 0) {
  throw new AggregateError(errors);
}

function getErrorPrefix(node) {
  return `Symbol at file://${node.getSourceFile().getFilePath()}:${node.getStartLineNumber()} is missing a `;
}
