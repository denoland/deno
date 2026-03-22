#!/usr/bin/env -S deno run --allow-run --allow-read --allow-write --allow-env --allow-net
/**
 * Prototype: `deno check` using stock `tsc` from npm:typescript.
 *
 * Uses ONLY tsconfig.json + node_modules — no custom CompilerHost.
 *
 * Flow:
 *   1. `deno info --json` → full module graph with local cached paths
 *   2. JSR packages → node_modules/@jsr/scope__name/ (symlinks to cache)
 *   3. npm packages → node_modules/name/ (symlinks to cache)
 *   4. @types/deno, @types/node → auto-installed into node_modules/
 *   5. Wildcard module declarations for bytes/text imports → node_modules/@deno/shims/
 *   6. Generate tsconfig.json with `paths` for jsr:/npm:/https: specifiers
 *   7. Run stock `tsc --noEmit --project tsconfig.json`
 */

import * as path from "jsr:@std/path@1";

// ── Types ───────────────────────────────────────────────────────────────────

interface DenoInfoOutput {
  roots: string[];
  modules: DenoInfoModule[];
  redirects: Record<string, string>;
  packages: Record<string, string>;
  npmPackages: Record<string, NpmPackageInfo>;
}

interface DenoInfoModule {
  kind?: string;
  specifier: string;
  local?: string;
  error?: string;
  dependencies?: Dependency[];
  npmPackage?: string;
  mediaType?: string;
}

interface Dependency {
  specifier: string;
  code?: { specifier: string };
  npmPackage?: string;
  assertionType?: string;
}

interface NpmPackageInfo {
  name: string;
  version: string;
  dependencies: string[];
  registryUrl: string;
}

// ── Helpers ─────────────────────────────────────────────────────────────────

const DENO_BIN = Deno.execPath();

