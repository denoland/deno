#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --allow-run
// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

// This file is used to transform Node.js internal streams code to
// Deno polyfills.
//
// Run this script with `--upgrade` to upgrade the streams code. This will update
// the code to the Node.js version specified in `tests/node_compat/runner/suite/node_version.ts`.
//
// This script applies the following transformations:
//   a. Rewrite CJS-style internal Node.js modules to ESM for Deno.
//   b. Remap internal Node.js modules to Deno equivalents.

// @ts-types="npm:@types/jscodeshift"
import jscodeshift from "npm:jscodeshift@0.15.2";
import type {
  AssignmentExpression,
  ASTPath,
  ExportSpecifier,
  FileInfo,
  Identifier,
  ImportDeclaration,
  JSCodeshift,
  ObjectExpression,
  Property,
} from "npm:jscodeshift@0.15.2";
import $ from "jsr:@david/dax@0.42.0";

import path from "node:path";

import { version } from "../../tests/node_compat/runner/suite/node_version.ts";
import { expandGlobSync } from "jsr:@std/fs@1.0.14/expand-glob";

const globs = [
  "internal/streams/*.js",
  "stream/*.js",
];

// These have special handling for lazy loading
const ignore = ["duplexify.js"];

const moduleMap: Record<string, string> = {
  "events": "node:events",
  "buffer": "node:buffer",
  "stream": "node:stream",
  "string_decoder": "node:string_decoder",
  "internal/abort_controller": "ext:deno_web/03_abort_signal.js",
  "internal/events/abort_listener":
    "ext:deno_node/internal/events/abort_listener.mjs",
  "internal/assert": "ext:deno_node/internal/assert.mjs",
  "internal/webstreams/adapters":
    "ext:deno_node/internal/webstreams/adapters.js",
  "internal/webstreams/compression": "ext:deno_web/14_compression.js",
  "internal/webstreams/encoding": "ext:deno_web/08_text_encoding.js",
  "internal/errors": "ext:deno_node/internal/errors.ts",
  "internal/event_target": "ext:deno_node/internal/event_target.mjs",
  "internal/util": "ext:deno_node/internal/util.mjs",
  "internal/util/debuglog": "ext:deno_node/internal/util/debuglog.ts",
  "internal/validators": "ext:deno_node/internal/validators.mjs",
  "internal/encoding": "ext:deno_web/08_text_encoding.js",
  "internal/blob": "ext:deno_web/09_file.js",
};

// Use default export for these conditional require()
const defaultLazy = [
  "internal/streams/passthrough",
  "internal/streams/readable",
  "internal/streams/duplexify",
];

// Workaround a bug in our formatter: "export default from;" does not work
// correctly, so we rename it to something else and export.
//
// https://github.com/dprint/dprint-plugin-typescript/issues/705
const renameForDefaultExport = ["from"];

const mapping = (source: string): string => {
  if (source.startsWith("internal/webstreams")) {
    return `ext:deno_web/06_streams.js`;
  }
  if (source.startsWith("internal/")) {
    return `ext:deno_node/${source}.js`;
  }
  return source;
};

const getSource = (source: string): string =>
  moduleMap[source] || mapping(source);

function createDefaultAndNamedExport(
  j: JSCodeshift,
  expr: ObjectExpression,
  getUniqueImportId: (id?: string) => Identifier,
) {
  const props = expr.properties;

  const specifiers: ExportSpecifier[] = props
    .filter((prop) => j.Property.check(prop) && j.Identifier.check(prop.value))
    .map((p) => {
      const prop = p as Property;
      return j.exportSpecifier.from({
        exported: j.identifier((prop.value as Identifier).name),
        local: j.identifier((prop.key as Identifier).name),
      });
    });

  const tmpId = getUniqueImportId("_defaultExport");

  const tmpDecl = j.variableDeclaration("const", [
    j.variableDeclarator(tmpId, expr),
  ]);

  const defaultExport = j.exportDefaultDeclaration(tmpId);
  const namedExport = j.exportNamedDeclaration(null, specifiers);

  return { tmpDecl, defaultExport, namedExport };
}

const topLevel = (path: ASTPath) => path.parent.node.type === "Program";

