#!/usr/bin/env -S deno run -A
// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import $, { Path } from "jsr:@david/dax@^0.42.0";
import * as semver from "@std/semver";
import { UntarStream } from "@std/tar/untar-stream";
import {
  InterfaceDeclaration,
  ModuleDeclarationKind,
  ModuleKind,
  ModuleResolutionKind,
  Node,
  Project,
  ScriptTarget,
  SyntaxKind,
  VariableDeclaration,
} from "jsr:@ts-morph/ts-morph@27.0.2";

const typesNodeVersion = "24.2.0";
const rootDir = $.path(import.meta.url).parentOrThrow().parentOrThrow();
const dtsDir = rootDir.join("cli/tsc/dts/");
const nodeTypesDir = dtsDir.join("node");
const undiciTypesDir = nodeTypesDir.join("undici");

await downloadAndExtractPackages();
modifySourceFiles();

async function downloadAndExtractPackages() {
  $.logStep("Downloading @types/node packument");
  const typesNodeInfo = await $.request(
    "https://registry.npmjs.org/@types/node",
  )
    .json();
  const typesNodeVersionInfo = typesNodeInfo.versions[typesNodeVersion];

  if (typesNodeVersionInfo == null) {
    throw new Error("Could not find types node version info.");
  }

  if (Object.keys(typesNodeVersionInfo.dependencies).length !== 1) {
    console.error("Dependencies", typesNodeVersionInfo.dependencies);
    throw new Error("Expected only one @types/node dep");
  }

  const undiciTypesVersionConstraint = semver.parseRange(
    typesNodeVersionInfo.dependencies["undici-types"],
  );
  if (undiciTypesVersionConstraint == null) {
    console.error("Dependencies", typesNodeVersionInfo.dependencies);
    throw new Error("Failed to find undici-types version constraint.");
  }

  $.logStep("Downloading undici-types packument");
  const undiciTypesInfo = await $.request(
    "https://registry.npmjs.org/undici-types",
  ).json();
  const undiciTypesVersion = semver.format(
    semver.maxSatisfying(
      Object.keys(undiciTypesInfo.versions).map((v) => semver.parse(v)),
      undiciTypesVersionConstraint,
    )!,
  );
  const undiciTypesVersionInfo = undiciTypesInfo.versions[undiciTypesVersion];
  if (undiciTypesVersionInfo == null) {
    throw new Error("Could not find undici types version info.");
  }

  if (
    undiciTypesVersionInfo.dependencies != null &&
    Object.keys(undiciTypesVersionInfo.dependencies).length !== null
  ) {
    console.error(undiciTypesVersionInfo.dependencies);
    throw new Error("Expected no deps for undici-types.");
  }

  await extractTarball(typesNodeVersionInfo.dist.tarball, nodeTypesDir);
  await extractTarball(undiciTypesVersionInfo.dist.tarball, undiciTypesDir);

  nodeTypesDir.join("README.md").writeTextSync(
    `# Built-in Node Type Declarations

Generate this folder by running \`./tools/update_types_node.ts\`.
`,
  );
}