const NPM_CACHE_BASE = await (async () => {
  const cmd = new Deno.Command(DENO_BIN, {
    args: ["info"],
    stdout: "piped",
    stderr: "piped",
    env: { ...Deno.env.toObject(), NO_COLOR: "1" },
  });
  const text = new TextDecoder()
    .decode((await cmd.output()).stdout)
    .replace(/\x1b\[[0-9;]*m/g, "");
  const m = text.match(/npm modules cache:\s*(.+)/);
  if (m) return m[1].trim();
  return path.join(Deno.env.get("HOME") ?? "/tmp", "Library/Caches/deno/npm");
})();

function guessExt(mediaType?: string, spec?: string): string {
  if (spec) {
    for (const e of [".ts", ".tsx", ".js", ".jsx", ".mts", ".mjs", ".d.ts", ".json"]) {
      if (spec.endsWith(e)) return "";
    }
  }
  return ({ TypeScript: ".ts", TSX: ".tsx", JavaScript: ".js", JSX: ".jsx", Dts: ".d.ts", Json: ".json" } as Record<string, string>)[mediaType ?? ""] ?? ".ts";
}

async function run(args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  const cmd = new Deno.Command(DENO_BIN, { args, stdout: "piped", stderr: "piped" });
  const { code, stdout, stderr } = await cmd.output();
  return { code, stdout: new TextDecoder().decode(stdout), stderr: new TextDecoder().decode(stderr) };
}

function resolveNpmTypes(pkgDir: string): string | undefined {
  try {
    const pkg = JSON.parse(Deno.readTextFileSync(path.join(pkgDir, "package.json")));
    for (const field of [
      pkg.types, pkg.typings,
      pkg.exports?.["."]?.types,
      pkg.exports?.["."]?.import?.types,
      pkg.exports?.["."]?.default?.types,
    ]) {
      if (field) {
        const r = path.resolve(pkgDir, field);
        try { Deno.statSync(r); return r; } catch { /* */ }
      }
    }
    for (const f of ["index.d.ts", "index.d.cts"]) {
      const p = path.join(pkgDir, f);
      try { Deno.statSync(p); return p; } catch { /* */ }
    }
    if (pkg.main) {
      const p = path.resolve(pkgDir, pkg.main.replace(/\.js$/, ".d.ts"));
      try { Deno.statSync(p); return p; } catch { /* */ }
      const js = path.resolve(pkgDir, pkg.main);
      try { Deno.statSync(js); return js; } catch { /* */ }
    }
  } catch { /* */ }
  const idx = path.join(pkgDir, "index.js");
  try { Deno.statSync(idx); return idx; } catch { return undefined; }
}

async function ensureSymlink(target: string, linkPath: string) {
  try { await Deno.lstat(linkPath); return; } catch { /* */ }
  try { await Deno.symlink(target, linkPath); } catch { /* */ }
}

// ── Module graph ────────────────────────────────────────────────────────────

async function getModuleGraph(entrypoints: string[]): Promise<DenoInfoOutput> {
  const { code, stdout, stderr } = await run(["info", "--json", ...entrypoints]);
  if (code !== 0) { console.error("deno info failed:", stderr); Deno.exit(1); }
  return JSON.parse(stdout);
}

// ── JSR package helpers ─────────────────────────────────────────────────────

/** "@std/fmt" → "@jsr/std__fmt" (the convention used by npm.jsr.io) */
function jsrPkgToNodeModulesName(jsrName: string): string {
  // @std/fmt → @jsr/std__fmt
  const m = jsrName.match(/^@([^/]+)\/(.+)$/);
  if (m) return `@jsr/${m[1]}__${m[2]}`;
  return `@jsr/${jsrName}`;
}

/**
 * Parse a jsr.io URL to extract package info.
 * "https://jsr.io/@std/fmt/1.0.9/printf.ts" → { scope: "std", name: "fmt", version: "1.0.9", file: "printf.ts" }
 */
function parseJsrUrl(specifier: string): { jsrName: string; version: string; file: string } | undefined {
  const m = specifier.match(/^https:\/\/jsr\.io\/(@[^/]+\/[^/]+)\/([^/]+)\/(.+)$/);
  if (!m) return undefined;
  return { jsrName: m[1], version: m[2], file: m[3] };
}

// ── Build mappings ──────────────────────────────────────────────────────────

interface BuildResult {
  paths: Record<string, string[]>;
  npmPkgs: Map<string, { name: string; version: string; cacheDir: string }>;
  jsrPkgs: Map<string, { jsrName: string; version: string; nodeModulesDir: string }>;
}

async function buildMappings(
  info: DenoInfoOutput,
  nodeModulesDir: string,
): Promise<BuildResult> {
  const paths: Record<string, string[]> = {};
  const npmPkgs = new Map<string, { name: string; version: string; cacheDir: string }>();
  const jsrPkgs = new Map<string, { jsrName: string; version: string; nodeModulesDir: string }>();
  const processed = new Set<string>();

  // Phase 1: Create JSR package directories in node_modules/@jsr/
  // Group all jsr.io modules by package, then symlink each file
  const jsrFilesByPkg = new Map<string, { jsrName: string; version: string; files: Map<string, string> }>();

  for (const mod of info.modules) {
    if (!mod.local || mod.specifier.startsWith("file://") || mod.kind === "npm") continue;
    const parsed = parseJsrUrl(mod.specifier);
    if (!parsed) continue;

    const pkgKey = `${parsed.jsrName}@${parsed.version}`;
    if (!jsrFilesByPkg.has(pkgKey)) {
      jsrFilesByPkg.set(pkgKey, { jsrName: parsed.jsrName, version: parsed.version, files: new Map() });
    }

    let file = parsed.file;
    const ext = guessExt(mod.mediaType, file);
    if (ext) file += ext;
    jsrFilesByPkg.get(pkgKey)!.files.set(file, mod.local);
  }

  for (const [pkgKey, pkg] of jsrFilesByPkg) {
    const nmName = jsrPkgToNodeModulesName(pkg.jsrName);
    const pkgDir = path.join(nodeModulesDir, nmName);

    // Create directory structure and symlink each file
    for (const [file, localPath] of pkg.files) {
      const filePath = path.join(pkgDir, file);
      await Deno.mkdir(path.dirname(filePath), { recursive: true });
      await ensureSymlink(localPath, filePath);
    }

    jsrPkgs.set(pkgKey, { jsrName: pkg.jsrName, version: pkg.version, nodeModulesDir: pkgDir });
  }

  // Phase 2: paths for jsr: and npm: redirects
  for (const [orig, resolved] of Object.entries(info.redirects)) {
    if (processed.has(orig)) continue;
    processed.add(orig);

    if (orig.startsWith("jsr:")) {
      const mod = info.modules.find((m) => m.specifier === resolved);
      if (mod?.specifier) {
        const parsed = parseJsrUrl(mod.specifier);
        if (parsed) {
          const nmName = jsrPkgToNodeModulesName(parsed.jsrName);
          let file = parsed.file;
          const ext = guessExt(mod.mediaType, file);
          if (ext) file += ext;
          paths[orig] = [path.join(nodeModulesDir, nmName, file)];
        }
      }
    } else if (orig.startsWith("npm:")) {
      const match = resolved.match(/^npm:\/?(.+)$/);
      if (!match) continue;
      const pkgSpec = match[1];
      const npmInfo = info.npmPackages[pkgSpec];
      if (!npmInfo) continue;
      const cacheDir = path.join(NPM_CACHE_BASE, "registry.npmjs.org", npmInfo.name, npmInfo.version);
      const typesEntry = resolveNpmTypes(cacheDir);
      if (typesEntry) paths[orig] = [typesEntry];
      npmPkgs.set(npmInfo.name, { name: npmInfo.name, version: npmInfo.version, cacheDir });
    }
  }

  // Phase 3: paths for direct https: imports from user code
  for (const mod of info.modules) {
    if (!mod.dependencies) continue;
    for (const dep of mod.dependencies) {
      if (processed.has(dep.specifier)) continue;
      if (!dep.specifier.startsWith("https://") && !dep.specifier.startsWith("http://")) continue;
      processed.add(dep.specifier);
      const resolvedSpec = dep.code?.specifier || dep.specifier;
      const targetMod = info.modules.find(
        (m) => m.specifier === resolvedSpec || m.specifier === dep.specifier,
      );
      if (targetMod?.specifier) {
        // Check if it's a jsr.io URL — if so, point to node_modules/@jsr/
        const parsed = parseJsrUrl(targetMod.specifier);
        if (parsed) {
          const nmName = jsrPkgToNodeModulesName(parsed.jsrName);
          let file = parsed.file;
          const ext = guessExt(targetMod.mediaType, file);
          if (ext) file += ext;
          paths[dep.specifier] = [path.join(nodeModulesDir, nmName, file)];
        }
        // For non-jsr URLs, we'd need a vendor dir — skip for now
      }
    }
  }

  // Also register all npm packages for node_modules symlinks
  for (const [, npmInfo] of Object.entries(info.npmPackages)) {
    if (!npmPkgs.has(npmInfo.name)) {
      npmPkgs.set(npmInfo.name, {
        name: npmInfo.name,
        version: npmInfo.version,
        cacheDir: path.join(NPM_CACHE_BASE, "registry.npmjs.org", npmInfo.name, npmInfo.version),
      });
    }
  }

  return { paths, npmPkgs, jsrPkgs };
}

// ── node_modules setup ──────────────────────────────────────────────────────

async function setupNpmLinks(
  nodeModulesDir: string,
  npmPkgs: Map<string, { name: string; version: string; cacheDir: string }>,
) {
  for (const [, pkg] of npmPkgs) {
    const parts = pkg.name.split("/");
    let targetDir: string;
    if (parts.length === 2) {
      await Deno.mkdir(path.join(nodeModulesDir, parts[0]), { recursive: true });
      targetDir = path.join(nodeModulesDir, parts[0], parts[1]);
    } else {
      targetDir = path.join(nodeModulesDir, pkg.name);
    }
    await ensureSymlink(pkg.cacheDir, targetDir);
  }
}

async function installTypesPackage(nodeModulesDir: string, pkg: string): Promise<boolean> {
  const atTypesDir = path.join(nodeModulesDir, "@types");
  await Deno.mkdir(atTypesDir, { recursive: true });
  const pkgName = pkg.replace("@types/", "");
  const targetDir = path.join(atTypesDir, pkgName);

  try { await Deno.lstat(targetDir); return true; } catch { /* */ }

  console.log(`  Installing ${pkg}...`);
  const { code } = await run(["cache", `npm:${pkg}`]);
  if (code !== 0) return false;

  const cacheBase = path.join(NPM_CACHE_BASE, "registry.npmjs.org", "@types", pkgName);
  try {
    const entries: string[] = [];
    for await (const entry of Deno.readDir(cacheBase)) {
      if (entry.isDirectory && entry.name !== "node_modules") entries.push(entry.name);
    }
    if (entries.length === 0) return false;
    entries.sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
    await ensureSymlink(path.join(cacheBase, entries[0]), targetDir);
    return true;
  } catch { return false; }
}

/** Generate wildcard module declarations for bytes/text imports */
async function writeImportShims(nodeModulesDir: string) {
  const shimsDir = path.join(nodeModulesDir, "@deno", "shims");
  await Deno.mkdir(shimsDir, { recursive: true });

  // Package.json so TypeScript recognizes it
  await Deno.writeTextFile(path.join(shimsDir, "package.json"), JSON.stringify({
    name: "@deno/shims",
    version: "0.0.0",
    types: "index.d.ts",
  }));

  // Wildcard module declarations for common asset extensions
  await Deno.writeTextFile(path.join(shimsDir, "index.d.ts"), `\
// Auto-generated: wildcard module declarations for Deno import assertions.
// "bytes" imports (binary assets) → Uint8Array
declare module "*.png" { const data: Uint8Array; export default data; }
declare module "*.jpg" { const data: Uint8Array; export default data; }
declare module "*.jpeg" { const data: Uint8Array; export default data; }
declare module "*.gif" { const data: Uint8Array; export default data; }
declare module "*.webp" { const data: Uint8Array; export default data; }
declare module "*.avif" { const data: Uint8Array; export default data; }
declare module "*.ico" { const data: Uint8Array; export default data; }
declare module "*.bmp" { const data: Uint8Array; export default data; }
declare module "*.wasm" { const data: Uint8Array; export default data; }
declare module "*.bin" { const data: Uint8Array; export default data; }
declare module "*.tar" { const data: Uint8Array; export default data; }
declare module "*.gz" { const data: Uint8Array; export default data; }
declare module "*.zip" { const data: Uint8Array; export default data; }
declare module "*.pdf" { const data: Uint8Array; export default data; }
declare module "*.mp3" { const data: Uint8Array; export default data; }
declare module "*.mp4" { const data: Uint8Array; export default data; }
declare module "*.wav" { const data: Uint8Array; export default data; }
declare module "*.ogg" { const data: Uint8Array; export default data; }
declare module "*.webm" { const data: Uint8Array; export default data; }
declare module "*.ttf" { const data: Uint8Array; export default data; }
declare module "*.woff" { const data: Uint8Array; export default data; }
declare module "*.woff2" { const data: Uint8Array; export default data; }

// "text" imports (text assets) → string
declare module "*.txt" { const data: string; export default data; }
declare module "*.csv" { const data: string; export default data; }
declare module "*.md" { const data: string; export default data; }
declare module "*.html" { const data: string; export default data; }
declare module "*.css" { const data: string; export default data; }
declare module "*.svg" { const data: string; export default data; }
declare module "*.xml" { const data: string; export default data; }
declare module "*.yaml" { const data: string; export default data; }
declare module "*.yml" { const data: string; export default data; }
declare module "*.toml" { const data: string; export default data; }
declare module "*.env" { const data: string; export default data; }
declare module "*.sql" { const data: string; export default data; }
declare module "*.graphql" { const data: string; export default data; }
declare module "*.gql" { const data: string; export default data; }
`);
}

// ── Generate tsconfig.json ──────────────────────────────────────────────────

function generateTsConfig(
  entrypoints: string[],
  tsconfigPaths: Record<string, string[]>,
): object {
  return {
    compilerOptions: {
      target: "ESNext",
      module: "preserve",
      moduleResolution: "bundler",
      lib: ["ESNext", "DOM", "DOM.Iterable"],
      strict: true,
      noEmit: true,
      skipLibCheck: true,
      esModuleInterop: true,
      allowImportingTsExtensions: true,
      allowJs: true,
      resolveJsonModule: true,
      // Resolve relative imports from the symlink location, not the target
      preserveSymlinks: true,
      paths: tsconfigPaths,
      baseUrl: ".",
      types: ["deno", "node", "@deno/shims"],
    },
    include: entrypoints,
  };
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  const args = Deno.args;
  if (args.length === 0) {
    console.error("Usage: deno_check.ts <file1.ts> [file2.ts ...]");
    Deno.exit(1);
  }

  const entrypoints = args.map((f) => path.resolve(f));
  const projectDir = path.dirname(entrypoints[0]);

  console.log("deno check (stock tsc prototype)");
  console.log();

  // Step 1: Module graph
  const info = await getModuleGraph(entrypoints);
  console.log(
    `Module graph: ${info.modules.length} modules, ` +
    `${Object.keys(info.redirects).length} redirects, ` +
    `${Object.keys(info.npmPackages).length} npm packages`,
  );

  const nodeModulesDir = path.join(projectDir, "node_modules");
  await Deno.mkdir(nodeModulesDir, { recursive: true });

  // Step 2: Build mappings — JSR goes into node_modules/@jsr/
  const result = await buildMappings(info, nodeModulesDir);
  console.log(`Path mappings: ${Object.keys(result.paths).length}`);
  console.log(`JSR packages: ${result.jsrPkgs.size}`);

  // Step 3: Symlink npm packages
  if (result.npmPkgs.size > 0) {
    await setupNpmLinks(nodeModulesDir, result.npmPkgs);
    console.log(`npm packages: ${result.npmPkgs.size}`);
  }

  // Step 4: @types + import shims
  console.log("Type definitions:");
  const hasDeno = await installTypesPackage(nodeModulesDir, "@types/deno");
  console.log(`  @types/deno: ${hasDeno ? "ok" : "failed"}`);
  const hasNode = await installTypesPackage(nodeModulesDir, "@types/node");
  console.log(`  @types/node: ${hasNode ? "ok" : "failed"}`);
  await writeImportShims(nodeModulesDir);
  console.log(`  @deno/shims: ok`);

  // Step 5: Generate tsconfig.json
  const tsconfig = generateTsConfig(entrypoints, result.paths);
  const tsconfigPath = path.join(projectDir, "tsconfig.json");
  await Deno.writeTextFile(tsconfigPath, JSON.stringify(tsconfig, null, 2));

  // Step 6: Run tsc
  console.log();
  console.log("Running tsc --noEmit ...");
  console.log();

  const tscCmd = new Deno.Command(DENO_BIN, {
    args: ["run", "--allow-all", "npm:typescript@5/tsc", "--noEmit", "--pretty", "--project", tsconfigPath],
    stdout: "inherit",
    stderr: "inherit",
    cwd: projectDir,
  });

  const tscResult = await tscCmd.output();

  if (tscResult.code === 0) {
    console.log("check passed.");
  } else {
    console.log("check failed.");
  }

  Deno.exit(tscResult.code);
}

main();