const transform = (file: FileInfo, j: JSCodeshift) => {
  const root = j(file.source);

  let uidCounter = 1;
  function getUniqueImportId(base = "imported") {
    const used = new Set(Object.keys(root.getVariableDeclarators(() => true)));
    let name;
    do {
      name = `${base}${uidCounter++}`;
    } while (used.has(name));
    return j.identifier(name);
  }

  const requireDecls: ImportDeclaration[] = [];
  const destructurings: { index: number; node: any }[] = [];
  const toRemove: ASTPath[] = [];
  let insertedPrimordialsImport = false;
  let insertedProcessImport = false;
  let hasDefaultExport = false;

  // If "process" is used, add import
  root.find(j.Identifier)
    .filter((path) => path.node.name === "process")
    .forEach(() => {
      if (!insertedProcessImport) {
        const processImport = j.importDeclaration(
          [j.importDefaultSpecifier(j.identifier("process"))],
          j.literal("node:process"),
        );
        requireDecls.push(processImport);
        insertedProcessImport = true;
      }
    });

  root.find(j.VariableDeclaration)
    .forEach((path) => {
      path.node.declarations.forEach((decl) => {
        if (decl.type !== "VariableDeclarator") return;

        if (
          j.ObjectPattern.check(decl.id) &&
          j.Identifier.check(
            decl.init,
            (s) => "name" in s && s.name == "primordials",
          )
        ) {
          // Insert import if it hasn’t been added yet
          if (!insertedPrimordialsImport) {
            const primordialsImport = j.importDeclaration(
              [j.importSpecifier(j.identifier("primordials"))],
              j.literal("ext:core/mod.js"),
            );
            requireDecls.push(primordialsImport);
            insertedPrimordialsImport = true;
          }
        }

        if (
          j.CallExpression.check(decl.init) &&
          (decl.init.callee as Identifier)?.name === "require" &&
          decl.init.arguments.length === 1 &&
          j.Literal.check(decl.init.arguments[0])
        ) {
          const callee = decl.init.callee as Identifier;
          // Make sure that name is "require"
          if (callee.name !== "require") {
            throw new Error(
              'Expected "require" as the callee name. Found: ' +
                callee.name,
            );
          }

          const source = decl.init.arguments[0].value as string;
          const id = decl.id;

          if (j.Identifier.check(id)) {
            // const foo = require('bar')
            const importDecl = j.importDeclaration(
              [j.importDefaultSpecifier(j.identifier(id.name))],
              j.literal(getSource(source)),
            );
            requireDecls.push(importDecl);
            toRemove.push(path);
          } else if (j.ObjectPattern.check(id)) {
            const isFlat = id.properties.every(
              (p) => j.Property.check(p) && j.Identifier.check(p.value),
            );

            if (isFlat) {
              // const { x, y } = require('bar')
              const importDecl = j.importDeclaration(
                id.properties.map((p) => {
                  const prop = p as Property;
                  return j.importSpecifier(
                    j.identifier((prop.key as Identifier).name),
                    j.identifier((prop.value as Identifier).name),
                  );
                }),
                j.literal(getSource(source)),
              );
              requireDecls.push(importDecl);
              toRemove.push(path);
            } else {
              // const { o: { a } } = require('baz') → import tmp from 'baz'; const { o: { a } } = tmp;
              const importId = getUniqueImportId();
              const importDecl = j.importDeclaration(
                [j.importDefaultSpecifier(importId)],
                j.literal(getSource(source)),
              );
              requireDecls.push(importDecl);

              const replacementDecl = j.variableDeclaration(path.node.kind, [
                j.variableDeclarator(id, importId),
              ]);
              destructurings.push({ index: path.name, node: replacementDecl });

              toRemove.push(path);
            }
          }
        } else if (
          j.MemberExpression.check(decl.init) &&
          j.CallExpression.check(decl.init.object) &&
          (decl.init.object.callee as Identifier)?.name === "require" &&
          decl.init.object.arguments.length === 1 &&
          j.Literal.check(decl.init.object.arguments[0])
        ) {
          // Example: require('internal/errors').codes
          const source = decl.init.object.arguments[0].value as string;
          const accessedProp = (decl.init.property as Identifier).name;
          const importId = getUniqueImportId("_mod"); // e.g., _mod1

          const importDecl = j.importDeclaration(
            [j.importDefaultSpecifier(importId)],
            j.literal(getSource(source)),
          );
          requireDecls.push(importDecl);

          // Reassign: const { ... } = _mod.codes
          const newInit = j.memberExpression(
            importId,
            j.identifier(accessedProp),
          );
          const replacementDecl = j.variableDeclaration(path.node.kind, [
            j.variableDeclarator(decl.id, newInit),
          ]);

          destructurings.push({ index: path.name, node: replacementDecl });
          toRemove.push(path);
        }
      });
    });

  const inlineRequires = new Map(); // module name → imported identifier

  // Replace module.exports = { x, y } with export { x, y }
  const namedExportAssignments: string[] = [];

  const pushEnd = (n: any) => root.get().node.program.body.push(n);

  root.find(j.ExpressionStatement)
    .filter(topLevel)
    .filter((path) => {
      const expr = path.node.expression;
      return (
        j.AssignmentExpression.check(expr) &&
        j.MemberExpression.check(expr.left) &&
        (expr.left.object as Identifier).name === "module" &&
        (expr.left.property as Identifier).name === "exports"
      );
    })
    .forEach((path) => {
      const expr = path.node.expression as AssignmentExpression;

      if (j.ObjectExpression.check(expr.right)) {
        const { tmpDecl, defaultExport, namedExport } =
          createDefaultAndNamedExport(j, expr.right, getUniqueImportId);
        j(path).insertBefore(tmpDecl);
        j(path).insertAfter(namedExport);
        j(path).insertAfter(defaultExport);
        hasDefaultExport = true;

        j(path).remove();
      } else if (j.Identifier.check(expr.right)) {
        // module.exports = Foo;
        const id = expr.right;
        if (!hasDefaultExport) {
          let name = id.name;
          if (renameForDefaultExport.includes(name)) {
            name = "_defaultExport";
            // Assign to a new variable
            const decl = j.variableDeclaration("const", [
              j.variableDeclarator(j.identifier(name), id),
            ]);
            pushEnd(decl);
          }
          const exportDefault = j.exportDefaultDeclaration(
            j.identifier(name),
          );
          pushEnd(exportDefault);
          hasDefaultExport = true;
        }

        const exportNamed = j.exportNamedDeclaration(
          null,
          [j.exportSpecifier.from({
            exported: j.identifier(id.name),
            local: j.identifier(id.name),
          })],
        );
        pushEnd(exportNamed);
        j(path).remove();
      } else {
        // module.exports = () => ... or {}
        const exportDefault = j.exportDefaultDeclaration(expr.right);
        j(path).replaceWith(exportDefault);
      }
    });

  // Handle module.exports.X = ...
  root.find(j.ExpressionStatement)
    .filter(topLevel)
    .filter((path) => {
      const expr = path.node.expression;
      return (
        j.AssignmentExpression.check(expr) &&
        j.MemberExpression.check(expr.left) &&
        j.MemberExpression.check(expr.left.object) &&
        (expr.left.object.object as Identifier).name == "module" &&
        (expr.left.object.property as Identifier).name == "exports"
      );
    })
    .forEach((path) => {
      const expr = path.node.expression as AssignmentExpression;
      const exportName = "property" in expr.left && "name" in expr.left.property
        ? expr.left.property.name
        : null;
      if (typeof exportName !== "string") {
        return;
      }

      const right = expr.right;
      namedExportAssignments.push(exportName);

      if (
        j.Identifier.check(right) &&
        right.name === exportName
      ) {
        // Just export the existing binding
        const exportStmt = j.exportNamedDeclaration(null, [
          j.exportSpecifier.from({
            local: j.identifier(exportName),
            exported: j.identifier(exportName),
          }),
        ]);
        j(path).replaceWith(exportStmt);
      } else {
        // Define new const and export it
        const decl = j.variableDeclaration("const", [
          j.variableDeclarator(j.identifier(exportName), right),
        ]);

        const exportStmt = j.exportNamedDeclaration(null, [
          j.exportSpecifier.from({
            local: j.identifier(exportName),
            exported: j.identifier(exportName),
          }),
        ]);

        j(path).insertBefore(decl);
        j(path).insertAfter(exportStmt);
        j(path).remove();
      }
    });

  // Remove original require declarations
  toRemove.forEach((path) => j(path).remove());

  // Remove `module.exports` from `module.exports.call()`
  root.find(j.MemberExpression)
    .filter((path) => {
      const { node } = path;
      return (
        j.Identifier.check(node.object) &&
        node.object.name === "module" &&
        j.Identifier.check(node.property) &&
        node.property.name === "exports"
      );
    })
    .forEach((path) => {
      const nextProp = path.parentPath.node.property;
      if (j.Identifier.check(nextProp)) {
        j(path.parentPath).replaceWith(nextProp);
      }
    });

  root.find(j.CallExpression)
    .forEach((path) => {
      const { node } = path;

      // Remaining dynamic require('foo')
      if (
        j.Identifier.check(node.callee) &&
        node.callee.name === "require" &&
        node.arguments.length === 1 &&
        j.Literal.check(node.arguments[0])
      ) {
        const source = node.arguments[0].value as string;

        let importId;
        if (inlineRequires.has(source)) {
          importId = inlineRequires.get(source);
        } else {
          importId = getUniqueImportId("_mod");
          inlineRequires.set(source, importId);

          const importDecl = j.importDeclaration(
            [
              defaultLazy.includes(source)
                ? j.importDefaultSpecifier(importId)
                : j.importNamespaceSpecifier(importId),
            ],
            j.literal(getSource(source)),
          );
          requireDecls.push(importDecl);
        }

        j(path).replaceWith(importId);
      }
    });

  // Insert import declarations at the top
  if (requireDecls.length > 0) {
    const program = root.get().node.program;
    program.body = [...requireDecls, ...program.body];
  }

  // Insert destructuring replacements below imports
  if (destructurings.length > 0) {
    destructurings.forEach(({ node }) => {
      root.get().node.program.body.splice(requireDecls.length, 0, node);
    });
  }
  if (!hasDefaultExport && namedExportAssignments.length > 0) {
    const defaultExportObject = j.objectExpression(
      namedExportAssignments.map((name) =>
        j.objectProperty.from({
          key: j.identifier(name),
          value: j.identifier(name),
          shorthand: true,
        })
      ),
    );

    const exportDefault = j.exportDefaultDeclaration(defaultExportObject);
    root.get().node.program.body.push(exportDefault);
    hasDefaultExport = true;
  }

  const prelude =
    "// deno-lint-ignore-file\n// Copyright 2018-2025 the Deno authors. MIT license.\n\n";
  return prelude + root.toSource({ quote: "single" });
};