async function extractTarball(url: string, destination: Path) {
  $.logStep("Downloading", url, "to", destination.toString());
  destination.ensureDirSync();
  destination.emptyDirSync();
  const tarballBlob = await $.request(url).blob();
  for await (
    const entry of tarballBlob.stream()
      .pipeThrough(new DecompressionStream("gzip"))
      .pipeThrough(new UntarStream())
  ) {
    if (
      entry.readable == null ||
      entry.path.endsWith("README.md") ||
      entry.path.endsWith("package.json") ||
      entry.path.includes("ts5.1") ||
      entry.path.includes("ts5.6") ||
      entry.path.includes("ts5.7")
    ) {
      if (entry.readable) {
        await entry.readable.cancel();
      }
      continue;
    }
    const adjustedPath = entry.path
      .replace(/^node\//, "")
      .replace(/^package\//, "");
    const path = destination.join(adjustedPath).resolve();
    path.parentOrThrow().mkdirSync({ recursive: true });
    using file = path.createSync();
    await entry.readable.pipeTo(file.writable);
  }
}

function modifySourceFiles() {
  const project = new Project({
    compilerOptions: {
      target: ScriptTarget.ESNext,
      module: ModuleKind.ESNext,
      moduleResolution: ModuleResolutionKind.Bundler,
    },
  });
  project.addSourceFilesAtPaths(nodeTypesDir.join("**/*.d.ts").toString());
  project.addSourceFilesAtPaths(nodeTypesDir.join("**/*.d.cts").toString());
  const undiciTypesSourceFile = project.getSourceFileOrThrow(
    undiciTypesDir.join("index.d.ts").toString(),
  );
  const typesNodeSourceFile = project.getSourceFile(
    nodeTypesDir.join("index.d.ts").toString(),
  ) ?? project.getSourceFileOrThrow(
    nodeTypesDir.join("index.d.cts").toString(),
  );

  for (const statement of typesNodeSourceFile.getStatementsWithComments()) {
    if (Node.isCommentStatement(statement)) {
      const text = statement.getText();
      if (text.includes("/// <reference path=") && text.includes(".d.ts")) {
        statement.replaceWithText(
          text.replace('path="', 'path="./')
            .replace('.d.ts"', '.d.cts"'),
        );
      }
    }
  }

  for (const sourceFile of project.getSourceFiles()) {
    const updateModuleSpecifier = (
      moduleSpecifier: string | undefined,
      setSpecifier: (value: string) => void,
    ) => {
      if (moduleSpecifier == null) {
        return;
      }
      if (moduleSpecifier.startsWith("./")) {
        if (!moduleSpecifier.endsWith(".d.ts")) {
          setSpecifier(moduleSpecifier + ".d.ts");
        }
        return;
      }
      if (moduleSpecifier === "undici-types") {
        const relativeSpecifier = sourceFile.getRelativePathAsModuleSpecifierTo(
          undiciTypesSourceFile,
        ) + ".d.ts";
        return setSpecifier(relativeSpecifier);
      }

      if (isKnownModuleSpecifier(moduleSpecifier)) {
        return;
      }

      $.logWarn(
        "WARN",
        "Encountered unknown module specifier:",
        moduleSpecifier,
      );
    };

    // Get all import declarations
    for (const importDecl of sourceFile.getImportDeclarations()) {
      updateModuleSpecifier(
        importDecl.getModuleSpecifierValue(),
        (value) => importDecl.setModuleSpecifier(value),
      );
    }

    // Get all export declarations with module specifiers
    for (const exportDecl of sourceFile.getExportDeclarations()) {
      updateModuleSpecifier(
        exportDecl.getModuleSpecifierValue(),
        (value) => exportDecl.setModuleSpecifier(value),
      );
    }

    // Get all import type queries (e.g., import("module").Type)
    for (
      const importType of sourceFile.getDescendantsOfKind(SyntaxKind.ImportType)
    ) {
      const argument = importType.getArgument();
      if (argument && argument.isKind(SyntaxKind.LiteralType)) {
        const literal = argument.getLiteral();
        if (literal.isKind(SyntaxKind.StringLiteral)) {
          updateModuleSpecifier(
            literal.getLiteralValue(),
            (value) => literal.setLiteralValue(value),
          );
        }
      }
    }

    // go over every `global` declaration and output the text
    for (
      const moduleDecl of sourceFile.getDescendantsOfKind(
        SyntaxKind.ModuleDeclaration,
      )
    ) {
      if (moduleDecl.getDeclarationKind() !== ModuleDeclarationKind.Global) {
        continue;
      }
      const body = moduleDecl.getBody();
      if (body == null) {
        continue;
      }
      // remove any globals that conflict with our globals
      for (const statement of body.getStatements()) {
        if (Node.isVariableStatement(statement)) {
          for (const decl of statement.getDeclarations()) {
            handleVarDecl(decl);
          }
        } else if (Node.isInterfaceDeclaration(statement)) {
          handleInterface(statement);
        }
      }
      if (moduleDecl.getStatements().length === 0) {
        moduleDecl.remove();
      }
    }

    if (
      !sourceFile.getFilePath().includes("undici") &&
      sourceFile.getFilePath().endsWith(".d.ts")
    ) {
      sourceFile.move(
        sourceFile.getFilePath().replace(/.d.ts$/, ".d.cts"),
      );
    }

    if (sourceFile.getFullText().includes('/// <reference types="node" />\n')) {
      sourceFile.replaceText(
        [0, sourceFile.getEnd()],
        sourceFile.getFullText().replace(
          '/// <reference types="node" />\n',
          "",
        ),
      );
    }
  }

  project.saveSync();
}

function handleInterface(decl: InterfaceDeclaration) {
  switch (decl.getName()) {
    case "ImportMeta": {
      // make these align with our ImportMeta, which is optional
      decl.getPropertyOrThrow("dirname").setHasQuestionToken(true);
      decl.getPropertyOrThrow("filename").setHasQuestionToken(true);
      break;
    }
    case "Blob":
    case "ByteLengthQueuingStrategy":
    case "CompressionStream":
    case "CountQueuingStrategy":
    case "CustomEvent":
    case "DecompressionStream":
    case "Event":
    case "EventTarget":
    case "File":
    case "ReadableByteStreamController":
    case "ReadableStream":
    case "ReadableStreamBYOBReader":
    case "ReadableStreamBYOBRequest":
    case "ReadableStreamDefaultController":
    case "ReadableStreamDefaultReader":
    case "TextDecoderStream":
    case "TextEncoderStream":
    case "TransformStream":
    case "TransformStreamDefaultController":
    case "WritableStream":
    case "WritableStreamDefaultController":
    case "WritableStreamDefaultWriter":
      decl.remove();
      break;
    default:
      console.log(decl.getName());
      break;
  }
}

function handleVarDecl(decl: VariableDeclaration) {
  switch (decl.getName()) {
    case "AbortController":
    case "AbortSignal":
    case "Blob":
    case "BroadcastChannel":
    case "ByteLengthQueuingStrategy":
    case "CloseEvent":
    case "CompressionStream":
    case "CountQueuingStrategy":
    case "crypto":
    case "CustomEvent":
    case "DecompressionStream":
    case "DOMException":
    case "Event":
    case "EventSource":
    case "EventTarget":
    case "File":
    case "FormData":
    case "Headers":
    case "MessageChannel":
    case "MessageEvent":
    case "MessagePort":
    case "performance":
    case "PerformanceEntry":
    case "PerformanceMark":
    case "PerformanceMeasure":
    case "ReadableByteStreamController":
    case "ReadableStream":
    case "ReadableStreamBYOBReader":
    case "ReadableStreamBYOBRequest":
    case "ReadableStreamDefaultController":
    case "ReadableStreamDefaultReader":
    case "Request":
    case "Response":
    case "Storage":
    case "TextDecoder":
    case "TextDecoderStream":
    case "TextEncoder":
    case "TextEncoderStream":
    case "TransformStream":
    case "TransformStreamDefaultController":
    case "URL":
    case "URLSearchParams":
    case "WebSocket":
    case "WritableStream":
    case "WritableStreamDefaultController":
    case "WritableStreamDefaultWriter":
      assertIsVariableWithConditionalTypeOnGlobalThis();
      decl.remove();
      break;
    case "URLPattern":
      decl.remove();
      break;
    default:
      console.log(decl.getName());
      break;
  }

  function assertIsVariableWithConditionalTypeOnGlobalThis() {
    const typeNode = decl.getTypeNode();
    // ensure we're removing what we're expecting to remove
    if (typeNode.getKind() !== SyntaxKind.ConditionalType) {
      console.error(typeNode.getText());
      throw new Error(
        "Assertion failed for type node being a conditional type.",
      );
    }
    const typeQuery = typeNode.getFirstDescendantByKindOrThrow(
      SyntaxKind.TypeQuery,
    );
    if (typeQuery.getExprName().getText() !== "globalThis") {
      console.error(typeNode.getText());
      throw new Error(
        "Assertion failed for type node containing `typeof globalThis`",
      );
    }
  }
}

function isKnownModuleSpecifier(text: string) {
  switch (text) {
    case "assert":
    case "assert/strict":
    case "async_hooks":
    case "buffer":
    case "child_process":
    case "cluster":
    case "console":
    case "constants":
    case "crypto":
    case "dgram":
    case "diagnostics_channel":
    case "dns":
    case "dns/promises":
    case "domain":
    case "events":
    case "fs":
    case "fs/promises":
    case "http":
    case "http2":
    case "https":
    case "inspector":
    case "inspector/promises":
    case "module":
    case "net":
    case "os":
    case "path":
    case "path/posix":
    case "path/win32":
    case "perf_hooks":
    case "process":
    case "punycode":
    case "querystring":
    case "readline":
    case "readline/promises":
    case "repl":
    case "stream":
    case "stream/consumers":
    case "stream/promises":
    case "stream/web":
    case "string_decoder":
    case "timers":
    case "timers/promises":
    case "tls":
    case "trace_events":
    case "tty":
    case "url":
    case "util":
    case "util/types":
    case "v8":
    case "vm":
    case "wasi":
    case "worker_threads":
    case "zlib":
      return true;
    default:
      return text.startsWith("node:");
  }
}