const upgrade = Deno.args.includes("--upgrade");

// Don't run if git status is dirty
const status = (await $`git status --porcelain`.text()).trim();
if (status) {
  console.error("Git status is dirty. Please commit or stash your changes.");
  Deno.exit(1);
}

const tag = "v" + version;
if (upgrade) {
  await $`rm -rf node`;
  await $`git clone --depth 1 --sparse --branch ${tag} --single-branch https://github.com/nodejs/node.git`;
  await $`git sparse-checkout add lib`.cwd("node");
}

const fromLib = new URL("./node/lib", import.meta.url).pathname;
const toLib = new URL("./polyfills", import.meta.url).pathname;
const root = new URL("../../", import.meta.url).pathname;

for (const glob of globs) {
  const sourcePath = path.join(fromLib, glob);
  const expand = expandGlobSync(sourcePath);

  for (const entry of expand) {
    if (ignore.includes(entry.name)) {
      console.log(`Ignoring ${entry.name}`);
      continue;
    }

    const sourcePath = entry.path;

    const code = await Deno.readTextFile(sourcePath);
    const output = transform({ path: sourcePath, source: code }, jscodeshift);

    const relativePath = path.relative(fromLib, sourcePath);
    const targetPath = path.join(toLib, relativePath);
    const targetDir = path.dirname(targetPath);
    await Deno.mkdir(targetDir, { recursive: true });
    await Deno.writeTextFile(targetPath, output);
    console.log(`${sourcePath} -> ${targetPath}`);
  }
}

await $`rm -rf node`;
await $`./tools/format.js`.cwd(root);
