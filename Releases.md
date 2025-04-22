# Releases

Binary releases can be downloaded manually at:
https://github.com/denoland/deno/releases

We also have one-line install commands at:
https://github.com/denoland/deno_install

### 2.2.11 / 2025.04.18

- fix(ext/node): Fix `Writable.toWeb()` (#28914)
- fix(ext/node): add `assert` property to test context object (#28904)
- fix(install/global): do not append `bin` to `DENO_INSTALL_ROOT` when ends with
  `bin` (#26446)
- fix(npm): panic when using tag with patched package (#28900)
- fix(task): document source of tasks, fix punctuation (#28413)
- fix: better cjs vs esm module detection and upgrade swc (#28810)
- fix: remove unnecessary whitespace in prompt function (#28868)
- fix: resolve shebang parse error in deno doc --test (#26079)
- perf(npm): don't try to cache npm packages we've already cached (#28938)

### 2.2.10 / 2025.04.14

- fix: enable explicit resource management for JavaScript (#28119)
- feat(unstable): support linux vsock (#28725)
- fix(ext/node): add basic support of suite/describe in node:test (#28847)
- fix(ext/node): export test as property of default export (#28881)
- fix(ext/node): querystring fallback to default decoder (#28838)
- fix(ext/node): upgrade `node:stream` (#28855)
- fix(fmt): upgrade malva to 0.11.2 (#28871)
- fix(install): read extra package info from node_modules and fallback to
  registry (#28893)
- fix(unstable): vsock nits (#28851)

### 2.2.9 / 2025.04.11

- fix(build): upgrade libffi to 4.0.0 (#28816)
- fix(compile): do not panic including node_modules directory (#28782)
- fix(compile): multi icon ordering on windows (#28771)
- fix(ext/canvas): handle integer overflow in `createImageBitmap` (#28764)
- fix(ext/node): add createReadStream & createWriteStream methods to the
  FileHandle class (#28700)
- fix(ext/node): add support for --no- prefix (allowNegative option) in
  parseArgs() (#28811)
- fix(ext/node): alias `shake-128` and `shake-256` hash algorithms (#28451)
- fix(ext/node): implement `Buffer.copyBytesFrom` (#28829)
- fix(ext/node): implement `process.loadEnvFile` (#28824)
- fix(ext/node): implement finished() for web streams (#28600)
- fix(ext/node): return `Buffer` from crypto cipher APIs (#28826)
- fix(ext/node): support input option in spawnSync (#28792)
- fix(ext/node): use primordials in `ext/node/polyfills/path/_posix.ts` (#28665)
- fix(ext/node): use primordials in `ext/node/polyfills/path/_win32.ts` (#28668)
- fix(ext/node): use primordials in `ext/node/polyfills/path/separator.ts`
  (#28669)
- fix(ext/node): verbose zlib error messages (#28831)
- fix(install): handle when bin entry info isn't present in package.json but is
  in registry (#28822)
- fix(install): regression where Deno not used when postinstall script ran
  script without file extension (#28786)
- fix(lockfile): handling of peer deps when migrating to lockfile v5 (#28844)
- fix(lockfile): omit tarball url from lockfile if it's the default (#28842)
- fix(lsp): exclude unmapped npm cache paths from auto-imports (#28841)
- fix(node): add reset method to event loop delay histogram (#28788)
- fix(task): support backticks and basic tilde expansion (#28832)
- fix(unstable): add missing decorators nodes in lint ast (#28834)
- fix(unstable): add parent types to lint ast nodes (#28802)
- fix(unstable): lint `.parent` property not traversing over groups (#28803)
- fix: dont strip n-api symbols in `denort` on mac (#28800)
- fix: use full SHA for canary panic URLs (#28819)
- perf(npm): load npm resolution snapshot directly from lockfile (#28647)

### 2.2.8 / 2025.04.05

- fix(compile): ensure atime/birthtime/mtime/ctime is set in vfs (#28731)
- fix(fmt): use non-zero exit code when formatting fails (#28523)
- fix(lint): resolve plugin paths from proper config specifier (#28752)
- fix(lsp): filter scheme in semantic tokens registration options (#28756)
- fix: only strip local and debug symbols from macOS binary to fix Node API
  (#28758)
- fix: upgrade v8 to 135.1.0 (#28697)

### 2.2.7 / 2025.04.04

- feat(unstable/otel): v8js metrics (#28592)
- fix(ext/node): better dns.lookup compatibility (#27936)
- fix(ext/node): sqlite handle empty blob being NULL (#28674)
- fix(ext/node): support the optional `previousValue` parameter for
  process.cpuUsage() (#28550)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_lstat.ts`
  (#28644)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_readv.ts`
  (#28645)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_realpath.ts`
  (#28652)
- fix(ext/webgpu): release current texture of surface after present (#28691)
- fix(install): remove duplicate deprecated messages (#28738)
- fix(lsp): format vscode-userdata schemed documents (#28706)
- fix(lsp): preserve notification order after init flag is raised (#28733)
- fix(lsp): url_to_uri() encoding on windows (#28737)
- fix(npm): further reduce duplicates with optional peers (#28705)
- fix(npm): reduce duplicate peers by preferring existing nv if nv anywhere in
  ancestor peers (#28663)
- fix(npm): use Deno instead of Node in more cases for lifecycle scripts
  (#28715)
- fix(runtime): fix duplicate unstable ids for no-legacy-abort (#28740)
- fix(task): filter empty elements in `xargs` (#28714)
- fix(test): ignore write errors in reporter (#28722)
- fix(types): add `Error.isError` type (#28679)
- fix(webgpu): move `isFallbackAdapter` from `GPUAdapter` to `GPUAdapterInfo`
  (#28650)
- fix: show referrer for Wasm module dependency errors (#28653)
- perf: remote symbolicate stack traces (#28470)

### 2.2.6 / 2025.03.28

- feat(unstable/run): ability to lazily load statically analyzable dynamic
  imports (#28593)
- fix(ext/crypto): support cross-curve ECDSA sign and verify (#28574)
- fix(ext/node): `mkdir()` parse file mode (#28609)
- fix(ext/node): emit 'close' event on ServerResponse object when client aborted
  the request (#28601)
- fix(ext/node): propagate 'close' event of IncomingMessage to Socket (#28582)
- fix(ext/node): sqlite error details (#28431)
- fix(ext/websocket): cancel in-flight handshake on close() (#28598)
- fix(npm): improve optional peer dep handling (#28651)
- fix(npm): reduce occurrences of duplicate packages due to peer dep resolution
  (#28586)
- fix(npm): resolve non-version matching peer deps and warn instead (#28616)
- fix(npm): set up bin entries for package even if it's already downloaded
  (#28626)
- perf(install): keep parsed npm package info in memory cache (#28636)

### 2.2.5 / 2025.03.21

- feat(unstable): basic otel event recording (#28552)
- feat(unstable): support using a local copy of npm packages (#28512)
- feat: upgrade deno_core and V8 13.5 (#28562)
- fix(doc): do not stack overflow for namespace that exports self or ancestor
  (#28533)
- fix(ext/node): add util.getCallSites (#28546)
- fix(ext/node): reset statement immidiately in run() (#28506)
- fix(ext/node): restrict ATTACH DATABASE statement (#28513)
- fix(ext/os): explicitly enable `sysinfoapi` feature on `winapi` dependency
  (#28568)
- fix(lsp): do not show import-map-remap diagnostic when referrer and target are
  within the entry base (#28560)
- fix(otel): replace `ArrayPrototypeSplit` with `StringPrototypeSplit` (#28538)
- fix: add stackTraceLimit to ErrorConstructor interface and removed
  ErrorWithStackTraceLimit interface (#28539)

### 2.2.4 / 2025.03.14

- feat(otel): span context propagators (#28460)
- feat(unstable/otel): add otel tracing to node:http.request (#28463)
- feat: support FORCE_COLOR (#28490)
- fix(bench): lower bench time budget when `n` is specified (#28454)
- fix(check): support `types@` export conditions (#28450)
- fix(check): support `typesVersions` in npm dependencies (#28468)
- fix(cli): warn when an otel env var has an invalid value (#28394)
- fix(ext/node): correct `STATUS_CODES` strings (#28489)
- fix(ext/node): use primordials in `ext/node/polyfills/path/_util.ts` (#28432)
- fix(install): exclude npm workspace packages from graph roots in `install`
  (#28401)
- fix(install): support "file:" dependencies in local package.json (#28396)
- fix(lsp): auto-import from npm package exports with manual node_modules
  (#28414)
- fix(lsp): silence errors from "codeAction/resolve" (#28400)
- fix(node): support re-exported esm modules in cjs export analysis (#28379)
- fix(otel): don't print otel warning when variable is not set (#28475)
- fix(otel/unstable): trace error cases of fetch (#28480)
- fix(run): skip the cjs suggestion for mjs/mts modules (#26698)
- fix(unstable): lint plugin `!==` wrongly parsed as `!=` (#28403)
- fix(unstable): wrong node with shorthand ObjectPattern + AssignPattern
  (#28402)
- fix: unhandled rejection from quic (#28448)
- perf(lsp): lazily start the ts server (#28392)

### 2.2.3 / 2025.03.05

- feat(unstable): lint plugins support field selectors (#28324)
- fix(add): better help text for --dev arg (#28304)
- fix(check/npm): move not found errors inside npm packages to tsc diagnostics
  (#28337)
- fix(ext/node): SQLite reset guards to prevent database locks (#28298)
- fix(ext/node): node compatibility issue missing fd in createServer callback
  socket object (#27789)
- fix(fmt/md): handle callout followed by non-text (#28333)
- fix(lint): run with --no-prompt (#28305)
- fix(lsp): include prefix and suffix for rename edits (#28327)
- fix(lsp): limit languages in semantic tokens provider (#28310)
- fix(node): require esm should prefer `module.exports` export (#28376)
- fix(otel): don't throw when calling setActiveSpan at root (#28323)
- fix(unstable): Missing `PrivateIdentifier` type for `PropertyDefinition` key
  (#28358)
- fix(unstable): lint plugin `ObjectPattern` inconsistencies (#28359)
- fix(unstable): lint plugin child combinator not working with groups (#28360)
- fix(unstable): lint plugin fix `:has()`, `:is/where/matches` and `:not()`
  selectors (#28348)
- fix(unstable): lint plugin regex attribute selector not working (#28340)
- fix(unstable): lint plugin swapped exported and source for
  ExportAllDeclaration (#28357)
- fix(unstable/lint): remove duplicated `Fix` vs `FixData` interface (#28344)
- fix: add "module.exports" export to ESM CJS wrapper module (#28373)
- fix: deno_ast 0.46 (#28331)
- fix: respect lockfile for multiple available jsr versions (#28375)
- perf(http): instantiate generic functions in `deno_http`, increase opt-level
  for some more hyper deps (#28317)
- perf(lsp): don't set resolver npm reqs if unchanged (#28302)
- perf(lsp): register semantic tokens provider upon opening enabled doc (#28384)

### 2.2.2 / 2025.02.25

- fix(check): regression - implicit jsxImportSource was not resolving (#28228)
- fix(cli): add `compilerOptions.lib` examples to config-file.v1.json (#28226)
- fix(config): allow specifying absolute path for patch and fix panic with
  exports in package.json (#28279)
- fix(ext/node): decipherIv() range error on invalid final block length (#28215)
- fix(ext/node): descriptive sqlite error messages (#28272)
- fix(fmt): support "--ext vto" and "--ext njk" (#28262)
- fix(http): generate `OtelInfo` only when otel metrics are enabled (#28286)
- fix(install): don't error on unknown media types in install (#28234)
- fix(lint): don't recurse infinitely for large ASTs (#28265)
- fix(lint): give access to SourceCode in 'deno test' (#28278)
- fix(lint): plugins ignored when no rust rule active (#28269)
- fix(lint): update deno_lint (#28271)
- fix(lsp): close server on exit notification (#28232)
- fix(lsp): create cacheable `ExportInfoMap` per language service (#28240)
- fix(unstable): lint plugin `:exit` called at wrong time (#28229)
- fix: add info suggestion for `unsafely-ignore-certificate-errors` and add
  `--help=full` (#28203)
- perf(install): only read initialized file if we care about the tags (#28242)

### 2.2.1 / 2025.02.20

- fix(check): remove instability in loading lib files (#28202)
- fix(check/lsp): fall back to `@types/*` packages if npm package doesn't have
  types (#28185)
- fix(coverage): exclude scripts with invalid URLs from raw coverage output
  (#28210)
- fix(ext/cache): add missing Cargo feature (#28178)
- fix(ext/node): Fix handling of sqlite large integers (#28193)
- fix(ext/node): rewrite SQLite named parameter handing (#28197)
- fix(outdated): hint to use `--latest` if new versions are available in
  `outdated --update` (#28190)
- fix(publish): support jsx/tsx (#28188)
- fix: better jsx workspace config resolution (#28186)
- fix: don't panic when running with // as a filepath (#28189)
- fix: move extension file declarations to cli/tsc/dts (#28180)

### 2.2.0 / 2025.02.18

- feat(bench): add `--permit-no-files` (#27048)
- feat(bench): add `warmup` and `n` for controlling number of iterations
  (#28123)
- feat(check/lsp): support "compilerOptions.rootDirs" (#27844)
- feat(compile): show remote modules and metadata size when compiling (#27415)
- feat(compile): support sloppy imports (#27944)
- feat(ext/cache): support lscache (#27628)
- feat(ext/canvas): enhance `createImageBitmap` specification compliance
  (#25517)
- feat(ext/node): implement `node:sqlite` (#27308)
- feat(http): add otel metrics (#28034)
- feat(jupyter): make GPUTexture and GPUBuffer displayable (#28117)
- feat(lint): add JavaScript plugin support (#27203)
- feat(lint): add rules for react/preact (#27162)
- feat(lint): change behavior of `--rules` flag (#27245)
- feat(node:http): add http information support (#27381)
- feat(outdated): interactive update (#27812)
- feat(task): add support for task wildcards (#27007)
- feat(unstable): WebTransport (#27431)
- feat(unstable): add `lint.plugins` to config schema (#27982)
- feat(unstable): add basic support for otel trace links (#27727)
- feat(unstable): add js lint plugin source code helpers (#28065)
- feat(unstable): add lint plugin ast types (#27977)
- feat(unstable): add test for lint plugin destroy hook (#27981)
- feat(unstable): align lint ast with TSEStree (#27996)
- feat(unstable): support multiple fixes from lint plugins (#28040)
- feat(unstable): type lint plugin visitor (#28005)
- feat: Deno.cwd() no longer requires --allow-read permission (#27192)
- feat: TypeScript 5.7 (#27857)
- feat: Upgrade V8 to 13.4 (#28080)
- feat: implement `process.cpuUsage` (`Deno.cpuUsage`) (#27217)
- feat: support XDG_CACHE_HOME for deno dir on macos (#28173)
- fix(check): npm resolution errors to tsc diagnostics (#28174)
- fix(check): support sloppy imports with "compilerOptions.rootDirs" (#27973)
- fix(cli): remove extraneous comma in task --eval help (#26985)
- fix(completions): remove problematic character for powershell (#28102)
- fix(ext/node): `DatabaseSync#exec` should execute batch statements (#28053)
- fix(ext/node): enforce -RW perms on `node:sqlite` (#27928)
- fix(ext/node): expose sqlite changeset constants (#27992)
- fix(ext/node): implement SQLite Session API (#27909)
- fix(ext/node): implement StatementSync#iterate (#28168)
- fix(ext/node): implement `DatabaseSync#applyChangeset()` (#27967)
- fix(ext/node): represent sqlite blob as Uint8Array (#27889)
- fix(ext/node): sqlite bind support bigint values (#27890)
- fix(ext/node): support read-only database in `node:sqlite` (#27930)
- fix(ext/node): throw RangeError when sqlite INTEGER is too large (#27907)
- fix(ext/node): throw Session methods when database is closed (#27968)
- fix(ext/node): use primordials in `ext/node/polyfills/path/common.ts` (#28164)
- fix(ext/sqlite): add `sourceSQL` and `expandedSQL` getters (#27921)
- fix(init): force --reload if npm or jsr package (#28150)
- fix(install/global): do not error if path is an npm pkg and relative file
  (#26975)
- fix(lint): `Deno.lint.runPlugin` throws in `deno run` (#28063)
- fix(lint): clear plugin diagnostics on each lint file run (#28011)
- fix(lint): disable incremental caching if JS plugins are used (#28026)
- fix(lint): don't mark plugin diagnostic as fixable, if it's not (#28147)
- fix(lint): don't show docs URLs for plugins (#28033)
- fix(lint): out of order diagnostics for plugins (#28029)
- fix(lint): react-rules-of-hooks works with destructuring (#28113)
- fix(lint): update jsx/react related rules and names (#27836)
- fix(lsp): include description for auto-import completions (#28088)
- fix(node/sqlite): sqlite named parameters (#28154)
- fix(publish): error on missing name field (#27131)
- fix(task): support --frozen flag (#28094)
- fix(task): update --filter flag description (#26974)
- fix(unstable): add missing rule context types (#28014)
- fix(unstable): align js lint context API with eslint (#28066)
- fix(unstable/temporal): implement
  `Temporal.ZonedDateTime.getTimeZoneTransition` (#27770)
- fix(workspace): diagnostic for imports in member with importMap at root
  (#28116)
- fix: add hint to run with `--no-check` when type checking fails (#28091)
- fix: cache bust http cache on lockfile integrity mismatch (#28087)
- fix: handle all values for buffers in turbocall codegen (#28170)
- perf(check): use v8 code cache for extension sources in `deno check` (#28089)
- perf(lsp): add built-in tracing support for the LSP (#27843)
- perf(lsp): don't clone asset text (#28165)
- perf(lsp): make auto-imports a little faster (#28106)

### 2.1.12 / 2025.04.11

The 2.1.11 release had an incorrect version number when doing `deno -v`.

- fix(ext/node): alias `shake-128` and `shake-256` hash algorithms (#28451)
- fix(ext/node): return `Buffer` from crypto cipher APIs (#28826)
- fix(ext/node): support input option in spawnSync (#28792)
- fix(ext/node): use primordials in `ext/node/polyfills/path/separator.ts`
  (#28669)
- fix(node): add reset method to event loop delay histogram (#28788)

### 2.1.11 / 2025.04.08

- docs: add examples for SubtleCrypto (#28068)
- docs: adding a missing full stop to context help text (#28465)
- docs: adding jsdocs for temporalAPI (#28542)
- docs: fix a numerical error in update_typescript.md (#28556)
- docs: fix a typo in specs README.md (#28524)
- docs: fixed a typo in update_typescript.md (#28486)
- docs: ignore absent window global variable in d.ts (#28456)
- docs: making copy a little clearer (#28481)
- docs: randomUUID and getRandomValues (#28496)
- docs(canvas): Add examples to createImageBitmap jsdocs (#28055)
- docs(console): update console documentation (#28196)
- docs(web): update docs for `globalThis.caches` (#28061)
- fix: add hint to run with `--no-check` when type checking fails (#28091)
- fix: add info suggestion for `unsafely-ignore-certificate-errors` and add
  `--help=full` (#28203)
- fix: add stackTraceLimit to ErrorConstructor interface and removed
  ErrorWithStackTraceLimit interface (#28539)
- fix: cache bust http cache on lockfile integrity mismatch (#28087)
- fix: don't panic when running with // as a filepath (#28189)
- fix(add): better help text for --dev arg (#28304)
- fix(check): npm resolution errors to tsc diagnostics (#28174)
- fix(cli): add `compilerOptions.lib` examples to config-file.v1.json (#28226)
- fix(completions): remove problematic character for powershell (#28102)
- fix(coverage): exclude scripts with invalid URLs from raw coverage output
  (#28210)
- fix(ext/cache): add missing Cargo feature (#28178)
- fix(ext/node): `mkdir()` parse file mode (#28609)
- fix(ext/node): correct `STATUS_CODES` strings (#28489)
- fix(ext/node): decipherIv() range error on invalid final block length (#28215)
- fix(ext/node): emit 'close' event on ServerResponse object when client aborted
  the request (#28601)
- fix(ext/node): node compatibility issue missing fd in createServer callback
  socket object (#27789)
- fix(ext/node): propagate 'close' event of IncomingMessage to Socket (#28582)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_lstat.ts`
  (#28644)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_readv.ts`
  (#28645)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs/_fs_realpath.ts`
  (#28652)
- fix(ext/node): use primordials in `ext/node/polyfills/path/_util.ts` (#28432)
- fix(ext/node): use primordials in `ext/node/polyfills/path/common.ts` (#28164)
- fix(ext/os): explicitly enable `sysinfoapi` feature on `winapi` dependency
  (#28568)
- fix(ext/websocket): cancel in-flight handshake on close() (#28598)
- fix(fmt): support "--ext vto" and "--ext njk" (#28262)
- fix(init): force --reload if npm or jsr package (#28150)
- fix(install): don't error on unknown media types in install (#28234)
- fix(install): exclude npm workspace packages from graph roots in `install`
  (#28401)
- fix(npm): further reduce duplicates with optional peers (#28705)
- fix(npm): improve optional peer dep handling (#28651)
- fix(npm): reduce duplicate peers by preferring existing nv if nv anywhere in
  ancestor peers (#28663)
- fix(npm): reduce occurrences of duplicate packages due to peer dep resolution
  (#28586)
- fix(outdated): hint to use `--latest` if new versions are available in
  `outdated --update` (#28190)
- fix(run): skip the cjs suggestion for mjs/mts modules (#26698)
- fix(task): support --frozen flag (#28094)
- fix(types): add `Error.isError` type (#28679)
- fix(unstable/temporal): implement
  `Temporal.ZonedDateTime.getTimeZoneTransition` (#27770)
- perf(install): keep parsed npm package info in memory cache (#28636)
- perf(install): only read initialized file if we care about the tags (#28242)
- perf(lsp): make auto-imports a little faster (#28106)

### 2.1.10 / 2025.02.13

- Revert "fix(lsp): silence debug error for 'move to a new file' action
  (#27780)" (#27903)
- fix(cli): Fix panic in `load_native_certs` (#27863)
- fix(compile): never include the specified output executable in itself (#27877)
- fix(ext/napi): napi_is_buffer tests for ArrayBufferView (#27956)
- fix(ext/node): expose brotli stream APIs (#27943)
- fix(ext/node): fix missing privateKey.x in curve25519 JWK (#27990)
- fix(ext/node): fix twitter-api-v2 compatibility (#27971)
- fix(ext/node): handle non-ws upgrade headers (#27931)
- fix(ext/node): set process fields on own instance (#27927)
- fix(ext/node): set process.env as own property (#27891)
- fix(ext/node): support proxy http request (#27871)
- fix(lsp): ignore a few more diagnostics for ambient modules (#27949)
- fix(node): resolve module as maybe CJS when it's missing a file extension
  (#27904)
- fix(node): show directory import and missing extension suggestions (#27905)
- fix(otel): custom span start + end times are fractional ms (#27995)
- fix(publish): correct coloring in --help (#27939)
- fix(streams): handle Resource stream error (#27975)
- fix: allow creating TSC host without a snapshot (#28058)
- fix: do special file permission check for `check_read_path` (#27989)
- fix: panic with js lint plugins and invalid js syntax (#28006)
- perf(compile): use bytes already in memory after downloading executable
  (#28000)
- perf(lsp): cancellation checks in blocking code (#27997)
- perf: node resolution cache (#27838)

### 2.1.9 / 2025.01.30

- fix(ext/node): add http information support (#27381)
- perf(crypto): use ring for asm implementations of sha256/sha512 (#27885)

### 2.1.8 / 2025.01.30

- feat(unstable): support https otlp endpoints (#27743)
- fix(check): better handling of TypeScript in npm packages for type checking
  (#27853)
- fix(check): compiler options from workspace members (#27785)
- fix(core): Fix `create_stack_trace` from empty trace (#27873)
- fix(core): handle dyn imports exceeding call stack size (#27825)
- fix(ext/crypto): export private x25519 JWK key (#27828)
- fix(ext/crypto): fix jwk key_ops validation (#27827)
- fix(ext/fetch): update h2 to fix sending a PROTOCOL_ERROR instead of
  REFUSED_STREAM when receiving oversized headers (#27531)
- fix(ext/node): clear tz cache when setting process.env.TZ (#27826)
- fix(ext/node): do not apply socket-init-workaround to ipc socket (#27779)
- fix(ext/node): fix async variant of brotliDecompress (#27815)
- fix(ext/node): fix formatting of debug logs (#27772)
- fix(ext/node): fix panic when invalid AES GCM key size (#27818)
- fix(ext/node): implement X509Certificate#checkHost (#27821)
- fix(ext/node): implement `aes-128-ctr`, `aes-192-ctr`, and `aes-256-ctr`
  (#27630)
- fix(ext/node): implement `crypto.hash` (#27858)
- fix(ext/node): npm:mqtt compatibility (#27792)
- fix(ext/node): reference error in zlib.crc32 (#27777)
- fix(ext/node): scrypt panic when `log_n` > 64 (#27816)
- fix(init): correct dev task for --lib (#27860)
- fix(install/global): warn about not including auto-discovered config file
  (#27745)
- fix(lsp): ignore errors on ambient module imports (#27855)
- fix(lsp): silence debug error for 'move to a new file' action (#27780)
- fix(node): align type stripping in node_modules error message with Node
  (#27809)
- fix(npmrc): merge `.npmrc` in user's homedir and project (#27119)
- fix(process/windows): correct command resolution when PATH env var not
  uppercase (#27846)
- fix(publish): unfurl sloppy imports in d.ts files and type imports (#27793)
- fix(types): `Deno.readDirSync`'s type returns an `IteratorObject` (#27805)
- fix: do not log cache creation failure on readonly file system (#27794)
- perf(lsp): cache completion item resolution during request (#27831)
- perf(node_resolver): reduce url to/from path conversions (#27839)
- perf: full LTO in sysroot (#27771)

### 2.1.7 / 2025.01.21

- fix(deps): update yanked crates (#27512)
- fix(ext/node): GCM auth tag check on DechiperIv#final (#27733)
- fix(ext/node): add FileHandle#sync (#27677)
- fix(ext/node): propagate socket error to client request object (#27678)
- fix(ext/node): tls.connect regression (#27707)
- fix(ext/os): pass SignalState to web worker (#27741)
- fix(install/global): remove importMap field from specified config file
  (#27744)
- fix: use 'getrandom' feature for 'sys_traits' crate
- perf(compile): remove swc from denort (#27721)

### 2.1.6 / 2025.01.16

- fix(check/lsp): correctly resolve compilerOptions.types (#27686)
- fix(check/lsp): fix bugs with tsc type resolution, allow npm packages to
  augment `ImportMeta` (#27690)
- fix(compile): store embedded fs case sensitivity (#27653)
- fix(compile/windows): better handling of deno_dir on different drive letter
  than code (#27654)
- fix(ext/console): change Temporal color (#27684)
- fix(ext/node): add `writev` method to `FileHandle` (#27563)
- fix(ext/node): add chown method to FileHandle class (#27638)
- fix(ext/node): apply `@npmcli/agent` workaround to `npm-check-updates`
  (#27639)
- fix(ext/node): fix playwright http client (#27662)
- fix(ext/node): show bare-node-builtin hint when using an import map (#27632)
- fix(ext/node): use primordials in `ext/node/polyfills/_fs_common.ts` (#27589)
- fix(lsp): handle pathless untitled URIs (#27637)
- fix(lsp/check): don't resolve unknown media types to a `.js` extension
  (#27631)
- fix(node): Prevent node:child_process from always inheriting the parent
  environment (#27343) (#27340)
- fix(node/fs): add utimes method to the FileHandle class (#27582)
- fix(outdated): Use `latest` tag even when it's the same as the current version
  (#27699)
- fix(outdated): retain strict semver specifier when updating (#27701)

### 2.1.5 / 2025.01.09

- feat(unstable): implement QUIC (#21942)
- feat(unstable): add JS linting plugin infrastructure (#27416)
- feat(unstable): add OTEL MeterProvider (#27240)
- feat(unstable): no config npm:@opentelemetry/api integration (#27541)
- feat(unstable): replace SpanExporter with TracerProvider (#27473)
- feat(unstable): support selectors in JS lint plugins (#27452)
- fix(check): line-break between diagnostic message chain entries (#27543)
- fix(check): move module not found errors to typescript diagnostics (#27533)
- fix(compile): analyze modules in directory specified in --include (#27296)
- fix(compile): be more deterministic when compiling the same code in different
  directories (#27395)
- fix(compile): display embedded file sizes and total (#27360)
- fix(compile): output contents of embedded file system (#27302)
- fix(ext/fetch): better error message when body resource is unavailable
  (#27429)
- fix(ext/fetch): retry some http/2 errors (#27417)
- fix(ext/fs): do not throw for bigint ctime/mtime/atime (#27453)
- fix(ext/http): improve error message when underlying resource of request body
  unavailable (#27463)
- fix(ext/net): update moka cache to avoid potential panic in `Deno.resolveDns`
  on some laptops with Ryzen CPU (#27572)
- fix(ext/node): fix `fs.access`/`fs.promises.access` with `X_OK` mode parameter
  on Windows (#27407)
- fix(ext/node): fix `os.cpus()` on Linux (#27592)
- fix(ext/node): RangeError timingSafeEqual with different byteLength (#27470)
- fix(ext/node): add `truncate` method to the `FileHandle` class (#27389)
- fix(ext/node): add support of any length IV for aes-(128|256)-gcm ciphers
  (#27476)
- fix(ext/node): convert brotli chunks with proper byte offset (#27455)
- fix(ext/node): do not exit worker thread when there is pending async op
  (#27378)
- fix(ext/node): have `process` global available in Node context (#27562)
- fix(ext/node): make getCiphers return supported ciphers (#27466)
- fix(ext/node): sort list of built-in modules alphabetically (#27410)
- fix(ext/node): support createConnection option in node:http.request() (#25470)
- fix(ext/node): support private key export in JWK format (#27325)
- fix(ext/web): add `[[ErrorData]]` slot to `DOMException` (#27342)
- fix(ext/websocket): Fix close code without reason (#27578)
- fix(jsr): Wasm imports fail to load (#27594)
- fix(kv): improve backoff error message and inline documentation (#27537)
- fix(lint): fix single char selectors being ignored (#27576)
- fix(lockfile): include dependencies listed in external import map in lockfile
  (#27337)
- fix(lsp): css preprocessor formatting (#27526)
- fix(lsp): don't skip dirs with enabled subdirs (#27580)
- fix(lsp): include "node:" prefix for node builtin auto-imports (#27404)
- fix(lsp): respect "typescript.suggestionActions.enabled" setting (#27373)
- fix(lsp): rewrite imports for 'Move to a new file' action (#27427)
- fix(lsp): sql and component file formatting (#27350)
- fix(lsp): use verbatim specifier for URL auto-imports (#27605)
- fix(no-slow-types): handle rest param with internal assignments (#27581)
- fix(node/fs): add a chmod method to the FileHandle class (#27522)
- fix(node): add missing `inspector/promises` (#27491)
- fix(node): handle cjs exports with escaped chars (#27438)
- fix(npm): deterministically output tags to initialized file (#27514)
- fix(npm): search node_modules folder for package matching npm specifier
  (#27345)
- fix(outdated): ensure "Latest" version is greater than "Update" version
  (#27390)
- fix(outdated): support updating dependencies in external import maps (#27339)
- fix(permissions): implicit `--allow-import` when using `--cached-only`
  (#27530)
- fix(publish): infer literal types in const contexts (#27425)
- fix(task): properly handle task name wildcards with --recursive (#27396)
- fix(task): support tasks without commands (#27191)
- fix(unstable): don't error on non-existing attrs or type attr (#27456)
- fix: FastString v8_string() should error when cannot allocated (#27375)
- fix: deno_resolver crate without 'sync' feature (#27403)
- fix: incorrect memory info free/available bytes on mac (#27460)
- fix: upgrade deno_doc to 0.161.3 (#27377)
- perf(fs/windows): stat - only open file once (#27487)
- perf(node/fs/copy): reduce metadata lookups copying directory (#27495)
- perf: don't store duplicate info for ops in the snapshot (#27430)
- perf: remove now needless canonicalization getting closest package.json
  (#27437)
- perf: upgrade to deno_semver 0.7 (#27426)

### 2.1.4 / 2024.12.11

- feat(unstable): support caching npm dependencies only as they're needed
  (#27300)
- fix(compile): correct read length for transpiled typescript files (#27301)
- fix(ext/node): accept file descriptor in fs.readFile(Sync) (#27252)
- fix(ext/node): handle Float16Array in node:v8 module (#27285)
- fix(lint): do not error providing --allow-import (#27321)
- fix(node): update list of builtin node modules, add missing export to
  _http_common (#27294)
- fix(outdated): error when there are no config files (#27306)
- fix(outdated): respect --quiet flag for hints (#27317)
- fix(outdated): show a suggestion for updating (#27304)
- fix(task): do not always kill child on ctrl+c on windows (#27269)
- fix(unstable): don't unwrap optional state in otel (#27292)
- fix: do not error when subpath has an @ symbol (#27290)
- fix: do not panic when fetching invalid file url on Windows (#27259)
- fix: replace the @deno-types with @ts-types (#27310)
- perf(compile): improve FileBackedVfsFile (#27299)

### 2.1.3 / 2024.12.05

- feat(unstable): add metrics to otel (#27143)
- fix(fmt): stable formatting of HTML files with JS (#27164)
- fix(install): use locked version of jsr package when fetching exports (#27237)
- fix(node/fs): support `recursive` option in readdir (#27179)
- fix(node/worker_threads): data url not encoded properly with eval (#27184)
- fix(outdated): allow `--latest` without `--update` (#27227)
- fix(task): `--recursive` option not working (#27183)
- fix(task): don't panic with filter on missing task argument (#27180)
- fix(task): forward signals to spawned sub-processes on unix (#27141)
- fix(task): kill descendants when killing task process on Windows (#27163)
- fix(task): only pass args to root task (#27213)
- fix(unstable): otel context with multiple keys (#27230)
- fix(unstable/temporal): respect locale in `Duration.prototype.toLocaleString`
  (#27000)
- fix: clear dep analysis when module loading is done (#27204)
- fix: improve auto-imports for npm packages (#27224)
- fix: support `workspace:^` and `workspace:~` version constraints (#27096)

### 2.1.2 / 2024.11.28

- feat(unstable): Instrument Deno.serve (#26964)
- feat(unstable): Instrument fetch (#27057)
- feat(unstable): repurpose `--unstable-detect-cjs` to attempt loading more
  modules as cjs (#27094)
- fix(check): support jsdoc `@import` tag (#26991)
- fix(compile): correct buffered reading of assets and files (#27008)
- fix(compile): do not error embedding same symlink via multiple methods
  (#27015)
- fix(compile): handle TypeScript file included as asset (#27032)
- fix(ext/fetch): don't throw when `bodyUsed` inspect after upgrade (#27088)
- fix(ext/node): `tls.connect` socket upgrades (#27125)
- fix(ext/node): add `fs.promises.fstat` and `FileHandle#stat` (#26719)
- fix(ext/webgpu): normalize limits to number (#27072)
- fix(ext/webgpu): use correct variable name (#27108)
- fix(ext/websocket): don't throw exception when sending to closed socket
  (#26932)
- fix(fmt): return `None` if sql fmt result is the same (#27014)
- fix(info): resolve bare specifier pointing to workspace member (#27020)
- fix(init): always force managed node modules (#27047)
- fix(init): support scoped npm packages (#27128)
- fix(install): don't re-set up node_modules if running lifecycle script
  (#26984)
- fix(lsp): remove stray debug output (#27010)
- fix(lsp): support task object notation for tasks request (#27076)
- fix(lsp): wasm file import completions (#27018)
- fix(node): correct resolution of dynamic import of esm from cjs (#27071)
- fix(node/fs): add missing stat path argument validation (#27086)
- fix(node/fs): missing uv error context for readFile (#27011)
- fix(node/http): casing ignored in ServerResponse.hasHeader() (#27105)
- fix(node/timers): error when passing id to clearTimeout/clearInterval (#27130)
- fix(runtime/ops): Fix watchfs remove event (#27041)
- fix(streams): reject `string` in `ReadableStream.from` type (#25116)
- fix(task): handle carriage return in task description (#27099)
- fix(task): handle multiline descriptions properly (#27069)
- fix(task): strip ansi codes and control chars when printing tasks (#27100)
- fix(tools/doc): HTML resolve main entrypoint from config file (#27103)
- fix: support bun specifiers in JSR publish (#24588)
- fix: support non-function exports in Wasm modules (#26992)
- perf(compile): read embedded files as static references when UTF-8 and reading
  as strings (#27033)
- perf(ext/webstorage): use object wrap for `Storage` (#26931)

### 2.1.1 / 2024.11.21

- docs(add): clarification to add command (#26968)
- docs(doc): fix typo in doc subcommand help output (#26321)
- fix(node): regression where ts files were sometimes resolved instead of js
  (#26971)
- fix(task): ensure root config always looks up dependencies in root (#26959)
- fix(watch): don't panic if there's no path provided (#26972)
- fix: Buffer global in --unstable-node-globals (#26973)

### 2.1.0 / 2024.11.21

- feat(cli): add `--unstable-node-globals` flag (#26617)
- feat(cli): support multiple env file argument (#26527)
- feat(compile): ability to embed directory in executable (#26939)
- feat(compile): ability to embed local data files (#26934)
- feat(ext/fetch): Make fetch client parameters configurable (#26909)
- feat(ext/fetch): allow embedders to use `hickory_dns_resolver` instead of
  default `GaiResolver` (#26740)
- feat(ext/fs): add ctime to Deno.stats and use it in node compat layer (#24801)
- feat(ext/http): Make http server parameters configurable (#26785)
- feat(ext/node): perf_hooks.monitorEventLoopDelay() (#26905)
- feat(fetch): accept async iterables for body (#26882)
- feat(fmt): support SQL (#26750)
- feat(info): show location for Web Cache (#26205)
- feat(init): add --npm flag to initialize npm projects (#26896)
- feat(jupyter): Add `Deno.jupyter.image` API (#26284)
- feat(lint): Add checked files list to the JSON output(#26936)
- feat(lsp): auto-imports with @deno-types directives (#26821)
- feat(node): stabilize detecting if CJS via `"type": "commonjs"` in a
  package.json (#26439)
- feat(permission): support suffix wildcards in `--allow-env` flag (#25255)
- feat(publish): add `--set-version <version>` flag (#26141)
- feat(runtime): remove public OTEL trace API (#26854)
- feat(task): add --eval flag (#26943)
- feat(task): dependencies (#26467)
- feat(task): support object notation, remove support for JSDocs (#26886)
- feat(task): workspace support with --filter and --recursive (#26949)
- feat(watch): log which file changed on HMR or watch change (#25801)
- feat: OpenTelemetry Tracing API and Exporting (#26710)
- feat: Wasm module support (#26668)
- feat: fmt and lint respect .gitignore file (#26897)
- feat: permission stack traces in ops (#26938)
- feat: subcommand to view and update outdated dependencies (#26942)
- feat: upgrade V8 to 13.0 (#26851)
- fix(cli): preserve comments in doc tests (#26828)
- fix(cli): show prefix hint when installing a package globally (#26629)
- fix(ext/cache): gracefully error when cache creation failed (#26895)
- fix(ext/http): prefer brotli for `accept-encoding: gzip, deflate, br, zstd`
  (#26814)
- fix(ext/node): New async setInterval function to improve the nodejs
  compatibility (#26703)
- fix(ext/node): add autoSelectFamily option to net.createConnection (#26661)
- fix(ext/node): handle `--allow-sys=inspector` (#26836)
- fix(ext/node): increase tolerance for interval test (#26899)
- fix(ext/node): process.getBuiltinModule (#26833)
- fix(ext/node): use ERR_NOT_IMPLEMENTED for notImplemented (#26853)
- fix(ext/node): zlib.crc32() (#26856)
- fix(ext/webgpu): Create GPUQuerySet converter before usage (#26883)
- fix(ext/websocket): initialize `error` attribute of WebSocket ErrorEvent
  (#26796)
- fix(ext/webstorage): use error class for sqlite error case (#26806)
- fix(fmt): error instead of panic on unstable format (#26859)
- fix(fmt): formatting of .svelte files (#26948)
- fix(install): percent encodings in interactive progress bar (#26600)
- fix(install): re-setup bin entries after running lifecycle scripts (#26752)
- fix(lockfile): track dependencies specified in TypeScript compiler options
  (#26551)
- fix(lsp): ignore editor indent settings if deno.json is present (#26912)
- fix(lsp): skip code action edits that can't be converted (#26831)
- fix(node): handle resolving ".//<something>" in npm packages (#26920)
- fix(node/crypto): support promisify on generateKeyPair (#26913)
- fix(permissions): say to use --allow-run instead of --allow-all (#26842)
- fix(publish): improve error message when missing exports (#26945)
- fix: otel resiliency (#26857)
- fix: update message for unsupported schemes with npm and jsr (#26884)
- perf(compile): code cache (#26528)
- perf(windows): delay load webgpu and some other dlls (#26917)
- perf: use available system memory for v8 isolate memory limit (#26868)

### 2.0.6 / 2024.11.10

- feat(ext/http): abort event when request is cancelled (#26781)
- feat(ext/http): abort signal when request is cancelled (#26761)
- feat(lsp): auto-import completions from byonm dependencies (#26680)
- fix(ext/cache): don't panic when creating cache (#26780)
- fix(ext/node): better inspector support (#26471)
- fix(fmt): don't use self-closing tags in HTML (#26754)
- fix(install): cache jsr deps from all workspace config files (#26779)
- fix(node:zlib): gzip & gzipSync should accept ArrayBuffer (#26762)
- fix: performance.timeOrigin (#26787)

### 2.0.5 / 2024.11.05

- fix(add): better error message when adding package that only has pre-release
  versions (#26724)
- fix(add): only add npm deps to package.json if it's at least as close as
  deno.json (#26683)
- fix(cli): set `npm_config_user_agent` when running npm packages or tasks
  (#26639)
- fix(coverage): exclude comment lines from coverage reports (#25939)
- fix(ext/node): add `findSourceMap` to the default export of `node:module`
  (#26720)
- fix(ext/node): convert errors from `fs.readFile/fs.readFileSync` to node
  format (#26632)
- fix(ext/node): resolve exports even if parent module filename isn't present
  (#26553)
- fix(ext/node): return `this` from `http.Server.ref/unref()` (#26647)
- fix(fmt): do not panic for jsx ignore container followed by jsx text (#26723)
- fix(fmt): fix several HTML and components issues (#26654)
- fix(fmt): ignore file directive for YAML files (#26717)
- fix(install): handle invalid function error, and fallback to junctions
  regardless of the error (#26730)
- fix(lsp): include unstable features from editor settings (#26655)
- fix(lsp): scope attribution for lazily loaded assets (#26699)
- fix(node): Implement `os.userInfo` properly, add missing `toPrimitive`
  (#24702)
- fix(serve): support serve hmr (#26078)
- fix(types): missing `import` permission on `PermissionOptionsObject` (#26627)
- fix(workspace): support wildcard packages (#26568)
- fix: clamp smi in fast calls by default (#26506)
- fix: improved support for cjs and cts modules (#26558)
- fix: op_run_microtasks crash (#26718)
- fix: panic_hook hangs without procfs (#26732)
- fix: remove permission check in op_require_node_module_paths (#26645)
- fix: surface package.json location on dep parse failure (#26665)
- perf(lsp): don't walk coverage directory (#26715)

### 2.0.4 / 2024.10.29

- Revert "fix(ext/node): fix dns.lookup result ordering (#26264)" (#26621)
- Revert "fix(ext/node): use primordials in `ext/node/polyfills/https.ts`
  (#26323)" (#26613)
- feat(lsp): "typescript.preferences.preferTypeOnlyAutoImports" setting (#26546)
- fix(check): expose more globals from @types/node (#26603)
- fix(check): ignore resolving `jsxImportSource` when jsx is not used in graph
  (#26548)
- fix(cli): Make --watcher CLEAR_SCREEN clear scrollback buffer as well as
  visible screen (#25997)
- fix(compile): regression handling redirects (#26586)
- fix(ext/napi): export dynamic symbols list for {Free,Open}BSD (#26605)
- fix(ext/node): add path to `fs.stat` and `fs.statSync` error (#26037)
- fix(ext/node): compatibility with {Free,Open}BSD (#26604)
- fix(ext/node): use primordials in
  ext\node\polyfills\internal\crypto\_randomInt.ts (#26534)
- fix(install): cache json exports of JSR packages (#26552)
- fix(install): regression - do not panic when config file contains \r\n
  newlines (#26547)
- fix(lsp): make missing import action fix infallible (#26539)
- fix(npm): match npm bearer token generation (#26544)
- fix(upgrade): stop running `deno lsp` processes on windows before attempting
  to replace executable (#26542)
- fix(watch): don't panic on invalid file specifiers (#26577)
- fix: do not panic when failing to write to http cache (#26591)
- fix: provide hints in terminal errors for Node.js globals (#26610)
- fix: report exceptions from nextTick (#26579)
- fix: support watch flag to enable watching other files than the main module on
  serve subcommand (#26622)
- perf: pass transpiled module to deno_core as known string (#26555)

### 2.0.3 / 2024.10.25

- feat(lsp): interactive inlay hints (#26382)
- fix: support node-api in denort (#26389)
- fix(check): support `--frozen` on deno check (#26479)
- fix(cli): increase size of blocking task threadpool on windows (#26465)
- fix(config): schemas for lint rule and tag autocompletion (#26515)
- fix(ext/console): ignore casing for named colors in css parsing (#26466)
- fix(ext/ffi): return u64/i64 as bigints from nonblocking ffi calls (#26486)
- fix(ext/node): cancel pending ipc writes on channel close (#26504)
- fix(ext/node): map `ERROR_INVALID_NAME` to `ENOENT` on windows (#26475)
- fix(ext/node): only set our end of child process pipe to nonblocking mode
  (#26495)
- fix(ext/node): properly map reparse point error in readlink (#26375)
- fix(ext/node): refactor http.ServerResponse into function class (#26210)
- fix(ext/node): stub HTTPParser internal binding (#26401)
- fix(ext/node): use primordials in `ext/node/polyfills/https.ts` (#26323)
- fix(fmt): --ext flag requires to pass files (#26525)
- fix(fmt): upgrade formatters (#26469)
- fix(help): missing package specifier (#26380)
- fix(info): resolve workspace member mappings (#26350)
- fix(install): better json editing (#26450)
- fix(install): cache all exports of JSR packages listed in `deno.json` (#26501)
- fix(install): cache type only module deps in `deno install` (#26497)
- fix(install): don't cache json exports of JSR packages (for now) (#26530)
- fix(install): update lockfile when using package.json (#26458)
- fix(lsp): import-map-remap quickfix for type imports (#26454)
- fix(node/util): support array formats in `styleText` (#26507)
- fix(node:tls): set TLSSocket.alpnProtocol for client connections (#26476)
- fix(npm): ensure scoped package name is encoded in URLs (#26390)
- fix(npm): support version ranges with && or comma (#26453)
- fix: `.npmrc` settings not being passed to install/add command (#26473)
- fix: add 'fmt-component' to unstable features in schema file (#26526)
- fix: share inotify fd across watchers (#26200)
- fix: unpin tokio version (#26457)
- perf(compile): pass module source data from binary directly to v8 (#26494)
- perf: avoid multiple calls to runMicrotask (#26378)

### 2.0.2 / 2024.10.17

- fix(cli): set napi object property properly (#26344)
- fix(ext/node): add null check for kStreamBaseField (#26368)
- fix(install): don't attempt to cache specifiers that point to directories
  (#26369)
- fix(jupyter): fix panics for overslow subtraction (#26371)
- fix(jupyter): update to the new logo (#26353)
- fix(net): don't try to set nodelay on upgrade streams (#26342)
- fix(node/fs): copyFile with `COPYFILE_EXCL` should not throw if the
  destination doesn't exist (#26360)
- fix(node/http): normalize header names in `ServerResponse` (#26339)
- fix(runtime): send ws ping frames from inspector server (#26352)
- fix: don't warn on ignored signals on windows (#26332)

### 2.0.1 / 2024.10.16

- feat(lsp): "deno/didRefreshDenoConfigurationTree" notifications (#26215)
- feat(unstable): `--unstable-detect-cjs` for respecting explicit
  `"type": "commonjs"` (#26149)
- fix(add): create deno.json when running `deno add jsr:<pkg>` (#26275)
- fix(add): exact version should not have range `^` specifier (#26302)
- fix(child_process): map node `--no-warnings` flag to `--quiet` (#26288)
- fix(cli): add prefix to install commands in help (#26318)
- fix(cli): consolidate pkg parser for install & remove (#26298)
- fix(cli): named export takes precedence over default export in doc testing
  (#26112)
- fix(cli): improve deno info output for npm packages (#25906)
- fix(console/ext/repl): support using parseFloat() (#25900)
- fix(ext/console): apply coloring for console.table (#26280)
- fix(ext/napi): pass user context to napi_threadsafe_fn finalizers (#26229)
- fix(ext/node): allow writing to tty columns (#26201)
- fix(ext/node): compute pem length (upper bound) for key exports (#26231)
- fix(ext/node): fix dns.lookup result ordering (#26264)
- fix(ext/node): handle http2 server ending stream (#26235)
- fix(ext/node): implement TCP.setNoDelay (#26263)
- fix(ext/node): timingSafeEqual account for AB byteOffset (#26292)
- fix(ext/node): use primordials in `ext/node/polyfills/internal/buffer.mjs`
  (#24993)
- fix(ext/webgpu): allow GL backend on Windows (#26206)
- fix(install): duplicate dependencies in `package.json` (#26128)
- fix(install): handle pkg with dep on self when pkg part of peer dep resolution
  (#26277)
- fix(install): retry downloads of registry info / tarballs (#26278)
- fix(install): support installing npm package with alias (#26246)
- fix(jupyter): copy kernels icons to the kernel directory (#26084)
- fix(jupyter): keep running event loop when waiting for messages (#26049)
- fix(lsp): relative completions for bare import-mapped specifiers (#26137)
- fix(node): make `process.stdout.isTTY` writable (#26130)
- fix(node/util): export `styleText` from `node:util` (#26194)
- fix(npm): support `--allow-scripts` on `deno run` (and `deno add`,
  `deno test`, etc) (#26075)
- fix(repl): importing json files (#26053)
- fix(repl): remove check flags (#26140)
- fix(unstable/worker): ensure import permissions are passed (#26101)
- fix: add hint for missing `document` global in terminal error (#26218)
- fix: do not panic on wsl share file paths on windows (#26081)
- fix: do not panic running remote cjs module (#26259)
- fix: do not panic when using methods on classes and interfaces in deno doc
  html output (#26100)
- fix: improve suggestions and hints when using CommonJS modules (#26287)
- fix: node-api function call should use preamble (#26297)
- fix: panic in `prepare_stack_trace_callback` when global interceptor throws
  (#26241)
- fix: use syntect for deno doc html generation (#26322)
- perf(http): avoid clone getting request method and url (#26250)
- perf(http): cache webidl.converters lookups in ext/fetch/23_response.js
  (#26256)
- perf(http): make heap allocation for path conditional (#26289)
- perf: use fast calls for microtask ops (#26236)

### 2.0.0 / 2024.10.09

Read announcement blog post at: https://deno.com/blog/v2

- BREAKING: `DENO_FUTURE=1` by default, or welcome to Deno 2.0 (#25213)
- BREAKING: disallow `new Deno.FsFile()` (#25478)
- BREAKING: drop support for Deno.run.{clearEnv,gid,uid} (#25371)
- BREAKING: improve types for `Deno.serve` (#25369)
- BREAKING: improved error code accuracy (#25383)
- BREAKING: make supported compilerOptions an allow list (#25432)
- BREAKING: move `width` and `height` options to `UnsafeWindowSurface`
  constructor (#24200)
- BREAKING: remove --allow-hrtime (#25367)
- BREAKING: remove "emit" and "map" from deno info output (#25468)
- BREAKING: remove `--allow-none` flag (#25337)
- BREAKING: remove `--jobs` flag (#25336)
- BREAKING: remove `--trace-ops` (#25344)
- BREAKING: remove `--ts` flag (#25338)
- BREAKING: remove `--unstable` flag (#25522)
- BREAKING: remove `deno bundle` (#25339)
- BREAKING: remove `deno vendor` (#25343)
- BREAKING: remove `Deno.[Tls]Listener.prototype.rid` (#25556)
- BREAKING: remove `Deno.{Conn,TlsConn,TcpConn,UnixConn}.prototype.rid` (#25446)
- BREAKING: remove `Deno.{Reader,Writer}[Sync]` and `Deno.Closer` (#25524)
- BREAKING: remove `Deno.Buffer` (#25441)
- BREAKING: remove `Deno.close()` (#25347)
- BREAKING: remove `Deno.ConnectTlsOptions.{certChain,certFile,privateKey}` and
  `Deno.ListenTlsOptions.certChain,certFile,keyFile}` (#25525)
- BREAKING: remove `Deno.copy()` (#25345)
- BREAKING: remove `Deno.customInspect` (#25348)
- BREAKING: remove `Deno.fdatasync[Sync]()` (#25520)
- BREAKING: remove `Deno.File` (#25447)
- BREAKING: remove `Deno.flock[Sync]()` (#25350)
- BREAKING: remove `Deno.FsFile.prototype.rid` (#25499)
- BREAKING: remove `Deno.fstat[Sync]()` (#25351)
- BREAKING: remove `Deno.FsWatcher.prototype.rid` (#25444)
- BREAKING: remove `Deno.fsync[Sync]()` (#25448)
- BREAKING: remove `Deno.ftruncate[Sync]()` (#25412)
- BREAKING: remove `Deno.funlock[Sync]()` (#25442)
- BREAKING: remove `Deno.futime[Sync]()` (#25252)
- BREAKING: remove `Deno.iter[Sync]()` (#25346)
- BREAKING: remove `Deno.read[Sync]()` (#25409)
- BREAKING: remove `Deno.readAll[Sync]()` (#25386)
- BREAKING: remove `Deno.seek[Sync]()` (#25449)
- BREAKING: remove `Deno.Seeker[Sync]` (#25551)
- BREAKING: remove `Deno.shutdown()` (#25253)
- BREAKING: remove `Deno.write[Sync]()` (#25408)
- BREAKING: remove `Deno.writeAll[Sync]()` (#25407)
- BREAKING: remove deprecated `UnsafeFnPointer` constructor type with untyped
  `Deno.PointerObject` parameter (#25577)
- BREAKING: remove deprecated files config (#25535)
- BREAKING: Remove obsoleted Temporal APIs part 2 (#25505)
- BREAKING: remove remaining web types for compatibility (#25334)
- BREAKING: remove support for remote import maps in deno.json (#25836)
- BREAKING: rename "deps" remote cache folder to "remote" (#25969)
- BREAKING: soft-remove `Deno.isatty()` (#25410)
- BREAKING: soft-remove `Deno.run()` (#25403)
- BREAKING: soft-remove `Deno.serveHttp()` (#25451)
- BREAKING: undeprecate `Deno.FsWatcher.prototype.return()` (#25623)
- feat: add `--allow-import` flag (#25469)
- feat: Add a hint on error about 'Relative import path ... not prefixed with
  ...' (#25430)
- feat: Add better error messages for unstable APIs (#25519)
- feat: Add suggestion for packages using Node-API addons (#25975)
- feat: Allow importing .cjs files (#25426)
- feat: default to TS for file extension and support ext flag in more scenarios
  (#25472)
- feat: deprecate import assertions (#25281)
- feat: Don't warn about --allow-script when using esbuild (#25894)
- feat: hide several --unstable-* flags (#25378)
- feat: improve lockfile v4 to store normalized version constraints and be more
  terse (#25247)
- feat: improve warnings for deprecations and lifecycle script for npm packages
  (#25694)
- feat: include version number in all --json based outputs (#25335)
- feat: lockfile v4 by default (#25165)
- feat: make 'globalThis.location' a configurable property (#25812)
- feat: print `Listening on` messages on stderr instead of stdout (#25491)
- feat: remove `--lock-write` flag (#25214)
- feat: require jsr prefix for `deno install` and `deno add` (#25698)
- feat: require(esm) (#25501)
- feat: Show hints when using `window` global (#25805)
- feat: stabilize `Deno.createHttpClient()` (#25569)
- feat: suggest `deno install --entrypoint` instead of `deno cache` (#25228)
- feat: support DENO_LOG env var instead of RUST_LOG (#25356)
- feat: TypeScript 5.6 and `npm:@types/node@22` (#25614)
- feat: Update no-window lint rule (#25486)
- feat: update warning message for --allow-run with no list (#25693)
- feat: warn when using `--allow-run` with no allow list (#25215)
- feat(add): Add npm packages to package.json if present (#25477)
- feat(add): strip package subpath when adding a package (#25419)
- feat(add/install): Flag to add dev dependency to package.json (#25495)
- feat(byonm): support `deno run npm:<package>` when package is not in
  package.json (#25981)
- feat(check): turn on noImplicitOverride (#25695)
- feat(check): turn on useUnknownInCatchVariables (#25465)
- feat(cli): evaluate code snippets in JSDoc and markdown (#25220)
- feat(cli): give access to `process` global everywhere (#25291)
- feat(cli): use NotCapable error for permission errors (#25431)
- feat(config): Node modules option for 2.0 (#25299)
- feat(ext/crypto): import and export p521 keys (#25789)
- feat(ext/crypto): X448 support (#26043)
- feat(ext/kv): configurable limit params (#25174)
- feat(ext/node): add abort helpers, process & streams fix (#25262)
- feat(ext/node): add rootCertificates to node:tls (#25707)
- feat(ext/node): buffer.transcode() (#25972)
- feat(ext/node): export 'promises' symbol from 'node:timers' (#25589)
- feat(ext/node): export missing constants from 'zlib' module (#25584)
- feat(ext/node): export missing symbols from domain, puncode, repl, tls
  (#25585)
- feat(ext/node): export more symbols from streams and timers/promises (#25582)
- feat(ext/node): expose ES modules for _ modules (#25588)
- feat(flags): allow double commas to escape values in path based flags (#25453)
- feat(flags): support user provided args in repl subcommand (#25605)
- feat(fmt): better error on malfored HTML files (#25853)
- feat(fmt): stabilize CSS, HTML and YAML formatters (#25753)
- feat(fmt): support vto and njk extensions (#25831)
- feat(fmt): upgrade markup_fmt (#25768)
- feat(install): deno install with entrypoint (#25411)
- feat(install): warn repeatedly about not-run lifecycle scripts on explicit
  installs (#25878)
- feat(lint): add `no-process-global` lint rule (#25709)
- feat(lsp): add a message when someone runs 'deno lsp' manually (#26051)
- feat(lsp): auto-import types with 'import type' (#25662)
- feat(lsp): html/css/yaml file formatting (#25353)
- feat(lsp): quick fix for @deno-types="npm:@types/*" (#25954)
- feat(lsp): turn on useUnknownInCatchVariables (#25474)
- feat(lsp): unstable setting as list (#25552)
- feat(permissions): `Deno.mainModule` doesn't require permissions (#25667)
- feat(permissions): allow importing from cdn.jsdelivr.net by default (#26013)
- feat(serve): Support second parameter in deno serve (#25606)
- feat(tools/doc): display subitems in symbol overviews where applicable
  (#25885)
- feat(uninstall): alias to 'deno remove' if -g flag missing (#25461)
- feat(upgrade): better error message on failure (#25503)
- feat(upgrade): print info links for Deno 2 RC releases (#25225)
- feat(upgrade): support LTS release channel (#25123)
- fix: add link to env var docs (#25557)
- fix: add suggestion how to fix importing CJS module (#21764)
- fix: add test ensuring als works across dynamic import (#25593)
- fix: better error for Deno.UnsafeWindowSurface, correct HttpClient name,
  cleanup unused code (#25833)
- fix: cjs resolution cases (#25739)
- fix: consistent with deno_config and treat `"experimentalDecorators"` as
  deprecated (#25735)
- fix: delete old Deno 1.x headers file when loading cache (#25283)
- fix: do not panic running invalid file specifier (#25530)
- fix: don't include extensionless files in file collection for lint & fmt by
  default (#25721)
- fix: don't prompt when using `Deno.permissions.request` with `--no-prompt`
  (#25811)
- fix: eagerly error for specifier with empty version constraint (#25944)
- fix: enable `Win32_Security` feature in `windows-sys` (#26007)
- fix: error on unsupported compiler options (#25714)
- fix: error out if a valid flag is passed before a subcommand (#25830)
- fix: fix jupyter display function type (#25326)
- fix: Float16Array type (#25506)
- fix: handle showing warnings while the progress bar is shown (#25187)
- fix: Hide 'deno cache' from help output (#25960)
- fix: invalid ipv6 hostname on `deno serve` (#25482)
- fix: linux canonicalization checks (#24641)
- fix: lock down allow-run permissions more (#25370)
- fix: make some warnings more standard (#25324)
- fix: no cmd prefix in help output go links (#25459)
- fix: only enable byonm if workspace root has pkg json (#25379)
- fix: panic when require(esm) (#25769)
- fix: precompile preserve SVG camelCase attributes (#25945)
- fix: reland async context (#25140)
- fix: remove --allow-run warning when using deno without args or subcommand
  (#25684)
- fix: remove entrypoint hack for Deno 2.0 (#25332)
- fix: remove recently added deno.json node_modules aliasing (#25542)
- fix: remove the typo in the help message (#25962)
- fix: removed unstable-htttp from deno help (#25216)
- fix: replace `npm install` hint with `deno install` hint (#25244)
- fix: trim space around DENO_AUTH_TOKENS (#25147)
- fix: update deno_doc (#25290)
- fix: Update deno_npm to fix `deno install` with crossws (#25837)
- fix: update hint for `deno add <package>` (#25455)
- fix: update malva in deno to support astro css comments (#25553)
- fix: update nodeModulesDir config JSON schema (#25653)
- fix: update patchver to 0.2 (#25952)
- fix: update sui to 0.4 (#25942)
- fix: upgrade deno_ast 0.42 (#25313)
- fix: upgrade deno_core to 0.307.0 (#25287)
- fix(add/install): default to "latest" tag for npm packages in
  `deno add npm:pkg` (#25858)
- fix(bench): Fix table column alignments and NO_COLOR=1 (#25190)
- fix(BREAKING): make dns record types have consistent naming (#25357)
- fix(byonm): resolve npm deps of jsr deps (#25399)
- fix(check): ignore noImplicitOverrides in remote modules (#25854)
- fix(check): move is cjs check from resolving to loading (#25597)
- fix(check): properly surface dependency errors in types file of js file
  (#25860)
- fix(cli): `deno task` exit with status 0 (#25637)
- fix(cli): Default to auto with --node-modules-dir flag (#25772)
- fix(cli): handle edge cases around `export`s in doc tests and default export
  (#25720)
- fix(cli): Map error kind to `PermissionDenied` when symlinking fails due to
  permissions (#25398)
- fix(cli): Only set allow net flag for deno serve if not already allowed all
  (#25743)
- fix(cli): Warn on not-run lifecycle scripts with global cache (#25786)
- fix(cli/tools): correct `deno init --serve` template behavior (#25318)
- fix(compile): support 'deno compile' in RC and LTS releases (#25875)
- fix(config): validate export names (#25436)
- fix(coverage): ignore urls from doc testing (#25736)
- fix(doc): surface graph errors as warnings (#25888)
- fix(dts): stabilize `fetch` declaration for use with `Deno.HttpClient`
  (#25683)
- fix(ext/console): more precision in console.time (#25723)
- fix(ext/console): prevent duplicate error printing when the cause is assigned
  (#25327)
- fix(ext/crypto): ensure EC public keys are exported uncompressed (#25766)
- fix(ext/crypto): fix identity test for x25519 derive bits (#26011)
- fix(ext/crypto): reject empty usages in SubtleCrypto#importKey (#25759)
- fix(ext/crypto): support md4 digest algorithm (#25656)
- fix(ext/crypto): throw DataError for invalid EC key import (#25181)
- fix(ext/fetch): fix lowercase http_proxy classified as https (#25686)
- fix(ext/fetch): percent decode userinfo when parsing proxies (#25229)
- fix(ext/http): do not set localhost to hostname unnecessarily (#24777)
- fix(ext/http): gracefully handle Response.error responses (#25712)
- fix(ext/node): add `FileHandle#writeFile` (#25555)
- fix(ext/node): add `vm.constants` (#25630)
- fix(ext/node): Add missing `node:path` exports (#25567)
- fix(ext/node): Add missing node:fs and node:constants exports (#25568)
- fix(ext/node): add stubs for `node:trace_events` (#25628)
- fix(ext/node): attach console stream properties (#25617)
- fix(ext/node): avoid showing `UNKNOWN` error from TCP handle (#25550)
- fix(ext/node): close upgraded socket when the underlying http connection is
  closed (#25387)
- fix(ext/node): delay accept() call 2 ticks in net.Server#listen (#25481)
- fix(ext/node): don't throw error for unsupported signal binding on windows
  (#25699)
- fix(ext/node): emit `online` event after worker thread is initialized (#25243)
- fix(ext/node): export `process.allowedNodeEnvironmentFlags` (#25629)
- fix(ext/node): export JWK public key (#25239)
- fix(ext/node): export request and response clases from `http2` module (#25592)
- fix(ext/node): fix `Cipheriv#update(string, undefined)` (#25571)
- fix(ext/node): fix Decipheriv when autoPadding disabled (#25598)
- fix(ext/node): fix process.stdin.pause() (#25864)
- fix(ext/node): Fix vm sandbox object panic (#24985)
- fix(ext/node): http2session ready state (#25143)
- fix(ext/node): Implement detached option in `child_process` (#25218)
- fix(ext/node): import EC JWK keys (#25266)
- fix(ext/node): import JWK octet key pairs (#25180)
- fix(ext/node): import RSA JWK keys (#25267)
- fix(ext/node): register `node:wasi` built-in (#25134)
- fix(ext/node): remove unimplemented promiseHook stubs (#25979)
- fix(ext/node): report freemem() on Linux in bytes (#25511)
- fix(ext/node): Rewrite `node:v8` serialize/deserialize (#25439)
- fix(ext/node): session close during stream setup (#25170)
- fix(ext/node): Stream should be instance of EventEmitter (#25527)
- fix(ext/node): stub `inspector/promises` (#25635)
- fix(ext/node): stub `process.cpuUsage()` (#25462)
- fix(ext/node): stub cpu_info() for OpenBSD (#25807)
- fix(ext/node): support x509 certificates in `createPublicKey` (#25731)
- fix(ext/node): throw when loading `cpu-features` module (#25257)
- fix(ext/node): update aead-gcm-stream to 0.3 (#25261)
- fix(ext/node): use primordials in `ext/node/polyfills/console.ts` (#25572)
- fix(ext/node): use primordials in ext/node/polyfills/wasi.ts (#25608)
- fix(ext/node): validate input lengths in `Cipheriv` and `Decipheriv` (#25570)
- fix(ext/web): don't ignore capture in EventTarget.removeEventListener (#25788)
- fix(ext/webgpu): allow to build on unsupported platforms (#25202)
- fix(ext/webgpu): sync category comment (#25580)
- fix(ext/webstorage): make `getOwnPropertyDescriptor` with symbol return
  `undefined` (#13348)
- fix(flags): --allow-all should conflict with lower permissions (#25909)
- fix(flags): don't treat empty run command as task subcommand (#25708)
- fix(flags): move some content from docs.deno.com into help output (#25951)
- fix(flags): properly error out for urls (#25770)
- fix(flags): require global flag for permission flags in install subcommand
  (#25391)
- fix(fmt): --check was broken for CSS, YAML and HTML (#25848)
- fix(fmt): fix incorrect quotes in components (#25249)
- fix(fmt): fix tabs in YAML (#25536)
- fix(fmt/markdown): fix regression with multi-line footnotes and inline math
  (#25222)
- fix(info): error instead of panic for npm specifiers when using byonm (#25947)
- fix(info): move "version" field to top of json output (#25890)
- fix(inspector): Fix panic when re-entering runtime ops (#25537)
- fix(install): compare versions directly to decide whether to create a child
  node_modules dir for a workspace member (#26001)
- fix(install): Make sure target node_modules exists when symlinking (#25494)
- fix(install): recommend using `deno install -g` when using a single http url
  (#25388)
- fix(install): store tags associated with package in node_modules dir (#26000)
- fix(install): surface package.json dependency errors (#26023)
- fix(install): Use relative symlinks in deno install (#25164)
- fix(installl): make bin entries executable even if not put in
  `node_modules/.bin` (#25873)
- fix(jupyter): allow unstable flags (#25483)
- fix(lint): correctly handle old jsx in linter (#25902)
- fix(lint): support linting jsr pkg without version field (#25230)
- fix(lockfile): use loose deserialization for version constraints (#25660)
- fix(lsp): encode url parts before parsing as uri (#25509)
- fix(lsp): exclude missing import quick fixes with bad resolutions (#26025)
- fix(lsp): panic on url_to_uri() (#25238)
- fix(lsp): properly resolve jsxImportSource for caching (#25688)
- fix(lsp): update diagnostics on npm install (#25352)
- fix(napi): Don't run microtasks in napi_resolve_deferred (#25246)
- fix(napi): Fix worker threads importing already-loaded NAPI addon (#25245)
- fix(no-slow-types): better `override` handling (#25989)
- fix(node): Don't error out if we fail to statically analyze CJS re-export
  (#25748)
- fix(node): fix worker_threads issues blocking Angular support (#26024)
- fix(node): implement libuv APIs needed to support `npm:sqlite3` (#25893)
- fix(node): Include "node" condition during CJS re-export analysis (#25785)
- fix(node): Pass NPM_PROCESS_STATE to subprocesses via temp file instead of env
  var (#25896)
- fix(node/byonm): do not accidentally resolve bare node built-ins (#25543)
- fix(node/cluster): improve stubs to make log4js work (#25146)
- fix(npm): better error handling for remote npm deps (#25670)
- fix(npm): root package has peer dependency on itself (#26022)
- fix(permissions): disallow any `LD_` or `DYLD_` prefixed env var without full
  --allow-run permissions (#25271)
- fix(permissions): disallow launching subprocess with LD_PRELOAD env var
  without full run permissions (#25221)
- fix(publish): ensure provenance is spec compliant (#25200)
- fix(regression): do not expose resolved path in Deno.Command permission denied
  error (#25434)
- fix(runtime): don't error `child.output()` on consumed stream (#25657)
- fix(runtime): use more null proto objects again (#25040)
- fix(runtime/web_worker): populate `SnapshotOptions` for `WebWorker` when
  instantiated without snapshot (#25280)
- fix(task): correct name for scoped npm package binaries (#25390)
- fix(task): support tasks with colons in name in `deno run` (#25233)
- fix(task): use current executable for deno even when not named deno (#26019)
- fix(types): simplify mtls related types (#25658)
- fix(upgrade): more informative information on invalid version (#25319)
- fix(windows): Deno.Command - align binary resolution with linux and mac
  (#25429)
- fix(workspace): handle when config has members when specified via --config
  (#25988)
- perf: fast path for cached dyn imports (#25636)
- perf: Use -O3 for sui in release builds (#26010)
- perf(cache): single cache file for remote modules (#24983)
- perf(cache): single cache file for typescript emit (#24994)
- perf(ext/fetch): improve decompression throughput by upgrading `tower_http`
  (#25806)
- perf(ext/node): reduce some allocations in require (#25197)
- perf(ext/web): optimize performance.measure() (#25774)

### 1.46.3 / 2024.09.04

- feat(upgrade): print info links for Deno 2 RC releases (#25225)
- fix(cli): Map error kind to `PermissionDenied` when symlinking fails due to
  permissions (#25398)
- fix(cli/tools): correct `deno init --serve` template behavior (#25318)
- fix(ext/node): session close during stream setup (#25170)
- fix(publish): ensure provenance is spec compliant (#25200)
- fix(upgrade): more informative information on invalid version (#25319)
- fix: fix jupyter display function type (#25326)

### 1.46.2 / 2024.08.29

- Revert "feat(fetch): accept async iterables for body" (#25207)
- fix(bench): Fix table column alignments and NO_COLOR=1 (#25190)
- fix(ext/crypto): throw DataError for invalid EC key import (#25181)
- fix(ext/fetch): percent decode userinfo when parsing proxies (#25229)
- fix(ext/node): emit `online` event after worker thread is initialized (#25243)
- fix(ext/node): export JWK public key (#25239)
- fix(ext/node): import EC JWK keys (#25266)
- fix(ext/node): import JWK octet key pairs (#25180)
- fix(ext/node): import RSA JWK keys (#25267)
- fix(ext/node): throw when loading `cpu-features` module (#25257)
- fix(ext/node): update aead-gcm-stream to 0.3 (#25261)
- fix(ext/webgpu): allow to build on unsupported platforms (#25202)
- fix(fmt): fix incorrect quotes in components (#25249)
- fix(fmt/markdown): fix regression with multi-line footnotes and inline math
  (#25222)
- fix(install): Use relative symlinks in deno install (#25164)
- fix(lsp): panic on url_to_uri() (#25238)
- fix(napi): Don't run microtasks in napi_resolve_deferred (#25246)
- fix(napi): Fix worker threads importing already-loaded NAPI addon (#25245)
- fix(node/cluster): improve stubs to make log4js work (#25146)
- fix(runtime/web_worker): populate `SnapshotOptions` for `WebWorker` when
  instantiated without snapshot (#25280)
- fix(task): support tasks with colons in name in `deno run` (#25233)
- fix: handle showing warnings while the progress bar is shown (#25187)
- fix: reland async context (#25140)
- fix: removed unstable-htttp from deno help (#25216)
- fix: replace `npm install` hint with `deno install` hint (#25244)
- fix: update deno_doc (#25290)
- fix: upgrade deno_core to 0.307.0 (#25287)
- perf(ext/node): reduce some allocations in require (#25197)

### 1.46.1 / 2024.08.22

- fix(ext/node): http2session ready state (#25143)
- fix(ext/node): register `node:wasi` built-in (#25134)
- fix(urlpattern): fallback to empty string for undefined group values (#25151)
- fix: trim space around DENO_AUTH_TOKENS (#25147)

### 1.46.0 / 2024.08.22

- BREAKING(temporal/unstable): Remove obsoleted Temporal APIs (#24836)
- BREAKING(webgpu/unstable): Replace async .requestAdapterInfo() with sync .info
  (#24783)
- feat: `deno compile --icon <ico>` (#25039)
- feat: `deno init --serve` (#24897)
- feat: `deno upgrade --rc` (#24905)
- feat: Add Deno.ServeDefaultExport type (#24879)
- feat: async context (#24402)
- feat: better help output (#24958)
- feat: codesign for deno compile binaries (#24604)
- feat: deno clean (#24950)
- feat: deno remove (#24952)
- feat: deno run <task> (#24891)
- feat: Deprecate "import assertions" with a warning (#24743)
- feat: glob and directory support for `deno check` and `deno cache` cli arg
  paths (#25001)
- feat: Print deprecation message for npm packages (#24992)
- feat: refresh "Download" progress bar with a spinner (#24913)
- feat: Rename --unstable-hmr to --watch-hmr (#24975)
- feat: support short flags for permissions (#24883)
- feat: treat bare deno command with run arguments as deno run (#24887)
- feat: upgrade deno_core (#24886)
- feat: upgrade deno_core (#25042)
- feat: upgrade V8 to 12.8 (#24693)
- feat: Upgrade V8 to 12.9 (#25138)
- feat: vm rewrite (#24596)
- feat(clean): add progress bar (#25026)
- feat(cli): Add --env-file as alternative to --env (#24555)
- feat(cli/tools): add a subcommand `--hide-stacktraces` for test (#24095)
- feat(config): Support frozen lockfile config option in deno.json (#25100)
- feat(config/jsr): add license field (#25056)
- feat(coverage): add breadcrumbs to deno coverage `--html` report (#24860)
- feat(ext/node): rewrite crypto keys (#24463)
- feat(ext/node): support http2session.socket (#24786)
- feat(fetch): accept async iterables for body (#24623)
- feat(flags): improve help output and make `deno run` list tasks (#25108)
- feat(fmt): support CSS, SCSS, Sass and Less (#24870)
- feat(fmt): support HTML, Svelte, Vue, Astro and Angular (#25019)
- feat(fmt): support YAML (#24717)
- feat(FUTURE): terse lockfile (v4) (#25059)
- feat(install): change 'Add ...' message (#24949)
- feat(lint): Add lint for usage of node globals (with autofix) (#25048)
- feat(lsp): node specifier completions (#24904)
- feat(lsp): registry completions for import-mapped specifiers (#24792)
- feat(node): support `username` and `_password` in `.npmrc` file (#24793)
- feat(permissions): link to docs in permission prompt (#24948)
- feat(publish): error on missing license file (#25011)
- feat(publish): suggest importing `jsr:@std/` for `deno.land/std` urls (#25046)
- feat(serve): Opt-in parallelism for `deno serve` (#24920)
- feat(test): rename --allow-none to --permit-no-files (#24809)
- feat(unstable): ability to use a local copy of jsr packages (#25068)
- feat(unstable/fmt): move yaml formatting behind unstable flag (#24848)
- feat(upgrade): refresh output (#24911)
- feat(upgrade): support `deno upgrade 1.46.0` (#25096)
- feat(urlpattern): add ignoreCase option & hasRegExpGroups property, and fix
  spec discrepancies (#24741)
- feat(watch): add watch paths to test subcommand (#24771)
- fix: `node:inspector` not being registered (#25007)
- fix: `rename` watch event missing (#24893)
- fix: actually add missing `node:readline/promises` module (#24772)
- fix: adapt to new jupyter runtime API and include session IDs (#24762)
- fix: add permission name when accessing a special file errors (#25085)
- fix: adjust suggestion for lockfile regeneration (#25107)
- fix: cache bust jsr meta file when version not found in dynamic branches
  (#24928)
- fix: CFunctionInfo and CTypeInfo leaks (#24634)
- fix: clean up flag help output (#24686)
- fix: correct JSON config schema to show vendor option as stable (#25090)
- fix: dd-trace http message compat (#25021)
- fix: deserialize lockfile v3 straight (#25121)
- fix: Don't panic if fail to handle JS stack frame (#25122)
- fix: Don't panic if failed to add system certificate (#24823)
- fix: Don't shell out to `unzip` in deno upgrade/compile (#24926)
- fix: enable the reporting of parsing related problems when running deno lint
  (#24332)
- fix: errors with CallSite methods (#24907)
- fix: include already seen deps in lockfile dep tracking (#24556)
- fix: log current version when using deno upgrade (#25079)
- fix: make `deno add` output more deterministic (#25083)
- fix: make vendor cache manifest more deterministic (#24658)
- fix: missing `emitWarning` import (#24587)
- fix: regressions around Error.prepareStackTrace (#24839)
- fix: stub `node:module.register()` (#24965)
- fix: support `npm:bindings` and `npm:callsites` packages (#24727)
- fix: unblock fsevents native module (#24542)
- fix: update deno_doc (#24972)
- fix: update dry run success message (#24885)
- fix: update lsp error message of 'relative import path' to 'use deno add' for
  npm/jsr packages (#24524)
- fix: upgrade deno_core to 0.298.0 (#24709)
- fix: warn about import assertions when using typescript (#25135)
- fix(add): better error message providing scoped pkg missing leading `@` symbol
  (#24961)
- fix(add): Better error message when missing npm specifier (#24970)
- fix(add): error when config file contains importMap field (#25115)
- fix(add): Handle packages without root exports (#25102)
- fix(add): Support dist tags in deno add (#24960)
- fix(cli): add NAPI support in standalone mode (#24642)
- fix(cli): Create child node_modules for conflicting dependency versions,
  respect aliases in package.json (#24609)
- fix(cli): Respect implied BYONM from DENO_FUTURE in `deno task` (#24652)
- fix(cli): shorten examples in help text (#24374)
- fix(cli): support --watch when running cjs npm packages (#25038)
- fix(cli): Unhide publish subcommand help string (#24787)
- fix(cli): update permission prompt message for compiled binaries (#24081)
- fix(cli/init): broken link in deno init sample template (#24545)
- fix(compile): adhoc codesign mach-o by default (#24824)
- fix(compile): make output more deterministic (#25092)
- fix(compile): support workspace members importing other members (#24909)
- fix(compile/windows): handle cjs re-export of relative path with parent
  component (#24795)
- fix(config): regression - should not discover npm workspace for nested
  deno.json not in workspace (#24559)
- fix(cron): improve error message for invalid cron names (#24644)
- fix(docs): fix some deno.land/manual broken urls (#24557)
- fix(ext/console): Error Cause Not Inspect-Formatted when printed (#24526)
- fix(ext/console): render properties of Intl.Locale (#24827)
- fix(ext/crypto): respect offsets when writing into ab views in randomFillSync
  (#24816)
- fix(ext/fetch): include TCP src/dst socket info in error messages (#24939)
- fix(ext/fetch): include URL and error details on fetch failures (#24910)
- fix(ext/fetch): respect authority from URL (#24705)
- fix(ext/fetch): use correct ALPN to proxies (#24696)
- fix(ext/fetch): use correct ALPN to socks5 proxies (#24817)
- fix(ext/http): correctly consume response body in `Deno.serve` (#24811)
- fix(ext/net): validate port in Deno.{connect,serve,listen} (#24399)
- fix(ext/node): add `CipherIv.setAutoPadding()` (#24940)
- fix(ext/node): add crypto.diffieHellman (#24938)
- fix(ext/node): client closing streaming request shouldn't terminate http
  server (#24946)
- fix(ext/node): createBrotliCompress params (#24984)
- fix(ext/node): do not expose `self` global in node (#24637)
- fix(ext/node): don't concat set-cookie in ServerResponse.appendHeader (#25000)
- fix(ext/node): don't throw when calling PerformanceObserver.observe (#25036)
- fix(ext/node): ed25519 signing and cipheriv autopadding fixes (#24957)
- fix(ext/node): fix prismjs compatibiliy in Web Worker (#25062)
- fix(ext/node): handle node child_process with --v8-options flag (#24804)
- fix(ext/node): handle prefix mapping for IPv4-mapped IPv6 addresses (#24546)
- fix(ext/node): http request uploads of subarray of buffer should work (#24603)
- fix(ext/node): improve shelljs compat with managed npm execution (#24912)
- fix(ext/node): node:zlib coerces quality 10 to 9.5 (#24850)
- fix(ext/node): pass content-disposition header as string instead of bytes
  (#25128)
- fix(ext/node): prevent panic in http2.connect with uppercase header names
  (#24780)
- fix(ext/node): read correct CPU usage stats on Linux (#24732)
- fix(ext/node): rewrite X509Certificate resource and add `publicKey()` (#24988)
- fix(ext/node): stat.mode on windows (#24434)
- fix(ext/node): support ieee-p1363 ECDSA signatures and pss salt len (#24981)
- fix(ext/node): use pem private keys in createPublicKey (#24969)
- fix(ext/node/net): emit `error` before `close` when connection is refused
  (#24656)
- fix(ext/web): make CompressionResource garbage collectable (#24884)
- fix(ext/web): make TextDecoderResource use cppgc (#24888)
- fix(ext/webgpu): assign missing `constants` property of shader about
  `GPUDevice.createRenderPipeline[Async]` (#24803)
- fix(ext/webgpu): don't crash while constructing GPUOutOfMemoryError (#24807)
- fix(ext/webgpu): GPUDevice.createRenderPipelineAsync should return a Promise
  (#24349)
- fix(ext/websocket): unhandled close rejection in WebsocketStream (#25125)
- fix(fmt): handle using stmt in for of stmt (#24834)
- fix(fmt): regression with pipe in code blocks in tables (#25098)
- fix(fmt): upgrade to dprint-plugin-markdown 0.17.4 (#25075)
- fix(fmt): was sometimes putting comments in front of commas in parameter lists
  (#24650)
- fix(future): Emit `deno install` warning less often, suggest `deno install` in
  error message (#24706)
- fix(http): Adjust hostname display for Windows when using 0.0.0.0 (#24698)
- fix(init): use bare specifier for `jsr:@std/assert` (#24581)
- fix(install): Properly handle dist tags when setting up node_modules (#24968)
- fix(lint): support linting tsx/jsx from stdin (#24955)
- fix(lsp): directly use file referrer when loading document (#24997)
- fix(lsp): don't always use byonm resolver when DENO_FUTURE=1 (#24865)
- fix(lsp): hang when caching failed (#24651)
- fix(lsp): import map lookup for jsr subpath auto import (#25025)
- fix(lsp): include scoped import map keys in completions (#25047)
- fix(lsp): resolve jsx import source with types mode (#25064)
- fix(lsp): rewrite import for 'infer return type' action (#24685)
- fix(lsp): scope attribution for asset documents (#24663)
- fix(lsp): support npm workspaces and fix some resolution issues (#24627)
- fix(node): better detection for when to surface node resolution errors
  (#24653)
- fix(node): cjs pkg dynamically importing esm-only pkg fails (#24730)
- fix(node): Create additional pipes for child processes (#25016)
- fix(node): Fix `--allow-scripts` with no `deno.json` (#24533)
- fix(node): Fix node IPC serialization for objects with undefined values
  (#24894)
- fix(node): revert invalid package target change (#24539)
- fix(node): Rework node:child_process IPC (#24763)
- fix(node): Run node compat tests listed in the `ignore` field (and fix the
  ones that fail) (#24631)
- fix(node): support `tty.hasColors()` and `tty.getColorDepth()` (#24619)
- fix(node): support wildcards in package.json imports (#24794)
- fix(node/crypto): Assign publicKey and privateKey with let instead of const
  (#24943)
- fix(node/fs): node:fs.read and write should accept typed arrays other than
  Uint8Array (#25030)
- fix(node/fs): Use correct offset and length in node:fs.read and write (#25049)
- fix(node/fs/promises): watch should be async iterable (#24805)
- fix(node/http): wrong `req.url` value (#25081)
- fix(node/inspector): Session constructor should not throw (#25041)
- fix(node/timers/promises): add scheduler APIs (#24802)
- fix(node/tty): fix `tty.WriteStream.hasColor` with different args (#25094)
- fix(node/util): add missing `debug` alias of `debuglog` (#24944)
- fix(node/worker_threads): support `port.once()` (#24725)
- fix(npm): handle packages with only pre-released 0.0.0 versions (#24563)
- fix(npm): use start directory deno.json as "root deno.json config" in npm
  workspace (#24538)
- fix(npmrc): skip loading .npmrc in home dir on permission error (#24758)
- fix(publish): show dirty files on dirty check failure (#24541)
- fix(publish): surface syntax errors when using --no-check (#24620)
- fix(publish): warn about missing license file (#24677)
- fix(publish): workspace included license file had incorrect path (#24747)
- fix(repl): Prevent panic on broken pipe (#21945)
- fix(runtime/windows): fix calculation of console size (#23873)
- fix(std/http2): release window capacity back to remote stream (#24576)
- fix(tls): print a warning if a system certificate can't be loaded (#25023)
- fix(types): Conform lib.deno_web.d.ts to lib.dom.d.ts and lib.webworker.d.ts
  (#24599)
- fix(types): fix streams types (#24770)
- fix(unstable): move sloppy-import warnings to lint rule (#24710)
- fix(unstable): panic when running deno install with DENO_FUTURE=1 (#24866)
- fix(unstable/compile): handle byonm import in sub dir (#24755)
- fix(upgrade): better error message when check_exe fails (#25133)
- fix(upgrade): correctly compute latest version based on current release
  channel (#25087)
- fix(upgrade): do not error if config in cwd invalid (#24689)
- fix(upgrade): fallback to Content-Length header for progress bar (#24923)
- fix(upgrade): return no RC versions if fetching fails (#25013)
- fix(upgrade): support RC release with --version flag (#25091)
- fix(upgrade): use proper version display (#25029)
- fix(urlpattern): correct typings for added APIs (#24881)
- fix(webgpu): Fix `GPUAdapter#isFallbackAdapter` and `GPUAdapter#info`
  properties (#24914)
- fix(workspace): do not resolve to self for npm pkg depending on matching req
  (#24591)
- fix(workspace): support resolving bare specifiers to npm pkgs within a
  workspace (#24611)
- fix(workspaces/publish): include the license file from the workspace root if
  not in pkg (#24714)
- perf: skip saving to emit cache after first failure (#24896)
- perf: update deno_ast to 0.41 (#24819)
- perf: update deno_doc (#24700)
- perf(ext/crypto): make randomUUID() 5x faster (#24510)
- perf(ext/fetch): speed up `resp.clone()` (#24812)
- perf(ext/http): Reduce size of `ResponseBytesInner` (#24840)
- perf(ext/node): improve `Buffer` from string performance (#24567)
- perf(ext/node): optimize fs.exists[Sync] (#24613)
- perf(lsp): remove fallback config scopes for workspace folders (#24868)
- refactor: `version` module exports a single const struct (#25014)
- refactor: decouple node resolution from deno_core (#24724)
- refactor: move importMap with imports/scopes diagnostic to deno_config
  (#24553)
- refactor: remove version::is_canary(), use ReleaseChannel instead (#25053)
- refactor: show release channel in `deno --version` (#25061)
- refactor: update to deno_config 0.25 (#24645)
- refactor: update to use deno_package_json (#24688)
- refactor(ext/node): create separate ops for node:http module (#24788)
- refactor(fetch): reimplement fetch with hyper instead of reqwest (#24237)
- refactor(lint): move reporters to separate module (#24757)
- refactor(node): internally add `.code()` to node resolution errors (#24610)
- refactor(upgrade): cleanup pass (#24954)
- refactor(upgrade): make fetching latest version async (#24919)
- Reland "fix: CFunctionInfo and CTypeInfo leaks (#24634)" (#24692)
- Reland "refactor(fetch): reimplement fetch with hyper instead of reqwest"
  (#24593)

### 1.45.5 / 2024.07.31

- fix(cli): Unhide publish subcommand help string (#24787)
- fix(compile/windows): handle cjs re-export of relative path with parent
  component (#24795)
- fix(ext/node): handle node child_process with --v8-options flag (#24804)
- fix(ext/node): prevent panic in http2.connect with uppercase header names
  (#24780)
- fix(ext/webgpu): don't crash while constructing GPUOutOfMemoryError (#24807)
- fix(http): Adjust hostname display for Windows when using 0.0.0.0 (#24698)
- fix(node): Rework node:child_process IPC (#24763)
- fix(node): support wildcards in package.json imports (#24794)
- fix(node/fs/promises): watch should be async iterable (#24805)
- fix(node/timers/promises): add scheduler APIs (#24802)
- fix(npmrc): skip loading .npmrc in home dir on permission error (#24758)
- fix(types): fix streams types (#24770)
- fix(unstable/compile): handle byonm import in sub dir (#24755)
- fix: actually add missing `node:readline/promises` module (#24772)
- fix: adapt to new jupyter runtime API and include session IDs (#24762)
- perf(ext/fetch): speed up `resp.clone()` (#24812)
- perf(ext/node): improve `Buffer` from string performance (#24567)

### 1.45.4 / 2024.07.26

- Reland "fix: CFunctionInfo and CTypeInfo leaks (#24634)" (#24692)
- fix(ext/fetch): respect authority from URL (#24705)
- fix(ext/fetch): use correct ALPN to proxies (#24696)
- fix(ext/node): read correct CPU usage stats on Linux (#24732)
- fix(ext/node/net): emit `error` before `close` when connection is refused
  (#24656)
- fix(future): Emit `deno install` warning less often, suggest `deno install` in
  error message (#24706)
- fix(lsp): rewrite import for 'infer return type' action (#24685)
- fix(node): better detection for when to surface node resolution errors
  (#24653)
- fix(node): cjs pkg dynamically importing esm-only pkg fails (#24730)
- fix(node/worker_threads): support `port.once()` (#24725)
- fix(publish): workspace included license file had incorrect path (#24747)
- fix(unstable): move sloppy-import warnings to lint rule (#24710)
- fix(upgrade): do not error if config in cwd invalid (#24689)
- fix(workspaces/publish): include the license file from the workspace root if
  not in pkg (#24714)
- fix: enable the reporting of parsing related problems when running deno lint
  (#24332)
- fix: support `npm:bindings` and `npm:callsites` packages (#24727)
- fix: update lsp error message of 'relative import path' to 'use deno add' for
  npm/jsr packages (#24524)
- fix: decode percent-encoding source string in `Error.stack` (#24709)
- perf: update deno_doc (#24700)

### 1.45.3 / 2024.07.22

- Reland "refactor(fetch): reimplement fetch with hyper instead of reqwest"
  (#24593)
- fix(cli): Create child node_modules for conflicting dependency versions,
  respect aliases in package.json (#24609)
- fix(cli): Respect implied BYONM from DENO_FUTURE in `deno task` (#24652)
- fix(cli): add NAPI support in standalone mode (#24642)
- fix(cron): improve error message for invalid cron names (#24644)
- fix(docs): fix some deno.land/manual broken urls (#24557)
- fix(ext/console): Error Cause Not Inspect-Formatted when printed (#24526)
- fix(ext/node): do not expose `self` global in node (#24637)
- fix(ext/node): http request uploads of subarray of buffer should work (#24603)
- fix(ext/node): stat.mode on windows (#24434)
- fix(fmt): was sometimes putting comments in front of commas in parameter lists
  (#24650)
- fix(init): use bare specifier for `jsr:@std/assert` (#24581)
- fix(lsp): hang when caching failed (#24651)
- fix(lsp): scope attribution for asset documents (#24663)
- fix(lsp): support npm workspaces and fix some resolution issues (#24627)
- fix(node): Fix `--allow-scripts` with no `deno.json` (#24533)
- fix(node): support `tty.hasColors()` and `tty.getColorDepth()` (#24619)
- fix(npm): handle packages with only pre-released 0.0.0 versions (#24563)
- fix(publish): surface syntax errors when using --no-check (#24620)
- fix(publish): warn about missing license file (#24677)
- fix(std/http2): release window capacity back to remote stream (#24576)
- fix(types): Conform lib.deno_web.d.ts to lib.dom.d.ts and lib.webworker.d.ts
  (#24599)
- fix(workspace): do not resolve to self for npm pkg depending on matching req
  (#24591)
- fix(workspace): support resolving bare specifiers to npm pkgs within a
  workspace (#24611)
- fix: make vendor cache manifest more deterministic (#24658)
- fix: missing `emitWarning` import (#24587)
- perf(ext/node): optimize fs.exists[Sync] (#24613)

### 1.45.2 / 2024.07.12

- fix(cli/init): broken link in deno init sample template (#24545)
- fix(config): regression - should not discover npm workspace for nested
  deno.json not in workspace (#24559)
- fix(ext/node): handle prefix mapping for IPv4-mapped IPv6 addresses (#24546)
- fix(ext/webgpu): GPUDevice.createRenderPipelineAsync should return a Promise
  (#24349)
- fix(node): revert invalid package target change (#24539)
- fix(publish): show dirty files on dirty check failure (#24541)
- fix: include already seen deps in lockfile dep tracking (#24556)
- fix: unblock fsevents native module (#24542)
- perf(ext/crypto): make randomUUID() 5x faster (#24510)

### 1.45.1 / 2024.07.11

- fix(node): Ignore broken default install scripts (#24534)
- fix(npm): only warn about lifecycle scripts not being run when setting up
  directory (#24530)
- fix(workspace): allow using --import-map flag with workspace (#24527)

### 1.45.0 / 2024.07.10

- BREAKING(unstable/ffi): remove callback reentrant flag (#24367)
- feat(cli): Add `--frozen` flag to error out if lockfile is out of date
  (#24355)
- feat(cli): `deno init --lib` (#22499)
- feat(compile): support `--env` (#24166)
- feat(ext/crypto): make `deriveBits` `length` parameter optional and nullable
  (#24426)
- feat(ext/web): add `Blob.prototype.bytes()` (#24148)
- feat(jsr): support publishing jsr packages in npm workspaces (#24507)
- feat(jupyter): support `confirm` and `prompt` in notebooks (#23592)
- feat(lsp): ts language service scopes (#24345)
- feat(node): Support executing npm package lifecycle scripts
  (preinstall/install/postinstall) (#24487)
- feat(workspace): support object config (#24483)
- feat: Deprecate `--lock-write` flag (#24436)
- feat: Upgrade to TypeScript 5.5.2 (#24326)
- feat: add `__tests__` to test file detection defaults (#24443)
- feat: deprecate `deno vendor` (#22183)
- feat: npm workspace and better Deno workspace support (#24334)
- feat: support wildcards in npm workspaces (#24471)
- feat: upgrade deno_core (#24364)
- feat: upgrade deno_core to 0.293.0 (#24482)
- fix(check): CJS types importing dual ESM/CJS package should prefer CJS types
  (#24492)
- fix(compile): atomically write compile output (#24378)
- fix(compile): prevent setting unstable feature twice (#24381)
- fix(ext/node): Add `fs.lutimes` / `fs.lutimesSync` (#23172)
- fix(ext/node): add `Module.parent` (#24333)
- fix(ext/node): add ServerResponse#appendHeader (#24216)
- fix(ext/node): add Symbol.toStringTag to KeyObject instances (#24377)
- fix(ext/node): discover .npmrc in user's homedir (#24021)
- fix(ext/node): don't panic on invalid utf-8 in pem (#24303)
- fix(ext/node): don't wait for end() call to send http client request (#24390)
- fix(ext/node): http chunked writes hangs (#24428)
- fix(ext/node): ignore stream error during enqueue (#24243)
- fix(ext/node): make next tick queue resilient to `Array.prototype` tampering
  (#24361)
- fix(ext/node): rewrite `crypto.Hash` (#24302)
- fix(ext/node): rewrite digest handling (#24392)
- fix(ext/node): use cppgc for node:zlib (#24267)
- fix(ext/webgpu): fix `GPUUncapturedErrorEvent` parent type (#24369)
- fix(ext/websocket): drop connection when close frame not ack (#24301)
- fix(lsp): correct scope attribution for injected @types/node (#24404)
- fix(lsp): do sloppy resolution for node-to-node imports in byonm (#24481)
- fix(lsp): don't use global cache paths for scope allocation (#24353)
- fix(lsp): inherit workspace-root-only fields in members (#24440)
- fix(lsp): respect lockfile redirects entries for resolution (#24365)
- fix(lsp): use CliLockfile (#24387)
- fix(net): handle panic on Windows for Unix socket usage in Deno.serve()
  (#24423)
- fix(net): set correct max size for Datagram (#21611)
- fix(node): Implement `fs.lchown` (and `process.getegid`) (#24418)
- fix(node): add missing readline/promises module (#24336)
- fix(node/assert): throws not checking error instance (#24466)
- fix(node/http): don't error if request destroyed before send (#24497)
- fix(node/http): don't send destroyed requests (#24498)
- fix(node/http): don't throw on .address() before .listen() (#24432)
- fix(node/http): support all `.writeHead()` signatures (#24469)
- fix(node/perf_hooks): stub eventLoopUtilization (#24501)
- fix(node/v8): stub serializer methods (#24502)
- fix(permissions): handle ipv6 addresses correctly (#24397)
- fix(publish): unfurling should always be done with the package json (#24435)
- fix(task): do not propagate env changes outside subshells (#24279)
- fix(windows): check USERPROFILE env var for finding home directory (#24384)
- fix(workspace): better cli file argument handling (#24447)
- fix: Add sys permission kinds for node compat (#24242)
- fix: add warning for invalid unstable feature use in deno.json/jsonc (#24120)
- fix: do not download compilerOptions -> types when not type checking (#24473)
- fix: do not return undefined for missing global properties (#24474)
- fix: make .setup-cache.bin in node_modules more reproducible (#24480)
- fix: memory leak when transpiling (#24490)
- fix: node-api get_value_string_utf8 should use utf8_length (#24193)
- fix: panic when piping "deno help" or "deno --version" (#22917)
- fix: test in presence of `.npmrc` (#24486)
- fix: upgrade deno_core to 0.291.0 (#24297)
- perf(ext/node): improve `Buffer.from(buffer)` (#24352)
- perf(ext/websocket): avoid global interceptor overhead (#24284)
- perf(ws): optimize fastwebsockets in release profile (#24277)
- perf: optimize Buffer.from("base64") for forgiving-base64 conforming input
  (#24346)

### 1.44.4 / 2024.06.19

- Revert "chore: upgrade to reqwest 0.12.4 and rustls 0.22 (#24056)" (#24262)
- fix(ext/node): Add Dirent.path and Dirent.parentPath (#24257)
- fix(ext/node): Add SIGPOLL and SIGUNUSED signals (#24259)
- fix(ext/node): use primordials in `ext/node/polyfills/_utils.ts` (#24253)

### 1.44.3 / 2024.06.18

- feat(lsp): multi deno.json resolver scopes (#24206)
- fix(cli): missing flag for `--unstable-process` (#24199)
- fix(docs): correctly resolve href for built-ins (#24228)
- fix(ext/console): bump default max str lengthto 10_00 (#24245)
- fix(ext/http): actually await `goAhead` promise (#24226)
- fix(ext/node): add missing BlockList & SocketAddress classes (#24229)
- fix(ext/node): `server.close()` does graceful shutdown (#24184)
- fix(ext/node): better support for `node:diagnostics_channel` module (#24088)
- fix(ext/node): make process.versions own property (#24240)
- fix(ext/node): use `Deno.FsFile.statSync()` (#24234)
- fix(ext/permissions): add correct feature flags to winapi (#24218)
- fix(ext/web): fix `AbortSignal.timeout()` leak (#23842)
- fix(ext/webgpu): fix surface creation panic when adapter not initialized
  (#24201)
- fix(inspector): crash on "Debugger.setBlackboxPatterns" (#24204)
- fix(lsp): use import map from workspace root (#24246)
- fix(napi): Read reference ownership before calling finalizer to avoid crash
  (#24203)
- fix(no-slow-types): handle named type in mapped type (#24205)
- fix(npm): use more relaxed package.json version constraint parsing (#24202)
- fix(repl): prevent panic when deleting globalThis.closed property (#24014)
- perf(lsp): store settings in Arc (#24191)
- perf(node): ensure cjs wrapper module has deterministic output (#24248)

### 1.44.2 / 2024.06.13

- FUTURE: support `deno install <alias>@npm:<package>` (#24156)
- feat(lsp): respect editor indentation options (#24181)
- feat(lsp): workspace jsr resolution (#24121)
- fix(check): attempt to resolve types from pkg before `@types` pkg (#24152)
- fix(cli): Explicitly cache NPM packages during `deno install` (#24190)
- fix(cli): Overwrite existing bin entries in `node_modules` (#24123)
- fix(ext/http): print `[]` around ipv6 addresses (#24150)
- fix(ext/net): make node:http2 work with DENO_FUTURE=1 (#24144)
- fix(ext/node): ServerResponse header array handling (#24149)
- fix(ext/node): add crypto and zlib constants (#24151)
- fix(ext/node): fix vm memory usage and context initialization (#23976)
- fix(ext/node): lossy UTF-8 read node_modules files (#24140)
- fix(ext/node): send data frame with end_stream flag on _final call (#24147)
- fix(ext/node): support stdin child_process IPC & fd stdout/stderr (#24106)
- fix(ext/web): correct string tag for MessageEvent (#24134)
- fix(ext/websocket): correctly order messages when sending blobs (#24133)
- fix(jupyter): Avoid panicking when `DEBUG` env var is set (#24168)
- fix(lsp): don't sort workspace files (#24180)
- fix(lsp): strip .js before probing for valid import fix (#24188)
- fix(npm): resolve dynamic npm imports individually (#24170)
- fix: Rewrite Node-API (#24101)
- fix: clean up some node-api details (#24178)
- fix: do not panic linting files with UTF-8 BOM (#24136)
- fix: don't panic when cache is not available (#24175)
- fix: make writing to the deps cache more reliable (#24135)
- fix: upgrade deno_core (#24128)

### 1.44.1 / 2024.06.05

- fix(console): add missing AssertionError to js (#22358)
- fix(docs): update Deno.Command docs (#24097)
- fix(lsp): complete exports for import mapped jsr specifiers (#24054)
- fix(npm): use configured auth for tarball urls instead of scope auth (#24111)
- fix: better handling of npm resolution occurring on workers (#24094)
- fix: retry writing lockfile on failure (#24052)
- fix: support importing statically unanalyzable npm specifiers (#24107)
- fix: update deno_npm (#24065)
- fix: validate integer values in `Deno.exitCode` setter (#24068)

### 1.44.0 / 2024.05.30

- BREAKING(ffi/unstable): always return u64 as bigint (#23981)
- BREAKING(ffi/unstable): use BigInt representation in turbocall (#23983)
- FUTURE(ext/ffi,ext/webgpu): stabilize FFI and WebGPU APIs (#24011)
- FUTURE(ext/fs): stabilize file system APIs (#23968)
- FUTURE: initial support for .npmrc file (#23560)
- feat(cli): Add slow test warning (#23874)
- feat(cli/test): `deno test --clean` (#23519)
- feat(ext/fetch): `Request.bytes()` and `Response.bytes()` (#23823)
- feat(ext/fs): stabilize `Deno.FsFile.syncData[Sync]()` and
  `Deno.FsFile.sync[Sync]()` (#23733)
- feat(ext/fs): stabilize `Deno.FsFile.unlock[Sync]()` and
  `Deno.FsFile.lock[Sync]()` (#23754)
- feat(ext/webgpu): byow support for {Free,Open}BSD (#23832)
- feat(lint): add `no-boolean-literal-for-arguments` rule and enable
  `no-unused-vars` for jsx files (#24034)
- feat(lsp): support .npmrc (#24042)
- feat(node): buffer isUtf8/isAscii (#23928)
- feat(serve): support `--port 0` to use an open port (#23846)
- feat(task): run `npm run` commands with Deno more often (#23794)
- feat(vendor): support modifying remote files in vendor folder without checksum
  errors (#23979)
- feat: Add `Deno.exitCode` API (#23609)
- feat: add lowercase `-v` version flag (#23750)
- feat: do not require `DENO_FUTURE=1` for npmrc support (#24043)
- feat: enable pointer compression via deno_core bump (#23838)
- fix(cli): Prefer npm bin entries provided by packages closer to the root
  (#24024)
- fix(cli): Support deno.lock with only package.json present + fix DENO_FUTURE
  install interactions with lockfile (#23918)
- fix(cli/test): decoding percent-encoding(non-ASCII) file path correctly
  (#23200)
- fix(coverage): add tooltip to line count in html report (#23971)
- fix(coverage): do not generate script coverage with empty url (#24007)
- fix(coverage): handle ignore patterns (#23974)
- fix(coverage): skip generating coverage json for http(s) scripts (#24008)
- fix(deno_task): more descriptive error message (#24001)
- fix(ext/fs): truncate files when a ReadableStream is passed to writeFile
  (#23330)
- fix(ext/http): flush gzip streaming response (#23991)
- fix(ext/node): add `throwIfNoEntry` option in `fs.lstatSync` (#24006)
- fix(ext/node): add stubs for perf_hooks.PerformaceObserver (#23958)
- fix(ext/node): don't encode buffer data as utf8 in http2 (#24016)
- fix(ext/node): return cancelled flag in get_response_body_chunk op (#23962)
- fix(ext/node): windows cancel stdin read in line mode (#23969)
- fix(ext/node/fs): `position` argument not applied (#24009)
- fix(ext/web): `ReadableStream.from()` allows `Iterable` instead of
  `IterableIterator` (#23903)
- fix(ext/web): `ReadableStream.from()` ignores null `Symbol.asyncIterator`
  (#23910)
- fix(ext/websocket): change default idleTimeout to 30s (#23985)
- fix(lsp): don't discover deno.json in vendor dir (#24032)
- fix(lsp): process Fenced Code Block in JSDoc on `completion` correctly
  (#23822)
- fix(node): set default http server response code 200 (#23977)
- fix(npm): set up node_modules/.bin/ entries for package that provide bin
  entrypoints (#23496)
- fix(publish): raise diagnostics for triple-slash directives for `--dry-run`
  instead of just `publish` (#23811)
- fix(runtime): use more null proto objects (#23921)
- fix(task): always use `npm` for `npm run` with flags (#24028)
- fix: `--env` flag confusing message on syntax error (#23915)
- fix: bump cache sqlite dbs to v2 for WAL journal mode change (#24030)
- fix: empty `process.platform` with `__runtime_js_sources` (#24005)
- fix: use hash of in-memory bytes only for code cache (#23966)
- perf(cli): Improve concurrency when setting up `node_modules` and loading
  cached npm package info (#24018)
- perf(cli): Optimize setting up `node_modules` on macOS (#23980)
- perf(lsp): lock out requests until init is complete (#23998)
- perf(repl): don't walk workspace in repl language server (#24037)
- perf(startup): use WAL journal for sqlite databases in DENO_DIR (#23955)
- perf: avoid building module graph if dynamic specifier already in graph
  (#24035)
- perf: parse source files in parallel (#23858)
- perf: skip npm install if graph has no new packages (#24017)

### 1.43.6 / 2024.05.21

- fix(cli): use CliNodeResolver::resolve() for managed node_modules (#23902)
- fix(cli/coverage): invalid line id in html reporter (#23908)
- fix(ext/web): fix potential leak of unread buffers (#23923)
- fix(ext/webgpu): Allow `depthClearValue` to be undefined when `depthLoadOp` is
  not "clear" (#23850)
- fix(lsp): Fix display of JSDoc named examples (#23927)
- fix(lsp): apply import fix to missing declaration code action (#23924)
- fix(node): instantiating process class without new (#23865)
- fix(node): patch MessagePort in worker_thread message (#23871)
- fix(node): stub findSourceMap for `ava` (#23899)
- fix(node): track `SIG*` listeners in `process.listeners` (#23890)
- fix(task): do not error if node_modules folder not exists (#23920)
- fix: add missing `URL.parse` types (#23893)
- fix: handle signal 0 in process.kill (#23473)
- fix: serve handler error with 0 arguments (#23652)
- perf(cache): compile ts to js in parallel for `deno cache` (#23892)
- perf: analyze cjs exports and emit typescript in parallel (#23856)
- perf: analyze cjs re-exports in parallel (#23894)
- perf: resolver - skip cwd lookup if able (#23851)

### 1.43.5 / 2024.05.18

- fix(npm): regression deserializing JSON for some npm packages (#23868)

### 1.43.4 / 2024.05.16

- fix(cli): panic with `deno coverage` (#23353)
- fix(doc): --lint - private ref diagnostic was displaying incorrect information
  (#23834)
- fix(doc/publish): support expando properties (#23795)
- fix(emit): regression - keep comments in emit (#23815)
- fix(ext/node): export geteuid from node:process (#23840)
- fix(ext/node): fix grpc error_handling example (#23755)
- fix(ext/node): homedir() `getpwuid`/`SHGetKnownFolderPath` fallback (#23841)
- fix(ext/node): process.uptime works without this (#23786)
- fix(ext/web): update ongoing promise in async iterator `return()` method
  (#23642)
- fix(lsp): respect types dependencies for tsc roots (#23825)
- fix(lsp): show reference code lens on methods (#23804)
- fix(node): error when throwing `FS_EISDIR` (#23829)
- fix(node): seperate worker module cache (#23634)
- fix(node): stub `AsyncResource.emitDestroy()` (#23802)
- fix(node): wrong `worker_threads.terminate()` return value (#23803)
- fix(npm): handle null fields in npm registry JSON (#23785)
- fix(npm): make tarball extraction more reliable (#23759)
- fix(publish): always include config file when publishing (#23797)
- fix(publish): error for missing version constraints on dry-publish instead of
  just publish (#23798)
- fix(runtime): output to stderr with colors if a tty and stdout is piped
  (#23813)
- fix: Add missing `"junction"` type for `SymlinkOptions.types` (#23756)
- fix: update swc_ecma_parser to 0.114.1 (#23816)
- fix: widen aarch64 linux minimum GLIBC version by improving sysroot build
  (#23791)
- perf(compile): Do not checksum eszip content (#23839)
- perf(jsr): download metadata files as soon as possible and in parallel
  (#23836)
- perf(lsp): Cache semantic tokens for open documents (#23799)

### 1.43.3 / 2024.05.10

- fix(ext/webgpu): invalidate GPUAdapter when a device is created (#23752)
- fix(lsp): completions for using decl identifiers (#23748)
- fix(lsp): move sloppy import resolution from loader to resolver (#23751)
- fix(node): better cjs re-export handling (#23760)
- fix(runtime): Allow opening /dev/fd/XXX for unix (#23743)
- fix(task): regression where `npx <command>` sometimes couldn't find command
  (#23730)
- fix: bump deno_core to fix unsoundness (#23768)

### 1.43.2 / 2024.05.08

- feat(runtime): allow adding custom extensions to snapshot (#23569)
- fix(compile): relative permissions should be retained as relative (#23719)
- fix(ext/node): check resource exists before close (#23655)
- fix(ext/node): don't rely on Deno.env to read NODE_DEBUG (#23694)
- fix(ext/node): napi_get_element and napi_set_element work with objects
  (#23713)
- fix(ext/node): support delete process.env.var (#23647)
- fix(ext/web): properly handle `Blob` case for `createImageBitmap` (#23518)
- fix(ext/webgpu): correctly validate GPUExtent3D, GPUOrigin3D, GPUOrigin2D &
  GPUColor (#23413)
- fix(fmt/js): `else` was moved to wrong `if` sometimes when formatting minified
  code (#23706)
- fix(jsr): panic when importing jsr package with deps via https (#23728)
- fix(lsp): Catch cancellation exceptions thrown by TSC, stop waiting for TS
  result upon cancellation (#23645)
- fix(lsp): Pass diagnostic codes to TSC as numbers (#23720)
- fix(lsp): always cache all npm packages (#23679)
- fix(lsp): handle multiline semantic tokens (#23691)
- fix(publish): public api - trace parent classes & interfaces when following a
  method (#23661)
- fix(runtime): allow r/w access to /etc without --allow-all (#23718)
- fix(test): proper type checking for files with doc tests (#23654)
- fix(workers): `importScripts` concurrently and use a new `reqwest::Client` per
  importScripts (#23699)
- fix: DOMException doesn't throw on __callSitesEvals (#23729)
- fix: upgrade TypeScript from 5.4.3 to 5.4.5 (#23740)

### 1.43.0 / 2024.05.01

- FUTURE(ext/net): remove
  `Deno.ConnectTlsOptions.(certFile|certChain|privateKey)` (#23270)
- FUTURE(ext/net): remove `Deno.ListenTlsOptions.(keyFile|certFile)` (#23271)
- FUTURE: remove `Deno.customInspect` (#23453)
- FUTURE: remove import assertions support for JavaScript (#23541)
- feat(check): allow using side effect imports with unknown module kinds (ex.
  css modules) (#23392)
- feat(ci): category & unstable tags checker (#23568)
- feat(cli): add support for jsxImportSourceTypes (#23419)
- feat(ext/http): Add `addr` to HttpServer (#23442)
- feat(ext/http): Implement request.signal for Deno.serve (#23425)
- feat(ext/net): extract TLS key and certificate from interfaces (#23327)
- feat(ext/url): add `URL.parse` (#23318)
- feat(ext/webgpu): support `UnsafeWindowSurface` on wayland (#23423)
- feat(jsr): support importing from jsr via HTTPS specifiers (except for type
  checking) (#23513)
- feat(runtime): Allow embedders to perform additional access checks on file
  open (#23208)
- feat(task): support running npm binary commands in deno.json (#23478)
- feat: Add `deno serve` subcommand (#23511)
- feat: add jsx precompile skip element option (#23457)
- feat: enable Float16Array support (#23490)
- feat: upgrade V8 to 12.4 (#23435)
- fix(ci): Fix bench job (#23410)
- fix(cli): Don't panic on invalid emit options (#23463)
- fix(cli): Identify and fix a test deadlock (#23411)
- fix(cli): TestEventSender should be !Clone (#23405)
- fix(cli): avoid `deno add` and `deno vendor` errors when deno.json is empty
  (#23439)
- fix(compile): certain jsr specifiers sometimes can't load (#23567)
- fix(config): move json schema unstable examples to item (#23506)
- fix(ext/http): ensure signal is created iff requested (#23601)
- fix(ext/net): check for TLS using undefined rather than using ReflectHas
  (#23538)
- fix(ext/node): Correctly send ALPN on node TLS connections (#23434)
- fix(ext/node): Support `env` option in worker_thread (#23462)
- fix(ext/node): `cp` into non-existent parent directory (#23469)
- fix(ext/node): add support for MessagePort.removeListener/off (#23598)
- fix(ext/node): define http.maxHeaderSize (#23479)
- fix(ext/node): dispatch beforeExit/exit events irrespective of listeners
  (#23382)
- fix(ext/node): exporting rsa public keys (#23596)
- fix(ext/node): implement process.kill in Rust (#23130)
- fix(ext/node): read(0) before destroying http2stream (#23505)
- fix(ext/node): remove unwraps from fallible conversions (#23447)
- fix(ext/node): support NODE_DEBUG env (#23583)
- fix(ext/node): support multiple message listeners on MessagePort (#23600)
- fix(ext/node): support process.stdin.unref() (#22865)
- fix(ext/node): worker_threads copies env object (#23536)
- fix(ext/node): worker_threads.receiveMessageOnPort doesn't panic (#23406)
- fix(fmt): error for more unterminated nodes (#23449)
- fix(fmt/md): better handling of lists in block quotes (#23604)
- fix(lsp): Fix logic for coalescing pending changes + clear script names cache
  when file is closed (#23517)
- fix(lsp): inherit missing fmt and lint config from parent scopes (#23547)
- fix(lsp): remove Document::open_data on close (#23483)
- fix(node): require.resolve - fallback to global cache when bare specifier from
  paths not found (#23618)
- fix(npm): do not panic hitting root dir while resolving npm package (#23556)
- fix(publish): --dry-publish should error for gitignored excluded files
  (#23540)
- fix(publish): handle variable declarations with a declare keyword (#23616)
- fix(publish): support import equals (#23421)
- fix(workspace): provide workspace members as 'imports' in import map (#23492)
- fix: Fix some typos in comments (#23470)
- fix: Float16Array support (#23512)
- fix: add `DENO_FUTURE` to `deno --help` (#23368)
- fix: allow WPT to successfully exit using `--exit-zero` (#23418)
- fix: handle specifying an import map in an ancestor dir of deno.json (#23602)
- fix: reenable syntax highlighting for doc html generator (#23570)
- fix: unref stdin read (#23534)
- fix: update CLI flags for WPT (#23501)
- perf(ext/http): cache abort signal error (#23548)
- perf(ext/http): recover memory for serve and optimize AbortController (#23559)
- perf(lsp): Avoid passing struct into op_resolve (#23452)
- perf(lsp): Batch "$projectChanged" notification in with the next JS request
  (#23451)
- perf(lsp): Call `serverRequest` via V8 instead of via `executeScript` (#23409)
- perf(lsp): Pass code action trigger kind to TSC (#23466)
- perf(lsp): cleanup document dependencies (#23426)
- perf(lsp): only store parsed sources for open documents (#23454)
- perf(lsp): release unused documents (#23398)
- perf: v8 code cache (#23081)

### 1.42.4 / 2024.04.15

- fix(check): cache bust when changing nodeModulesDir setting (#23355)
- fix(ext/io): Fix NUL termination error in windows named pipes (#23379)
- fix(ext/node): add stub for AsyncResource#asyncId() (#23372)
- fix(ext/node): panic on 'worker_threads.receiveMessageOnPort' (#23386)
- fix(ext/node): promise rejection in VM contexts (#23305)
- fix(ext/node): use ext/io stdio in WriteStream (#23354)
- fix(lsp): ensure project version is incremented when config changes (#23366)
- fix(lsp): improved cjs tracking (#23374)
- fix(lsp): slice strings by byte index in code actions (#23387)
- fix(publish): do not error for param with initializer before required
  parameter (#23356)
- fix(publish): handle definite assignment on ts private properties (#23345)
- perf(lsp): Only deserialize response from `op_respond` once (#23349)
- perf: do not clone swc `Program` when transpiling (#23365)

### 1.42.3 / 2024.04.12

- Revert "refactor(ext/net): extract TLS key and certificate from inter…
  (#23325)
- fix(inspector): don't panic if port is not free (#22745)
- fix(lsp): Denormalize specifiers before calling `$projectChanged` (#23322)
- fix(npm): local nodeModulesDir was sometimes resolving duplicates of same
  package (#23320)
- fix(publish): do not warn about excluded external modules in node_modules
  directory (#23173)
- fix: upgrade deno_ast related crates (#23187)
- perf(lsp): use a stub module in tsc for failed resolutions (#23313)

### 1.42.2 / 2024.04.11

- FUTURE(ext/fs): make `Deno.FsFile` constructor illegal (#23235)
- FUTURE(ext/fs): remove `Deno.FsWatcher.prototype.rid` (#23234)
- FUTURE(ext/net): remove
  `Deno.(Conn|TlsConn|Listener|TlsListener|UnixConn).prototype.rid` (#23219)
- FUTURE: enable BYONM by default (#23194)
- FUTURE: override byonm with nodeModulesDir setting (#23222)
- FUTURE: remove deprecated APIs within workers (#23220)
- feat(lsp): respect nested deno.json for fmt and lint config (#23159)
- fix(cli): Enforce a human delay in prompt to fix paste problem (#23184)
- fix(cli): fix deadlock in test writer when test pipe is full (#23210)
- fix(cli): update `deno doc` help to fit current usage (#23224)
- fix(ext/fs): account for all ops in leak checks (#23300)
- fix(ext/http): Make `Deno.serveHttp()` work when proxying (#23269)
- fix(ext/net): Improve ts types for network APIs (#23228)
- fix(ext/node): Add "module" to builtinsModule (#23242)
- fix(ext/node): Add fs.readv, fs.readvSync (#23166)
- fix(ext/node): MessagePort works (#22999)
- fix(ext/node): Support returning tokens and option defaults in
  `node:util.parseArgs` (#23192)
- fix(ext/node): `node:vm` contexts (#23202)
- fix(ext/node): count MessagePort message listeners in hasMessageEventListener
  (#23209)
- fix(ext/node): hostname is valid IPv4 addr (#23243)
- fix(ext/node): implement MessagePort.unref() (#23278)
- fix(ext/node): improve AsyncLocalStorage api (#23175)
- fix(ext/node): out-of-order writes of fs.createWriteStream (#23244)
- fix(ext/node): patch MessagePort if provided as workerData (#23198)
- fix(ext/node): polyfill node:domain module (#23088)
- fix(ext/tls): add support EC private key (#23261)
- fix(lsp): Remove client-facing format failure warning (#23196)
- fix(lsp): respect DENO_FUTURE for BYONM config (#23207)
- fix(runtime): fix Windows permission prompt (#23212)
- fix: prevent cache db errors when deno_dir not exists (#23168)
- perf(lsp): Don't retain `SourceFileObject`s in `sourceFileCache` longer than
  necessary (#23258)
- perf(lsp): More granular locking of `FileSystemDocuments` (#23291)
- perf(lsp): Only evict caches on JS side when things actually change (#23293)
- perf(lsp): cache ts config in isolate until new project version (#23283)
- perf(lsp): don't keep remote module ast's in memory (#23230)
- perf(lsp): don't pass remote modules as tsc roots (#23259)
- perf(lsp): replace document registry source cache on update (#23311)
- perf(lsp): use lockfile to reduce npm pkg resolution time (#23247)
- perf(node): put pkg json into an `Rc` (#23156)
- perf: reduce allocations in `MediaType::from_specifier` (#23190)

### 1.42.1 / 2024.04.01

- fix(check): ignore certain diagnostics in remote modules and when publishing
  (#23119)
- fix(ext/node): support stdin: "inherit" in node:child_process (#23110)
- fix(ext/node): use tty stdin from ext/io (#23044)
- fix(jsr): exclude yanked versions from 'deno add' and completions (#23113)
- fix(lsp): don't apply preload limit to workspace walk (#23123)
- fix(lsp): implement missing ts server host apis (#23131)
- fix(node): handle empty 'main' entry in pkg json (#23155)
- fix(node): remove unwrap in op_require_node_module_paths (#23114)
- fix: deno_graph 0.69.10 (#23147)

### 1.42.0 / 2024.03.28

- feat(add): always produce multiline config file (#23077)
- feat(ext/node): add riscv64 in process.arch (#23016)
- feat(init): use jsr specifier for @std/assert (#23073)
- feat(install): require -g / --global flag (#23060)
- feat(lint): `deno lint --fix` and lsp quick fixes (#22615)
- feat(lint): automatically opt-in packages to `jsr` lint tag (#23072)
- feat(node): load ES modules defined as CJS (#22945)
- feat(publish): check for uncommitted files in `deno publish --dry-run`
  (#22981)
- feat(task): Task description in the form of comments (#23101)
- feat(task): cross-platform shebang support (#23091)
- feat(unstable/publish): error when a package's module is excluded from
  publishing (#22948)
- feat: TypeScript 5.4 (#23086)
- feat: add `--watch-exclude` flag (#21935)
- feat: deno_task_shell 0.15 (#23019)
- feat: remove deprecated methods from namespace with `DENO_FUTURE=1` (#23075)
- feat: type declarations for new Set methods (#23090)
- fix(bench): Fix group header printing logic + don't filter out the warmup
  benchmark (#23083)
- fix(check): do not suggest running with `--unstable` (#23092)
- fix(cli): output more detailed information for steps when using JUnit reporter
  (#22797)
- fix(cli): sanitizer should ignore count of ops started before tests begin
  (#22932)
- fix(coverage): Error if no files are included in the report (#22952)
- fix(ext/fetch): do not truncate field value in `EventSource` (#22368)
- fix(ext/fetch): make `EventSource` more robust (#22493)
- fix(ext/node): ECDH.publicKey() point encoding (#23013)
- fix(ext/node): FsWatcher ref and unref (#22987)
- fix(ext/node): Reimplement StringDecoder to match node's behavior (#22933)
- fix(ext/node): add crypto.getRandomValues (#23028)
- fix(ext/node): add crypto.subtle (#23027)
- fix(ext/node): add process.setSourceMapsEnabled noop (#22993)
- fix(ext/node): handle KeyObject in `prepareAsymmetricKey` (#23026)
- fix(ext/node): handle `null` in stdio array (#23048)
- fix(ext/node): implement EventEmitterAsyncResource (#22994)
- fix(ext/node): implement v8 serialize and deserialize (#22975)
- fix(ext/node): panic in `op_node_ecdh_generate_keys` (#23011)
- fix(ext/node): pass normalized watchFile handler to StatWatcher (#22940)
- fix(ext/node): spread args in setImmediate (#22998)
- fix(ext/node): support Diffie-Hellman key type in `crypto.createPrivateKey()`
  (#22984)
- fix(ext/node): support MessagePort in `WorkerOptions.workerData` (#22950)
- fix(ext/node): support public key point encoding in ECDH.generateKeys()
  (#22976)
- fix(ext/node): worker_threads ESM handling (#22841)
- fix(ext/node): worker_threads doesn't exit if there are message listeners
  (#22944)
- fix(ext/web): Fix structuredClone Web API type declaration (any -> generic)
  (#22968)
- fix(jupyter): Do not increase counter if store_history=false (#20848)
- fix(lsp): decoding percent-encoding(non-ASCII) file path correctly (#22582)
- fix(lsp): prefer cache over tsc quick fixes (#23093)
- fix(lsp): use registry cache for completion search (#23094)
- fix(runtime): use FQDN in NetDescriptor (#23084)
- fix: do not memoize `Deno.ppid` (#23006)
- fix: don't panic in test and bench if ops not available (#23055)
- fix: handle cache body file not existing when using etag (#22931)
- fix: less aggressive vendor folder ignoring (#23100)
- perf: warm expensive init code at snapshot time (#22714)

### 1.41.3 / 2024.03.14

- fix(cli): occasional panics on progress bar (#22809)
- fix(cli): show asserts before leaks (#22904)
- fix(cli): unbreak extension example and fix __runtime_js_sources (#22906)
- fix(cli): use Instant for test times (#22853)
- fix(config): add unstable features as examples to config schema (#22814)
- fix(config): remove pkg name example and add pattern to schema (#22813)
- fix(ext/node): add more named curves in `crypto.generateKeyPair[Sync]()`
  (#22882)
- fix(ext/node) implement receiveMessageOnPort for node:worker_threads (#22766)
- fix(ext/node): DH (`dhKeyAgreement`) support for `createPrivateKey` (#22891)
- fix(ext/node): Add Immediate class to mirror NodeJS.Immediate (#22808)
- fix(ext/node): Implement `isBuiltin` in `node:module` (#22817)
- fix(ext/node): Match punycode module behavior to node (#22847)
- fix(ext/node): Support private EC key signing (#22914)
- fix(ext/node): allow automatic worker_thread termination (#22647)
- fix(ext/node): crypto.getCipherInfo() (#22916)
- fix(ext/node): flush brotli decompression stream (#22856)
- fix(ext/node): initial `crypto.createPublicKey()` support (#22509)
- fix(ext/node): make worker ids sequential (#22884)
- fix(ext/node): make worker setup synchronous (#22815)
- fix(ext/node): support `spki` format in createPublicKey (#22918)
- fix(ext/node): support junction symlinks on Windows (#22762)
- fix(ext/node): worker_threads.parentPort is updated on startup (#20794)
- fix(ext/websocket): do not continue reading if socket rid closes (#21849)
- fix(node): add nul byte to statfs path on windows (#22905)
- fix(node): implement fs.statfs() (#22862)
- fix(node): require of pkg json imports was broken (#22821)
- fix(node): resolve .css files in npm packages when type checking (#22804)
- fix(node): resolve types via package.json for directory import (#22878)
- fix(node:http) Export `validateHeaderName` and `validateHeaderValue` functions
  (#22616)
- fix(publish): ability to un-exclude when .gitignore ignores everything
  (#22805)
- fix(publish): regression - publishing with vendor folder (#22830)
- fix(publish): suggest using `--allow-dirty` on uncommitted changes (#22810)
- fix(publish): typo in `--allow-dirty` help text (#22799)
- fix(runtime): Restore default signal handler after user handlers are
  unregistered (#22757)
- fix(runtime): negate partial condition for deny flags (#22866)
- fix(slow-types): improved exports tracing and infer type literals in as exprs
  (#22849)
- fix: fix crate vulnerabilities (#22825)
- fix: stop type checking during runtime (#22854)
- fix: support sloppy resolution to file where directory exists (#22800)
- fix: typo in error from GPUBuffer.prototype.mapAsync (#22913)
- perf(permissions): Fast exit from checks when permission is in "fully-granted"
  state (#22894)

### 1.41.2 / 2024.03.08

- fix(ext/node): ref/unref on workers (#22778)
- feat(lsp): include registry url in jsr import hover text (#22676)
- feat(node/util): styleText (#22758)
- feat(publish): add `npm:` suggestion for esm.sh specifiers (#22343)
- feat(unstable/pm): support npm packages in 'deno add' (#22715)
- feat(unstable/pm): support version contraints in 'deno add' (#22646)
- fix(cli): force flush output after test unloads (#22660)
- fix(cli): improve logging on failed named pipe (#22726)
- fix(cli): limit test parallelism on Windows to avoid pipe error (#22776)
- fix(cli): remove possible deadlock in test channel (#22662)
- fix(ext/node): add default methods to fs.StatsBase (#22750)
- fix(ext/node): http2.createServer (#22708)
- fix(ext/node): strip `--enable-source-maps` from argv (#22743)
- fix(lsp): do not warn about local file "redirects" from .js to .d.ts files
  (#22670)
- fix(lsp): don't apply renames to remote modules (#22765)
- fix(lsp): ignore code errors when type passes for non-`@deno-types` reolution
  (#22682)
- fix(lsp): output more information on error (#22665)
- fix(lsp): regression - caching in lsp broken when config with imports has
  comments (#22666)
- fix(node): errno property when command missing (#22691)
- fix(node): implement ALS enterWith (#22740)
- fix(node): improve cjs tracking (#22673)
- fix(node): stat/statSync returns instance of fs.Stats (#22294)
- fix(publish): do not include .gitignore (#22789)
- fix(publish): include explicitly specified .gitignored files and directories
  (#22790)
- fix(publish): make include and exclude work (#22720)
- fix(publish): permissionless dry-run in GHA (#22679)
- fix(publish): properly display graph validation errors (#22775)
- fix(publish): reland error if there are uncommitted changes (#22613) (#22632)
- fix(publish): silence warnings for sloppy imports and node builtins with env
  var (#22760)
- fix(tools/publish): correctly handle importing from self in unfurling (#22774)
- fix(unstable/publish): repect `--no-check` in no-slow-types (#22653)
- fix: Provide source map for internal extension code (#22716)
- fix: don't include source maps in release mode (#22751)
- fix: point to correct WPT runner file (#22753)
- fix: respect unstable "temporal" configuration in config file (#22134)
- fix: update node process version to latest node LTS (#22709)
- perf(cli): faster standalone executable determination (#22717)
- perf(cli): use faster_hex (#22761)
- perf(cli): use new deno_core timers (#22569)
- perf(cli): hard link npm cache (#22773)

### 1.41.1 / 2024.02.29

- feat(unstable): `deno add` subcommand (#22520)
- feat(unstable/lsp): jsr specifier completions (#22612)
- feat(unstable/publish): discover jsr.json and jsr.jsonc files (#22587)
- feat(unstable/publish): enable package provenance by default on github actions
  (#22635)
- feat(unstable/publish): infer dependencies from package.json (#22563)
- feat(unstable/publish): provenance attestation (#22573)
- feat(unstable/publish): respect .gitignore during `deno publish` (#22514)
- feat(unstable/publish): support sloppy imports and bare node built-ins
  (#22588)
- fix(compile): add aarch64 linux to `CliOptions::npm_system_info` (#22567)
- fix(compile): allow to compile for ARM linux (#22542)
- fix(ext/crypto): align the return type of `crypto.randomUUID` to
  `cli/tsc/dts/lib.dom.d.ts` (#22465)
- fix(ext/node) add node http methods (#22630)
- fix(ext/node): init arch, pid, platform at startup (#22561)
- fix(ext/node): set correct process.argv0 (#22555)
- fix(io): create_named_pipe parallelism (#22597)
- fix(jsr): do not allow importing a non-JSR url via unanalyzable dynamic import
  from JSR (#22623)
- fix(jsr): relative dynamic imports in jsr packages (#22624)
- fix(lsp): import map expansion (#22553)
- fix(publish): disable provenance if not in GHA (#22638)
- fix(publish): make the already published message look like a warning (#22620)
- fix(publish): print a warning when .jsx or .tsx is imported (#22631)
- fix(publish): reduce warnings about dynamic imports (#22636)
- fix(test): ensure that pre- and post-test output is flushed at the appropriate
  times (#22611)
- fix(unstable): add `Date#toTemporalInstant` type (#22637)
- fix(unstable): sloppy imports should resolve .d.ts files during types
  resolution (#22602)
- perf(cli): reduce overhead in test registration (#22552)
- perf(fmt): reduce memory usage and improve performance (#22570)

### 1.41.0 / 2024.02.22

- BREAKING(net/unstable): remove `Deno.DatagramConn.rid` (#22475)
- BREAKING(unstable): remove `Deno.HttpClient.rid` (#22075)
- BREAKING: add `Deno.CreateHttpClientOptions.{cert,key}` (#22280)
- feat(fs): `Deno.FsFile.{isTerminal,setRaw}()` (#22234)
- feat(lsp): auto-import completions for jsr specifiers (#22462)
- feat(publish): type check on publish (#22506)
- feat(unstable): single checksum per JSR package in the lockfile (#22421)
- feat(unstable/lint): no-slow-types for JSR packages (#22430)
- feat: `Deno.ConnectTlsOptions.{cert,key}` (#22274)
- fix(compile): respect compiler options for emit (#22521)
- fix(ext/fs): make errors in tempfile creation clearer (#22498)
- fix(ext/node): pass alpnProtocols to Deno.startTls (#22512)
- fix(ext/node): permission prompt for missing `process.env` permissions
  (#22487)
- fix(fmt): remove debug output when formatting dynamic imports (#22433)
- fix(lsp): add schema for JSR related config options (#22497)
- fix(node/test): disable Deno test sanitizers (#22480)
- fix(publish): better no-slow-types type discovery (#22517)
- fix(publish): ignore .DS_Store while publishing (#22478)
- fix(publish): print files that will be published (#22495)
- fix: util.parseArgs() missing node:process import (#22405)
- fix: write lockfile in `deno info` (#22272)
- perf(jsr): fast check cache and lazy fast check graph (#22485)
- perf: linter lsp memory leak fix and deno_graph executor (#22519)
- perf: strip `denort` on unix (#22426)

### 1.40.5 / 2024.02.15

- feat(lsp): jsr support first pass (#22382)
- feat(lsp): jsr support with cache probing (#22418)
- feat(publish): allow passing config flag (#22416)
- feat(unstable): define config in publish url (#22406)
- perf: denort binary for `deno compile` (#22205)
- fix(console): support NO_COLOR and colors option in all scenarios (#21910)
- fix(ext/node): export process.umask (#22348)
- fix(ext/web): Prevent (De-)CompressionStream resource leak on stream
  cancellation (#21199)
- fix(lsp): complete npm specifier versions correctly (#22332)
- fix: cache bust jsr deps on constraint failure (#22372)
- fix: handle non-file scopes in synthetic import map (#22361)
- fix: lockfile was sometimes getting corrupt when changing config deps (#22359)
- fix: upgrade to deno_ast 0.33 (#22341)

### 1.40.4 / 2024.02.08

- feat(unstable): `Deno.FsFile.lock[Sync]()` and `Deno.FsFile.unlock[Sync]()`
  (#22235)
- feat: ARM64 builds (#22298)
- fix(cli): Add IP address support to DENO_AUTH_TOKEN (#22297)
- fix(ext/node): Ensure os.cpus() works on arm64 linux (#22302)
- fix(ext/node): fix timeout param validation in cp.execFile (#22262)
- fix(jupyter): ensure op is available (#22240)
- fix(lint): point to migration docs in deprecated APIs rule (#22338)
- fix(lsp): disable no-cache diagnostics for jsr specifiers (#22284)
- fix(node): add `cp` to fs/promises (#22263)
- fix(node): handle brotli compression end chunk sizes (#22322)
- fix(os): total and free memory in bytes (#22247)
- fix(publish): 'explit' typo (#22296)
- fix(publish): handle diagnostic outside graph (#22310)
- fix(publish): lazily parse sources (#22301)
- fix(publish): use lighter crate for opening browser (#22224)
- fix(test/regression): handle CLI arg directory using `../` in path (#22244)
- fix(unstable): validate kv list selector (#22265)
- fix: Fix segmentation fault in tests on CPUs with PKU support (#22152)
- fix: Support Symbol.metadata (#22282)
- fix: enable "--allow-sys=cpus" for "deno run" (#22260)
- perf: remove duplicate `env::current_dir` call in package.json search (#22255)

### 1.40.3 / 2024.02.01

- Revert "refactor(cli): use new sanitizer for resources (#22125)" (#22153)
- feat(unstable): implement `navigator.gpu.getPreferredCanvasFormat()` (#22149)
- fix(ext/node): add `aes256` algorithm support (#22198)
- fix(ext/node): limit OpState borrow in op_napi_open (#22151)
- fix(fs): copyFile NUL path on macOS (#22216)
- fix(install): forward granular --unstable-* flags (#22164)
- fix(lockfile): only consider package.json beside lockfile in workspace
  property (#22179)
- fix(lsp): don't normalize urls in cache command params (#22182)
- fix(node): `util.callbackify` (#22200)
- fix(node): add `ppid` getter for `node:process` (#22167)
- fix(publish): add node specifiers (#22213)
- fix(publish): rename --no-fast-check to --no-zap (#22214)
- fix(runtime): return number from `op_ppid` instead of bigint (#22169)
- fix: canary for arm64 macos (#22187)

### 1.40.2 / 2024.01.26

- feat(lsp): complete parameters as tab stops and placeholders (#22126)
- fix(ext/http): smarter handling of Accept-Encoding (#22130)
- fix(fs): instanceof check for Deno.FsFile (#22121)
- fix(node): remove deprecation warnings (#22120)
- fix(testing): add op_spawn_wait mapping in resource sanitizer (#22129)
- fix: make deprecation warnings less verbose (#22128)

### 1.40.1 / 2024.01.25

- fix(lsp): disable experimentalDecorators by default (#22101)

### 1.40.0 / 2024.01.25

- feat(unstable): remove `Deno.cron()` overload (#22035)
- feat: improved diagnostics printing (#22049)
- feat(jupyter): don't require --unstable flag (#21963)
- feat(lockfile): track JSR and npm dependencies in config file (#22004)
- feat(lsp): include scope uri in "deno/didChangeDenoConfiguration" (#22002)
- feat(lsp): send "deno/didChangeDenoConfiguration" on init (#21965)
- feat(publish): error on invalid external imports (#22088)
- feat(publish): exclude and include (#22055)
- feat(publish): give diagnostic on invalid package files (#22082)
- feat(unstable): add Temporal API support (#21738)
- feat(unstable): remove Deno.upgradeHttp API (#21856)
- feat(web): ImageBitmap (#21898)
- feat: "rejectionhandled" Web event and "rejectionHandled" Node event (#21875)
- feat: Expand 'imports' section of deno.json (#22087)
- feat: Stabilize Deno.connect for 'unix' transport (#21937)
- feat: Stabilize Deno.listen for 'unix' transport (#21938)
- feat: TC39 decorator proposal support (#22040)
- feat: `Deno.FsFile.dataSync()` and `Deno.FsFile.dataSyncSync()` (#22019)
- feat: `Deno.FsFile.{utime,utimeSync}()` and deprecate
  `Deno.{futime,futimeSync}` (#22070)
- feat: `Deno.{stdin,stdout,stderr}.isTerminal()`, deprecate `Deno.isatty()`
  (#22011)
- feat: `FsFile.sync()` and `FsFile.syncSync()` (#22017)
- feat: deprecate `Deno.serveHttp` API (#21874)
- feat: deprecate `Deno.FsFile` constructor and `Deno.FsFile.rid` (#22072)
- feat: deprecate `Deno.FsWatcher.rid` (#22074)
- feat: deprecate `Deno.Listener.rid` (#22076)
- feat: deprecate `Deno.close()` (#22066)
- feat: deprecate `Deno.fstat()` and `Deno.fstatSync()` (#22068)
- feat: deprecate `Deno.ftruncate()` and `Deno.ftruncateSync()` (#22069)
- feat: deprecate `Deno.read()` and `Deno.readSync()` (#22063)
- feat: deprecate `Deno.resources()` (#22059)
- feat: deprecate `Deno.seek()` and `Deno.seekSync()` (#22065)
- feat: deprecate `Deno.shutdown()` (#22067)
- feat: deprecate `Deno.write()` and `Deno.writeSync()` (#22064)
- feat: deprecate `Deno.{Conn,TcpConn,TlsConn,UnixConn}.rid` (#22077)
- feat: deprecate `Deno.{stdin,stdout,stderr}.rid` (#22073)
- feat: deprecate `window` global (#22057)
- feat: import.meta.filename and import.meta.dirname (#22061)
- feat: remove conditional unstable type-checking (#21825)
- feat: stabilize Deno.Conn.ref/unref (#21890)
- feat: stabilize Deno.connectTls options and Deno.TlsConn.handshake (#21889)
- feat: warn when using --unstable, prefer granular flags (#21452)
- feat: External webgpu surfaces / BYOW (#21835)
- fix(BREAKING): remove dead `--prompt` flag (#22038)
- fix(ext/cron): automatically override unspecified values (#22042)
- fix(ext/node): fix no arg call of fs.promises.readFile (#22030)
- fix(info): return proper exit code on error (#21952)
- fix(lsp): improved npm specifier to import map entry mapping (#22016)
- fix(lsp): regression - formatting was broken on windows (#21972)
- fix(node): remove use of non existing `FunctionPrototypeApply` primordial
  (#21986)
- fix(node): update `req.socket` on WS upgrade (#21984)
- fix(node): use `cppgc` for managing X509Certificate (#21999)
- fix(node/fs): promises not exporting fs constants (#21997)
- fix(node/http): remoteAddress and remotePort not being set (#21998)
- fix(types): align global deno worker type with deno.worker/webworker one
  (#21936)

### 1.39.4 / 2024.01.13

- fix(config): regression - handle relative patterns with leading dot slash
  (#21922)
- fix(check): should not panic when all specified files excluded (#21929)

### 1.39.3 / 2024.01.12

- feat(unstable): fast subset type checking of JSR dependencies (#21873)
- fix(ci): update copright year for _fs_cp.js (#21803)
- fix(cli): update import map url (#21824)
- fix(compile): preserve granular unstable features (#21827)
- fix(ext): enable prefer-primordials for internal TypeScript (#21813)
- fix(ext/crypto): initial support for p521 in `generateKey` and `importKey`
  (#21815)
- fix(ext/node): add WriteStream.isTTY (#21801)
- fix(ext/node): add fs.cp, fs.cpSync, promises.cp (#21745)
- fix(ext/websocket): pass on uncaught errors in idleTimeout (#21846)
- fix(fast_check): analyze identifiers in type assertions/as exprs (#21899)
- fix(kv): improve .listenQueue types (#21781)
- fix(lsp): implement host.getGlobalTypingsCacheLocation() (#21882)
- fix(lsp): show test code lens for template literal names (#21798)
- fix(lsp): use a dedicated thread for the parent process checker (#21869)
- fix(registry): wait for already pending publish (#21663)
- fix(task): do not eagerly auto-install packages in package.json when
  `"nodeModulesDir": false` (#21858)
- fix(unstable/tar): skip node_modules, .git, and config "exclude" (#21816)
- fix(web): use rustyline for prompt (#21893)
- fix: add EventSource typings (#21908)
- fix: android support (#19437)
- fix: cjs export rewritten to invalid identifier (#21853)
- fix: update deno_lint and swc (#21718)
- perf(lsp): use host-owned cache for auto-import completions (#21852)
- perf: skip expanding exclude globs (#21817)

### 1.39.2 / 2024.01.04

- Revert "fix(runtime): Make native modal keyboard interaction consistent with
  browsers" (#21739)
- feat(lsp): allow to connect V8 inspector (#21482)
- feat(lsp): cache jsxImportSource automatically (#21687)
- feat(unstable): only allow http2 for kv remote backend (#21616)
- fix(ci): copyright year for console_test.ts (#21787)
- fix(cli): harden permission stdio check (#21778)
- fix(cli): make signals tests more reliable (#21772)
- fix(cli): respect `exclude` option for `deno check` command (#21779)
- fix(ext/http): use arraybuffer binaryType for server websocket (#21741)
- fix(ext/node): Implement `aes-192-ecb` and `aes-256-ecb` (#21710)
- fix(ext/node): UdpSocket ref and unref (#21777)
- fix(ext/node): add ClientRequest#setNoDelay (#21694)
- fix(ext/node): add process.abort() (#21742)
- fix(ext/node): implement os.machine (#21751)
- fix(ext/node): querystring stringify without encode callback (#21740)
- fix(ext/node): use node:process in _streams.mjs (#21755)
- fix(http_client): Fix Deno.createHttpClient to accept poolIdleTimeout
  parameter (#21603)
- fix(jupyter): error message when install fails due to jupyter command not
  being on PATH (#21767)
- fix(lsp): support test code lens for Deno.test.{ignore,only}() (#21775)
- fix(node): Implement os.cpus() (#21697)
- fix(node): support nested tests in "node:test" (#21717)
- fix(node/zlib): accept dataview and buffer in zlib bindings (#21756)
- fix(node/zlib): cast Dataview and Buffer to uint8 (#21746)
- fix(node/zlib): consistently return buffer (#21747)
- fix(unstable): kv watch should stop when db is closed (#21665)
- fix(unstable/byonm): support using an import map with byonm (#21786)
- fix: `Object.groupBy` return type should be a partial (#21680)
- fix: allow npm: specifiers in import.meta.resolve (#21716)
- fix: strict type check for cross realms (#21669)
- perf(coverage): faster source mapping (#21783)
- perf(lsp): use LanguageServiceHost::getProjectVersion() (#21719)
- perf: remove opAsync (#21690)

### 1.39.1 / 2023.12.21

- fix(bench): added group banner to bench output. (#21551)
- fix(console): inspect for `{Set,Map}Iterator` and `Weak{Set,Map}` (#21554)
- fix(coverage): add default coverage include dir (#21625)
- fix(coverage): error if no files found (#21615)
- fix(devcontainer): moved settings to customizations/vscode (#21512)
- fix(ext/napi): don't close handle scopes in NAPI as the pointers are invalid
  (#21629)
- fix(jupyter): Deno.test() panic (#21606)
- fix(lsp): apply specifier rewrite to CompletionItem::text_edit (#21564)
- fix(net): remove unstable check for unix socket listen (#21592)
- fix(node): add crypto.pseudoRandomBytes (#21649)
- fix(node): child_process IPC on Windows (#21597)
- fix(node): child_process kill cancel pending IPC reads (#21647)
- fix(node): return false from vm.isContext (#21568)
- fix(node): support resolving a package.json import to a builtin node module
  (#21576)
- fix(repl): remove stray debug log (#21635)
- fix: prompts when publishing (#21596)
- fix: urls for publishing (#21613)

### 1.39.0 / 2023.12.13

- Reland "fix(ext/console): fix inspecting iterators error. (#20720)" (#21370)
- Update doc for deno fmt `--no-semicolons` arg. (#21414)
- feat(compile): support "bring your own node_modules" in deno compile (#21377)
- feat(compile): support discovering modules for more dynamic arguments (#21381)
- feat(coverage): add html reporter (#21495)
- feat(coverage): add summary reporter (#21535)
- feat(cron): added the support for json type schedule to cron api (#21340)
- feat(ext/fetch): allow `Deno.HttpClient` to be declared with `using` (#21453)
- feat(ext/kv) add backoffSchedule to enqueue (#21474)
- feat(ext/web): add ImageData Web API (#21183)
- feat(fmt): support formatting code blocks in Jupyter notebooks (#21310)
- feat(lsp): debug log file (#21500)
- feat(lsp): provide quick fixes for specifiers that could be resolved sloppily
  (#21506)
- feat(streams): ReadableStream.read min option (#20849)
- feat(test): add default to --coverage option (#21510)
- feat(unstable): --unstable-unsafe-proto (#21313)
- feat(unstable): ability to resolve specifiers with no extension, specifiers
  for a directory, and TS files from JS extensions (#21464)
- feat(unstable): append commit versionstamp to key (#21556)
- feat: TypeScript 5.3 (#21480)
- feat: add suggestions to module not found error messages for file urls
  (#21498)
- feat: bring back WebGPU (#20812)
- feat: stabilize Deno.HttpServer.shutdown and Unix socket support (#21463)
- fix (doc): Typo in `runtime/README.md` (#20020)
- fix(cli/installer): percent decode name (#21392)
- fix(compile/npm): ignore symlinks to non-existent paths in node_modules
  directory (#21479)
- fix(coverage): escape source code in html coverage report (#21531)
- fix(coverage): rename --pretty to --detailed (#21543)
- fix(cron): move deprecated Deno.cron overload (#21407)
- fix(doc): ambient namespaces should have members as exports (#21483)
- fix(dts): `Deno.ChildProcess` actually implements `AsyncDisposable` (#21326)
- fix(ext/kv): throw error if already closed (#21459)
- fix(ext/node): ServerResponse getHeader() return undefined (#21525)
- fix(ext/node): add stubbed process.report (#21373)
- fix(ext/node): add util.parseArgs (#21342)
- fix(ext/node): allow null value for req.setHeader (#21391)
- fix(ext/node): basic vm.runInNewContext implementation (#21527)
- fix(ext/node): fix Buffer.copy when sourceStart > source.length (#21345)
- fix(ext/node): fix duplexify compatibility (#21346)
- fix(ext/node): fix os.freemem (#21347)
- fix(ext/node): include non-enumerable keys in `Reflect.ownKeys(globalThis)`
  (#21485)
- fix(ext/node): sign with PEM private keys (#21287)
- fix(ext/node): stub ServerResponse#flushHeaders (#21526)
- fix(ext/node): use primordials in ext/node/polyfills/_util (#21444)
- fix(ext/websocket): don't panic on bad resource id (#21431)
- fix(fmt): `"singleQuote": true` should prefer single quote—not always use one
  (#21470)
- fix(fmt): remove trailing comma for single type param in default export in jsx
  (#21425)
- fix(fmt/jupyter): handle "source" property that's a string (#21361)
- fix(lsp): handle byonm specifiers in jupyter notebooks (#21332)
- fix(node): setting process.exitCode should change exit code of process
  (#21429)
- fix(node/tls): fix NotValidForName for host set via socket / servername
  (#21441)
- fix(npm): do not create symlink for non-system optional dep in node_modules
  directory (#21478)
- fix(perm): allow-net with port 80 (#21221)
- fix(permissions): fix panics when revoking net permission (#21388)
- fix(runtime): Make native modal keyboard interaction consistent with browsers
  (#18453)
- fix(task): handle node_modules/.bin directory with byonm (#21386)
- fix(task): use exit code 127 for command not found and parse escaped parens
  (#21316)
- fix(unstable): Honor granular unstable flags in js runtime (#21466)
- fix(websockets): server socket field initialization (#21433)
- fix(zlib): handle no flush flag in handle_.write (#21432)
- fix: add more warnings when using sloppy imports (#21503)
- fix: allow reserved word 'mod' in exports (#21537)
- fix: batch upload authentication (#21397)
- fix: correct flag in tar & upload (#21327)
- fix: correct the batch upload length (#21401)
- fix: display unstable flags at bottom of help text (#21468)
- fix: don't error if a version already published (#21455)
- fix: error code used for duplicate version publish (#21457)
- fix: extraneous slash in tar & upload (#21349)
- fix: ignore more paths in dynamic arg module search (#21539)
- fix: implement child_process IPC (#21490)
- fix: use correct import map in tar & upload (#21380)
- perf(ext/ffi): switch from middleware to tasks (#21239)
- perf(ext/napi): port NAPI to v8 tasks (#21406)
- perf(ext/url): improve URLPattern perf (#21488)
- perf(ext/web): Avoid changing prototype by setting hostObjectBrand directly
  (#21358)
- perf(lsp): collect counts and durations of all requests (#21540)
- perf(lsp): instrument all ops with performance marks (#21536)
- perf(lsp): simplify some of the startup code (#21538)
- perf(lsp): use null types instead of stub modules (#21541)
- perf(node/fs): faster `existsSync` when not exists (#21458)
- perf: move "cli/js/40_testing.js" out of main snapshot (#21212)

### 1.38.5 / 2023.12.05

- feat(unstable): kv.watch() (#21147)
- perf(lsp): better op performance logging (#21423)
- perf(lsp): check tsc request cancellation before execution (#21447)
- perf(lsp): fix redundant clones for ts responses (#21445)
- perf(lsp): fix redundant serialization of sources (#21435)

### 1.38.4 / 2023.11.30

- fix(node): `spawnSync`'s `status` was incorrect (#21359)
- perf(lsp): add performance marks for TSC requests (#21383)
- perf(lsp): avoid redundant getNavigationTree() calls (#21396)
- perf(lsp): cancel ts requests on future drop (#21387)
- perf(lsp): remove throttling of cancellation token (#21395)

### 1.38.3 / 2023.11.24

- feat(unstable): tar up directory with deno.json (#21228)
- fix(ext,runtime): add missing custom inspections (#21219)
- fix(ext/http): avoid lockup in graceful shutdown (#21253)
- fix(ext/http): fix crash in dropped Deno.serve requests (#21252)
- fix(ext/node): fix node:stream.Writable (#21297)
- fix(ext/node): handle closing process.stdin more than once (#21267)
- fix(ext/url): add deno_console dependency for bench (#21266)
- fix(fmt): maintain parens for jsx in member expr (#21280)
- fix(lsp): force shutdown after a timeout (#21251)
- fix(runtime): fix for panic in classic workers (#21300)
- fix(swc): support jsx pragma when hashbang present (#21317)
- fix: 'Promise was collected' error in REPL/jupyter (#21272)
- fix: Deno.noColor should not be true when NO_COLOR is empty string (#21275)

### 1.38.2 / 2023.11.17

- feat(ext/web): add `AbortSignal.any()` (#21087)
- feat(lsp): upgrade check on init and notification (#21105)
- fix(cli): Allow executable name start with digit (#21214)
- fix(doc): issue discovering re-exports of re-exports sometimes (#21223)
- fix(ext/node): Re-enable alloc max size test (#21059)
- fix(ext/node): add APIs perf_hook.performance (#21192)
- fix(ext/node): implement process.geteuid (#21151)
- fix(ext/web): Prevent TextDecoderStream resource leak on stream cancellation
  (#21074)
- fix(ext/web): webstorage has trap for symbol (#21090)
- fix(install): should work with non-existent relative root (#21161)
- fix(lsp): update tsconfig after refreshing settings on init (#21170)
- fix(node/http): export globalAgent (#21081)
- fix(npm): support cjs entrypoint in node_modules folder (#21224)
- fix(runtime): fix Deno.noColor when stdout is not tty (#21208)
- fix: improve `deno doc --lint` error messages (#21156)
- fix: use short git hash for deno version (#21218)
- perf(cli): strace mode for ops (undocumented) (#21131)
- perf(ext/http): Object pooling for HttpRecord and HeaderMap (#20809)
- perf: lazy bootstrap options - first pass (#21164)
- perf: move jupyter esm out of main snapshot (#21163)
- perf: snapshot runtime ops (#21127)
- perf: static bootstrap options in snapshot (#21213)

### 1.38.1 / 2023.11.10

- feat(ext/kv): increase checks limit (#21055)
- fix small Deno.createHttpClient typo in lib.deno.unstable.d.ts (#21115)
- fix(byonm): correct resolution for scoped packages (#21083)
- fix(core/types): `Promise.withResolvers`: Unmark callback param as optional
  (#21085)
- fix(cron): update Deno.cron doc example (#21078)
- fix(doc): `deno doc --lint mod.ts` should output how many files checked
  (#21084)
- fix(doc): require source files if --html or --lint used (#21072)
- fix(ext): use `String#toWellFormed` in ext/webidl and ext/node (#21054)
- fix(ext/fetch): re-align return type in op_fetch docstring (#21098)
- fix(ext/http): Throwing Error if the return value of Deno.serve handler is not
  a Response class (#21099)
- fix(node): cjs export analysis should probe for json files (#21113)
- fix(node): implement createPrivateKey (#20981)
- fix(node): inspect ancestor directories when resolving cjs re-exports during
  analysis (#21104)
- fix(node): use closest package.json to resolve package.json imports (#21075)
- fix(node/child_process): properly normalize stdio for 'spawnSync' (#21103)
- fix(node/http): socket.setTimeout (#20930)
- fix(test) reduce queue persistence test time from 60 secs to 6 secs (#21142)
- perf: lazy `atexit` setup (#21053)
- perf: remove knowledge of promise IDs from deno (#21132)

### 1.38.0 / 2023.11.01

- feat(cron) implement Deno.cron() (#21019)
- feat(doc): display non-exported types referenced in exported types (#20990)
- feat(doc): improve non-exported diagnostic (#21033)
- feat(doc): support multiple file entry (#21018)
- feat(ext/kv): support key expiration in remote backend (#20688)
- feat(ext/web): EventSource (#14730)
- feat(ext/websocket): split websocket read/write halves (#20579)
- feat(ext/websocket): use rustls-tokio-stream instead of tokio-rustls (#20518)
- feat(ext/websocket): websockets over http2 (#21040)
- feat(lsp): respect "typescript.preferences.quoteStyle" when deno.json is
  absent (#20891)
- feat(task): add `head` command (#20998)
- feat(unstable): `deno run --env` (#20300)
- feat(unstable): ability to `npm install` then `deno run main.ts` (#20967)
- feat(unstable): allow bare specifier for builtin node module (#20728)
- feat: `deno doc --lint` (#21032)
- feat: deno doc --html (#21015)
- feat: deno run --unstable-hmr (#20876)
- feat: disposable Deno resources (#20845)
- feat: enable Array.fromAsync (#21048)
- feat: granular --unstable-* flags (#20968)
- feat: precompile JSX (#20962)
- feat: rename Deno.Server to Deno.HttpServer (#20842)
- fix(ext/ffi): use anybuffer for op_ffi_buf_copy_into (#21006)
- fix(ext/http): patch regression in variadic args to serve handler (#20796)
- fix(ext/node): adapt dynamic type checking to Node.js behavior (#21014)
- fix(ext/node): process.argv0 (#20925)
- fix(ext/node): tty streams extends net socket (#21026)
- fix(lsp): don't commit registry completions on "/" (#20902)
- fix(lsp): include mtime in tsc script version (#20911)
- fix(lsp): show diagnostics for untitled files (#20916)
- fix(node): resolve file.d specifiers in npm packages (#20918)
- fix(polyfill): correctly handle flag when its equal 0 (#20953)
- fix(repl): jsxImportSource was not working (#21049)
- fix(repl): support transforming JSX/TSX (#20695)
- fix(test): --junit-path should handle when the dir doesn't exist (#21044)
- fix(unstable/byonm): improve error messages (#20987)
- fix: add 'unstable' property to config json schema (#20984)
- fix: add missing `Object.groupBy()` and `Map.groupBy()` types (#21050)
- fix: implement node:tty (#20892)
- fix: improved using declaration support (#20959)
- perf(ext/streams): optimize streams (#20649)
- perf(lsp): cleanup workspace settings scopes (#20937)
- perf(lsp): fix redundant walk when collecting tsc code lenses (#20974)
- perf: use deno_native_certs crate (#18072)

### 1.37.2 / 2023.10.12

- feat(ext/web): cancel support for TransformStream (#20815)
- feat(lsp): jupyter notebook analysis (#20719)
- feat(lsp): send "deno/didChangeDenoConfiguration" notifications (#20827)
- feat(unstable): add Deno.jupyter.display API (#20819)
- feat(unstable): add unix domain socket support to Deno.serve (#20759)
- feat(unstable): Await return from `Jupyter.display` (#20807)
- feat(unstable): send binary data with `Deno.jupyter.broadcast` (#20755)
- feat(unstable): send Jupyter messaging metadata with `Deno.jupyter.broadcast`
  (#20714)
- feat(unstable): support Deno.test() (#20778)
- fix(bench): use total time when measuring wavg (#20862)
- fix(cli): Support using both `--watch` and `--inspect` at the same time
  (#20660)
- fix(cli): panic with __runtime_js_sources (#20704)
- fix(ext/ffi): use anybuffer for op_ffi_ptr_of (#20820)
- fix(ext/formdata): support multiple headers in FormData (#20801)
- fix(ext/http): Deno.Server should not be thenable (#20723)
- fix(ext/kv): send queue wake messages accross different kv instances (#20465)
- fix(ext/node): don't call undefined nextTick fn (#20724)
- fix(ext/node): fix TypeError in Buffer.from with base64url encoding. (#20705)
- fix(ext/node): implement uv.errname (#20785)
- fix(ext/web): writability of `ReadableStream.from` (#20836)
- fix(jupyter): Rename logo assets so they are discoverable (#20806)
- fix(jupyter): keep `this` around (#20789)
- fix(jupyter): more robust Deno.jupyter namespace (#20710)
- fix(lsp): allow formatting vendor files (#20844)
- fix(lsp): normalize "deno:" urls statelessly (#20867)
- fix(lsp): percent-encode host in deno: specifiers (#20811)
- fix(lsp): show diagnostics for type imports from untyped deps (#20780)
- fix(node/buffer): utf8ToBytes should return a Uint8Array (#20769)
- fix(node/http2): fixes to support grpc (#20712)
- fix(npm): upgrade to deno_npm 0.15.2 (#20772)
- fix(upgrade): use tar.exe to extract on Windows (#20711)
- fix: define window.name (#20804)
- fix: upgrade dprint-plugin-markdown 0.16.2 and typescript 0.88.1 (#20879)
- perf(ext/web): optimize DOMException (#20715)
- perf(ext/web): optimize structuredClone without transferables (#20730)
- perf(lsp): fix redundant file reads (#20802)
- perf(lsp): optimize formatting minified files (#20829)
- perf(node): use faster utf8 byte length in Buffer#from (#20746)

### 1.37.1 / 2023.09.27

- feat(ext/web): use readableStreamDefaultReaderRead in
  resourceForReadableStream (#20622)
- feat(kv_queues): increase max queue delay to 30 days (#20626)
- feat(lsp): cache all dependencies quick fix (#20665)
- feat(lsp): support more vscode built-in settings (#20679)
- feat(unstable): add `Deno.jupyter.broadcast` API (#20656)
- fix(cli/test): clear connection pool after tests (#20680)
- fix(ext/http): ensure that resources are closed when request is cancelled
  (#20641)
- fix(ext/node): Fix invalid length variable reference in blitBuffer (#20648)
- fix(ext/node): simplified array.from + map (#20653)
- fix(ext/web): Aggregate small packets for Resource implementation of
  ReadableStream (#20570)
- fix(jupyter): await Jupyter.display evaluation (#20646)
- fix(kv): unflake kv unit tests (#20640)
- fix(kv_queues): graceful shutdown (#20627)
- fix(lsp): allow query strings for "deno:/status.md" (#20697)
- fix(lsp): resolve remote import maps (#20651)
- fix(lsp): show related information for tsc diagnostics (#20654)
- fix(node): point process.version to Node 18.18.0 LTS (#20597)
- fix(node): supported arguments to `randomFillSync` (#20637)
- fix(node/package_json): Avoid panic when "exports" field is null (#20588)
- fix(upgrade): error instead of panic on unzip failure (#20691)
- perf(ext/fetch): use new instead of createBranded (#20624)
- perf(test): use core.currentUserCallSite (#20669)
- perf(test): use fast ops for deno test register (#20670)

### 1.37.0 / 2023.09.19

- feat: Add "deno jupyter" subcommand (#20337, #20552, #20530, #20537, #20546)
- feat(test): add TAP test reporter (#14390, #20073)
- feat(ext/node): http2.connect() API (#19671)
- feat(ext/web): Add name to `Deno.customInspect` of File objects (#20415)
- feat(lint): `--rules` print all rules (#20256)
- feat(lockfile): add redirects to the lockfile (#20262)
- feat(lsp): WorkspaceSettings::disablePaths (#20475)
- feat(lsp): enable via config file detection (#20334, #20349)
- feat(lsp): include source in auto import completion label (#20523)
- feat(lsp): npm specifier completions (#20121)
- feat(lsp): provide the deno.cache command server-side (#20111)
- feat(lsp): update imports on file rename (#20245)
- feat(test): Add Deno.test.ignore and Deno.test.only (#20365)
- feat(unstable): package manager (#20517)
- feat: TypeScript 5.2 (#20425)
- feat: explicit resource management in TypeScript (#20506)
- feat: lockfile v3 (#20424)
- feat: support import attributes (#20342)
- fix(cli): ensure that an exception in getOwnPropertyDescriptor('constructor')
  doesn't break Deno.inspect (#20568)
- fix(cli): for main-module that exists in package.json, use the version defined
  in package.json directly (#20328)
- fix(compile): support providing flags as args (#20422)
- fix(evt/kv): Add serde feature to uuid (#20350)
- fix(ext/crypto): remove EdDSA alg key checks and export (#20331)
- fix(ext/http): create a graceful shutdown API (#20387)
- fix(ext/http): ensure aborted bodies throw (#20503)
- fix(ext/kv): add a warning for listenQueue if used with remote KV (#20341)
- fix(ext/kv): same `expireIn` should generate same `expireAt` (#20396)
- fix(ext/node): implement AES GCM cipher (#20368)
- fix(ext/node): remove unnecessary and incorrect type priority_t (#20276)
- fix(ext/node/ops/zlib/brotli): Allow decompressing more than 4096 bytes
  (#20301)
- fix(fmt/markdown): improve ignore comment handling (#20421)
- fix(init): skip existing files instead of erroring (#20434)
- fix(lsp): always enable semantic tokens responses (#20440)
- fix(lsp): force correct media type detection from tsc (#20562)
- fix(lsp): include JSON modules in local import completions (#20536)
- fix(lsp): match enable_paths by whole path components (#20470)
- fix(lsp): pass quote preference to tsc (#20547)
- fix(lsp): prefer local auto-import specifiers (#20539)
- fix(lsp): properly handle disabled configuration requests (#20358)
- fix(lsp): recreate npm search cache when cache path changes (#20327)
- fix(lsp): refresh npm completions on each character (#20565)
- fix(lsp): respect configured exclusions for testing APIs (#20427)
- fix(lsp): restore tsc's quick fix ordering (#20545)
- fix(lsp): sort quickfix actions (#17221)
- fix(node): Bump hardcoded version to latest (#20366)
- fix(node/child_process): don't crash on undefined/null value of an env var
  (#20378)
- fix(node/http): correctly send `Content-length` header instead of
  `Transfer-Encoding: chunked` (#20127)
- fix(npm): properly handle legacy shasum of package (#20557)
- fix(runtime/permissions): Resolve executable specifiers in allowlists and
  queries (#14130)
- fix(test): apply filter before checking for "only" (#20389)
- fix(test): share fail fast tracker between threads (#20515)
- fix: `Deno.Command` - improve error message when `cwd` is not a directory
  (#20460)
- fix: don't show filtered test suites as running (#20385)
- fix: empty include in config file excludes all (#20404)
- fix: exclude internal JS files from coverage (#20448)
- fix: init v8 platform once on main thread (#20495)
- fix: output traces for op sanitizer in more cases (#20494)
- perf(ext/http): optimize `set_response` for small responses (#20527)
- perf(ext/node): Optimise Buffer string operations (#20158)
- perf(ext/streams): optimize async iterator (#20541)
- perf(node/net): optimize socket reads for 'npm:ws' package (#20449)
- perf: improve async op santizer speed and accuracy (#20501)
- perf: make `deno test` 10x faster (#20550)

### 1.36.4 / 2023.09.01

- feat(ext/kv): connect to remote database (#20178)
- feat(node): use i32 for priority_t on MacOS and {Free,Open}BSD (#20286)
- fix(bench): explicit timers don't force high precision measurements (#20272)
- fix(ext/http): don't panic on stream responses in cancelled requests (#20316)
- fix(ext/kv): don't panic if listening on queues and KV is not closed (#20317)
- fix(ext/node): fix argv[1] in Worker (#20305)
- fix(ext/node): shared global buffer unlock correctness fix (#20314)
- fix(ext/tls): upgrade webpki version (#20285)
- fix(fmt/markdown): ignore trailing words in code block info string for
  language detection (#20310)
- fix(kv) increase number of allowed mutations in atomic (#20126)
- fix(lsp): delete test modules with all tests deleted (#20321)
- fix(lsp): implement deno.suggest.completeFunctionCalls (#20214)
- fix(lsp): test explorer panic on step result (#20289)
- fix(lsp/testing): don't queue modules without tests (#20277)
- fix(lsp/testing): use full ancestry to compute static id of step (#20297)
- fix(napi): ignore tsfn recv error (#20324)
- fix(network): adjust Listener type params (#18642)
- fix(node): propagate create cipher errors (#20280)
- fix(node/http): don't leak resources on destroyed request (#20040)
- fix: unexpected lsp function arg comma completion (#20311)

### 1.36.3 / 2023.08.24

- fix(build): socket2 compile error
- fix(cli): add timeout on inspector tests (#20225)
- fix(ext/node): simultaneous reads can leak into each other (#20223)
- fix(ext/web): add stream tests to detect v8slice split bug (#20253)
- fix(ext/web): better handling of errors in resourceForReadableStream (#20238)
- fix(lint): erroneous remove await in async (#20235)
- fix: add missing `URL.canParse()` types (#20244)

### 1.36.2 / 2023.08.21

- feat(ext/kv): key expiration (#20091)
- feat(ext/node): eagerly bootstrap node (#20153)
- feat(unstable): Improve FFI types (#20215)
- fix(cli) error gracefully when script arg is not present and `--v8-flags` is
  present in `deno run` (#20145)
- fix(cli): handle missing `now` field in cache (#20192)
- fix(ext/fetch): clone second branch chunks in Body.clone() (#20057)
- fix(ext/http): ensure request body resource lives as long as response is alive
  (#20206)
- fix(ext/kv): retry transaction on `SQLITE_BUSY` errors (#20189)
- fix(ext/net): implement a graceful error on an invalid SSL certificate
  (#20157)
- fix(ext/node): allow for the reassignment of userInfo() on Windows (#20165)
- fix(ext/node): support dictionary option in zlib init (#20035)
- fix(lsp): pass fmt options to completion requests (#20184)
- fix(node): don't print warning on process.dlopen.flags (#20124)
- fix(node): implement TLSSocket._start (#20120)
- fix(node): object keys in publicEncrypt (#20128)
- fix(node/http): emit error when addr in use (#20200)
- fix(npm): do not panic providing file url to require.resolve paths (#20182)
- fix(require): use canonicalized path for loading content (#20133)
- fix(runtime): navigator.userAgent in web worker (#20129)
- fix(runtime): use host header for inspector websocket URL (#20171)
- fix(test): JUnit reporter includes file, line and column attributes (#20174)
- fix(unstable): disable importing from the vendor directory (#20067)
- fix: release ReadeableStream in fetch (#17365)
- perf(ext/event): always set timeStamp to 0 (#20191)
- perf(ext/event): optimize Event constructor (#20181)
- perf(ext/event): optimize addEventListener options converter (#20203)
- perf(ext/event): replace ReflectHas with object lookup (#20190)
- perf(ext/headers): cache iterableHeaders for immutable Headers (#20132)
- perf(ext/headers): optimize getHeader using for loop (#20115)
- perf(ext/headers): optimize headers iterable (#20155)
- perf(ext/headers): use regex.test instead of .exec (#20125)
- perf(ext/http): use ServeHandlerInfo class instead of object literal (#20122)
- perf(ext/node): cache `IncomingMessageForServer.headers` (#20147)
- perf(ext/node): optimize http headers (#20163)
- perf(ext/request): optimize Request constructor (#20141)
- perf(ext/request): optimize validate and normalize HTTP method (#20143)
- perf(ext/urlpattern): optimize URLPattern.exec (#20170)
- perf(http): use Cow<[u8]> for setting header (#20112)

### 1.36.1 / 2023.08.10

- feat(unstable): rename `deno_modules` to `vendor` (#20065)
- fix(ext/abort): trigger AbortSignal events in correct order (#20095)
- fix(ext/file): resolve unresolved Promise in Blob.stream (#20039)
- fix(ext/http): serveHttp brotli compression level should be fastest (#20058)
- fix(ext/http): unify default gzip compression level (#20050)
- fix(ext/timers): some timers are not resolved (#20055)
- fix(fmt): do not insert expr stmt leading semi-colon in do while stmt body
  (#20093)
- fix(node): polyfill process.title (#20044)
- fix(node): repl._builtinLibs (#20046)
- fix(node/async_hooks): don't pop async context frame if stack if empty
  (#20077)
- fix(test): handle ASCII escape chars in test name (#20081)
- fix(test): make test runner work when global setTimeout is replaced (#20052)
- fix(test): use only a single timeout for op sanitizers (#20042)
- fix(unstable): vendor cache override should handle forbidden windows directory
  names (#20069)
- fix(unstable): vendor cache should support adding files to hashed directories
  (#20070)
- perf(ext/headers): use .push loop instead of spread operator (#20108)

### 1.36.0 / 2023.08.03

- feat(bench): add BenchContext::start() and BenchContext::end() (#18734)
- feat(bench): print iter/s in the report (#19994)
- feat(cli): Add dot test reporter (#19804)
- feat(cli): Adding JUnit test reports (#19747)
- feat(compile): Add `--no-terminal` to compile command (#17991)
- feat(ext/http): Upgrade to hyper1.0-rc4 (#19987)
- feat(ext/websocket): allow HTTP(S) protocol in URL (#19862)
- feat(node): add polyfill for node:test module (#20002)
- feat(node/os): implement getPriority, setPriority & userInfo (#19370)
- feat(npm): support running non-bin scripts in npm pkgs via `deno run` (#19975)
- feat(permissions): add "--deny-*" flags (#19070)
- feat(unstable): optional `deno_modules` directory (#19977)
- feat(unstable/lsp): support navigating to deno_modules folder (#20030)
- feat: Deno.createHttpClient allowHost (#19689)
- fix(Deno.serve): accessing .url on cloned request throws (#19869)
- fix(bench): iter/s calculation (#20016)
- fix(check): should bust check cache when json module or npm resolution changes
  (#19941)
- fix(ext/compression): throw TypeError on corrupt input (#19979)
- fix(ext/fs): fix MaybeArc when not sync_fs (#19950)
- fix(ext/node): fix import json using npm specifier (#19723)
- fix(lsp): handle import mapped `node:` specifier (#19956)
- fix(node): node:test reports correct location (#20025)
- fix(node): package path not exported error - add if types resolution was
  occurring (#19963)
- fix(npm): improve declaration resolution for filename with different
  extensions (#19966)
- fix(repl): highlight from ident in import from or export from (#20023)
- fix(test): request cloning should throw if body stream is locked (#19990)
- fix: call setIsTrusted for generated events (MessageEvent) (#19919)
- fix: deno diagnostic - clarify where to put triple-slash directive (#20009)
- fix: do not include jsx without `@ts-check` in tsc roots (#19964)
- fix: error on invalid & unsupported jsx compiler options (#19954)
- fix: make "suggest.autoImports" to switch completions from external modules
  (#19845)
- fix: regression in workers using dynamic imports (#20006)
- fix: retry module download once if server errored (#17252)
- perf(ext/ffi): Avoid receiving on FFI async work channel when no
  UnsafeCallback exists (#19454)
- perf: faster node globals access in cjs (#19997)

### 1.35.3 / 2023.07.26

- feat(runtime): sys_info.rs - Use KERN_OSRELEASE on {Free,Open}BSD (#19849)
- fix(cli): build script panics on musl due to glibc_version check (#19913)
- fix(cli): output file handling in deno upgrade (#18994)
- fix(cli/init): update to `assert/mod.ts` (#19924)
- fix(cli/test): fix clear screen behavior when run `deno test --watch` (#19888)
- fix(ext/http): Error on deprecated/unavailable features (#19880)
- fix(ext/http): Quietly ignore invalid status codes (#19936)
- fix(ext/net): fix string port number handling in listen (#19921)
- fix(ext/node): inspector with seggregated globals (#19917)
- fix(lint): allow to use --rules with --rules-tags (#19754)
- fix(lsp): auto-discover deno.json in more cases (#19894)
- fix(lsp): handle watched files events from symlinked config files (#19898)
- fix(node): add writable and readable fields to FakeSocket (#19931)
- fix(node/http): add encrypted field to FakeSocket (#19886)
- fix(node_compat): Wrap require resolve exports in try catch block (#19592)
- fix(task): ensure quoted strings are maintained mid-word (#19944)
- fix: deno info should respect import map (#19781)
- perf(lsp): format in a blocking task (#19883)
- perf: cache node resolution when accesing a global (#19930)

### 1.35.2 / 2023.07.20

- fix(bench): run warmup benchmark to break JIT bias (#19844)
- fix(ext/node): check if resource can be used with write_vectored (#19868)
- fix(ext/node): fix stream/promises export (#19820)
- fix(ext/node): properly segregate node globals (#19307)
- fix(napi): update env_test.js (#19876)
- fix(node): add process.dlopen API (#19860)
- fix(node): improve error message requiring non-npm es module (#19856)
- fix(node): improve require esm error messages (#19853)
- fix(node/http): call callback after request is sent (#19871)
- fix(node/net): Server connection callback include socket value (#19779)
- fix(npm): improve error message importing non-existent file in a node_modules
  npm package (#19835)
- fix(npm): improve error message on directory import in npm package (#19538)
- fix(npm): support dynamic import of Deno TS from npm package (#19858)
- fix(runtime): print process name in case of spawn error (#19855)
- fix(tsc): more informative diagnostic when `Deno` does not exist (#19825)
- fix(vendor): do not panic vendoring with jsxImportSource and no jsx files
  (#19837)

### 1.35.1 / 2023.07.12

- fix(ext/http): Use brotli compression params (#19758)
- fix(lsp): exclude files in deno.json "exclude" (#19791)
- fix(lsp): remove quotes and period surrounding specifier in uncached messages
  (#19794)
- fix(lsp): stop diagnostics flickering (#19803)
- fix(node/http): add destroy to FakeSocket (#19796)
- fix(node/http): allow callback in first argument of end call (#19778)
- fix(node/http): server use FakeSocket and add end method (#19660)
- fix(vendor): support import mapped jsxImportSource (#19724)
- fix: remove unstable check for Deno.listenTls#alpnProtocols (#19732)
- perf(ext/node): native vectored write for server streams (#19752)
- perf(ext/node): optimize net streams (#19678)
- perf(ext/websocket): optimize server websocket js (#19719)
- perf(node/async_hooks): optimize AsyncLocalStorage (#19729)
- perf: add setup cache for node_modules folder (#19787)

### 1.35.0 / 2023.07.04

- feat: add more Deno.errors classes (#19514)
- feat: ReadableStream.from (#19446)
- feat: stabilize 'alpnProtocols' setting (#19704)
- feat: Stabilize Deno.serve() API (#19141)
- feat: upgrade to TypeScript 5.1.6 (#19695)
- feat(ext/fetch): add Headers#getSetCookie (#13542)
- feat(ext/url): URLSearchParams two-argument delete() and has() (#19654)
- feat(lock): skip saving declaration files in the lockfile (#19447)
- feat(lsp): basic support of auto-imports for npm specifiers (#19675)
- feat(lsp): support import maps in quick fix and auto-imports (#19692)
- fix: add `exactOptionalPropertyTypes` for configuration file JSON schema
  (#19647)
- fix: bump default @types/node version range to 18.16.19 (#19706)
- fix(cli): don't store blob and data urls in the module cache (#18581)
- fix(cli): Fix the bug where the command description is not displayed. (#19604)
- fix(cli/napi): `napi_get_buffer_info` accepts `ArrayBufferView` … (#19551)
- fix(cli/napi): property with getter/setter always failed (#19562)
- fix(console): correct the parseCssColor algorithm (#19645)
- fix(dts): make globals available on globalThis (#19438)
- fix(ext/crypto): Fix WebCrypto API's deriveKey (#19545)
- fix(ext/fs): fix boolean checks in JS parser (#19586)
- fix(ext/http): Catch errors in eager stream timeout to avoid uncaught promise
  rejections (#19691)
- fix(ext/kv): expose Deno.AtomicOperation (#19674)
- fix(ext/node): Define performance.timeOrigin as getter property (#19714)
- fix(ext/node): ignore cancelled timer when node timer refresh (#19637)
- fix(ext/node): support brotli APIs (#19223)
- fix(ext/websocket): Ensure that errors are available after async response
  returns (#19642)
- fix(node/http): add setKeepAlive to FakeSocket (#19659)
- fix(npm): escape export identifier in double quoted string (#19694)
- fix(npm): handle more reserved words as cjs exports (#19672)
- fix(npm): support siblings that are peer dependencies of each other (#19657)

### 1.34.3 / 2023.06.15

- feat(UNSTABLE) kv queue implementation (#19459)
- fix(cli): avoid crash on import of invalid module names (#19523)
- fix(compile): some npm dependencies were missing in compiled output (#19503)
- fix(config): do not canonicalize config file path before loading (#19436)
- fix(ext/http): Include hostname in onListen argument (#19497)
- fix(ext/http): replace await Deno.serve with await Deno.serve().finished
  (#19485)
- fix(ext/node): HTTPS server (#19362)
- fix(ext/node): handle 'upgrade' responses (#19412)
- fix(ext/node): make Buffer.slice be the same as subarray (#19481)
- fix(ext/websockets): ensure we fully send frames before close (#19484)
- fix(fmt): do not panic formatting json with multiple values (#19442)
- fix(lsp): don't pre-load documents matched in the config file's "exclude"
  (#19431)
- fix(lsp): update import map config when deno.json changes (#19476)
- fix(ext/node): Worker constructor doesn't check type: module of package.json
  (#19480)
- fix(npm): warn when tarball contains hardlink or symlink (#19474)
- fix: reload config files on watcher restarts (#19487)
- perf(ext/http): from_maybe_shared_unchecked for header values (#19478)
- perf(http): cache verified headers (#19465)
- perf(node): cache realpath_sync calls in read permission check (#19379)
- perf(serve): hoist promise error callback (#19456)
- perf(serve): hoist repeated condition (#19449)
- perf(web): optimize timer resolution (#19493)
- perf: don't run microtask checkpoint if macrotask callback did no work
  (#19492)
- perf: optimize ByteString checks, hoist server rid getter (#19452)

### 1.34.2 / 2023.06.08

- fix: do not show cache initialization errors if stderr is piped (#18920)
- fix: upgrade to deno_ast 0.27 (#19375)
- fix(cli): formatting bench with colors (#19323)
- fix(ext/console): fix inspecting large ArrayBuffers (#19373)
- fix(ext/crypto): fix JWK import of Ed25519 (#19279)
- fix(ext/web): Copy EventTarget list before dispatch (#19360)
- fix(ext/websocket): Close socket on bad string data (#19424)
- fix(kv) run sqlite transactions via spawn_blocking (#19350)
- fix(napi): don't panic if symbol can't be found (#19397)
- fix(node): add missing process.reallyExit method (#19326)
- fix(node): Added base implementation of FileHandle (#19294)
- fix(node): don't close stdio streams (#19256)
- fix(node): FileHandle.close() (#19357)
- fix(node): FileHandle.read() (#19359)
- fix(node): FileHandle.write() (#19385)
- fix(node): map stdio [0, 1, 2] to "inherit" (#19352)
- fix(node): Very basic node:http2 support (#19344)
- fix(node): proper url handling (#19340)
- fix(repl): correctly print string exception (#19391)
- fix(runtime): add missing SIGIOT alias to SIGABRT (#19333)
- perf(cli): conditionally load typescript declaration files (#19392)
- perf(ext/http): Add a sync phase to http serving (#19321)
- perf(ext/http): Migrate op_http_get_request_headers to v8::Array (#19354)
- perf(ext/http): Migrate op_http_get_request_method_and_url to v8::Array
  (#19355)
- perf(ext/http): Use flat list of headers for multiple set/get methods (#19336)
- perf(ext/websocket): Make send sync for non-stream websockets (#19376)
- perf(ext/websocket): Reduce GC pressure & monomorphize op_ws_next_event
  (#19405)
- perf(ext/websocket): monomorphize code (#19394)
- perf(http): avoid flattening http headers (#19384)
- perf: optimize RegExp usage in JS (#19364)
- perf: use sendto syscalls (#19414)

### 1.34.1 / 2023.05.29

- fix(compile): handle when DENO_DIR is readonly (#19257)
- fix(compile): implicit read permission to npm vfs (#19281)
- fix(compile): improve panic message when stripping root path fails (#19258)
- fix(compile): inline symlinks as files outside node_modules dir and warn for
  directories (#19285)
- fix(ext/http): fix a possible memleak in Brotli (#19250)
- fix(napi): clear currently registering module slot (#19249)
- fix(napi): properly handle arguments in napi_get_cb_info (#19269)
- fix(node): http.IncomingMessageForClient.complete (#19302)
- fix(node): make 'v8.setFlagsFromString' a noop (#19271)
- fix: don't print release notes on version check prompt (#19252)
- fix: use proper ALPN protocols if HTTP client is HTTP/1.1 only (#19303)

### 1.34.0 / 2023.05.24

- BREAKING(unstable): change return type of Deno.serve() API (#19189)
- feat(cli): add `nodeModulesDir` option to config file (#19095)
- feat(cli): top-level `exclude` field in `deno.json` (#17778)
- feat(ext/fs): add isBlockDevice, isCharDevice, isFifo, isSocket to FileInfo
  (#19008)
- feat(ext/http): Add support for trailers w/internal API (HTTP/2 only) (#19182)
- feat(ext/http): Brotli Compression (#19216)
- feat(ext/http): ref/unref for server (#19197)
- feat(lsp): support lockfile and node_modules directory (#19203)
- feat(runtime): Provide environment-configurable options for tokio parameters
  (#19173)
- feat(task): glob expansion (#19084)
- feat(unstable): add more options to Deno.createHttpClient (#17385)
- feat(vendor): support for npm specifiers (#19186)
- feat: add support for globs in the config file and CLI arguments for files
  (#19102)
- feat: top level package.json install when node_modules dir is explicitly opted
  into (#19233)
- fix(ext/node): ClientRequest.setTimeout(0) should remove listeners (#19240)
- fix(ext/node): add basic node:worker_threads support (#19192)
- fix(ext/web): improve timers resolution for 0ms timeouts (#19212)
- fix(napi): add napi_async_init and napi_async_destroy (#19234)
- fix(node): add http.Server.unref() (#19201)
- fix(node): duplicate node_module suffixes (#19222)
- fix(node): fire 'unhandledrejection' event when using node: or npm: imports
  (#19235)
- fix(node): make sure "setImmediate" is not clamped to 4ms (#19213)
- fix(npm): `process` not defined in readline (#19184)
- fix(npm): better handling of optional peer dependencies (#19236)
- fix(npm): create `node_modules/.deno/node_modules` folder (#19242)
- fix(npm): run pre and post tasks if present (#19178)
- fix(npm): store npm binary command resolution in lockfile (#19219)

### 1.33.4 / 2023.05.18

- fix(ext/web): Request higher-resolution timer on Windows if user requests
  setTimeout w/short delay (#19149)
- feat(node/crypto): Builtin Diffie-Hellman Groups (#19137)
- feat(node/crypto): Diffie Hellman Support (#18943)
- fix(cli/napi): handle finalizers (#19168)
- fix(deno/upgrade): allow --version vX.Y.Z (#19139)
- fix(dts): move BroadcastChannel type to lib.deno.unstable.d.ts (#19108)
- fix(ext/http): Ensure cancelled requests don't crash Deno.serve (#19154)
- fix(ext/node): fix whatwg url formatting (#19146)
- fix(ext/node): make nodeGlobalThis configurable (#19163)
- fix(ext/webidl): change createPromiseConverter (#16367)
- fix(ext/websocket): order of ws writes (#19131)
- fix(fetch): Correctly decode `multipart/form-data` names and filenames
  (#19145)
- fix(kv): kv.close() interrupts in-flight operations (#19076)
- fix(lsp): increase default max heap size to 3Gb (#19115)
- fix(napi): BigInt related APIs (#19174)
- fix(node): export diagnostics_channel module (#19167)
- fix(node): export punycode module (#19151)
- fix(node): support passing parent stdio streams (#19171)
- fix(npm): add performance.markResourceTiming sham (#19123)
- fix(npm): improved optional dependency support (#19135)
- fix(runtime): Box the main future to avoid blowing up the stack (#19155)
- fix(runtime): Example hello_runtime panic (#19125)
- fix: support "fetch" over HTTPS for IP addresses (#18499)

### 1.33.3 / 2023.05.12

- feat(compile): unstable npm and node specifier support (#19005)
- feat(ext/http): Automatic compression for Deno.serve (#19031)
- feat(lsp): ability to configure document pre-load limit (#19097)
- feat(node): add `Module.runMain()` (#19080)
- fix(cli): upgrade to Typescript 5.0.4 (#19090)
- fix(console): handle error when inspecting promise-like (#19083)
- fix(core): always report the first error on unhandled rejection (#18992)
- fix(core): let V8 drive extension ESM loads (#18997)
- fix(dts): align `seekSync` `position` arg with `seek` (#19077)
- fix(ext/ffi): Callbacks panic on returning isize (#19022)
- fix(ext/ffi): UnsafeCallback can hang with 'deno test' (#19018)
- fix(ext/fs): add more context_path (#19101)
- fix(ext/http): Ensure Deno.serve works across --watch restarts (#18998)
- fix(lsp): hard to soft error when unable to get completion info (#19091)
- fix(lsp): preload documents when `deno.documentPreloadLimit` changes (#19103)
- fix(node): conditional exports edge case (#19082)
- fix(node): expose channels in worker_threads (#19086)
- fix(npm): make http2 module available, make 'nodeGlobalThisName' writable
  (#19092)
- fix(runtime): `ChildProcess::kill()` doesn't require additional perms (#15339)
- fix(vendor): better handling of redirects (#19063)
- perf(ext/ffi): Use `Box<[NativeType]>` in CallbackInfo parameters (#19032)
- perf(fmt): faster formatting for minified object literals (#19050)

### 1.33.2 / 2023.05.04

- fix(core): Use primordials for methods (#18839)
- fix(core): allow esm extensions not included in snapshot (#18980)
- fix(core): rebuild when JS sources for snapshotting change (#18976)
- fix(ext/io) several sync fs fixes (#18886)
- fix(ext/kv): KvU64#valueOf and KvU64 inspect (#18656)
- fix(ext/kv): stricter structured clone serializer (#18914)
- fix(ext/kv): throw on the Kv constructor (#18978)
- fix(ext/node): add missing `release` property to node's `process` (#18923)
- fix(ext/url): throw `TypeError` for empty argument (#18896)
- fix(ext/websocket): update fastwebsockets to 0.3.1 (#18916)
- fix(fmt/json): support formatting number with exponent and no sign (#18894)
- fix(node/http): Request.setTimeout(0) should clear (#18949)
- fix(npm): canonicalize filename before returning (#18948)
- fix(npm): canonicalize search directory when looking for package.json (#18981)
- fix(test): disable preventDefault() for beforeunload event (#18911)
- perf(core): async op pseudo-codegen and performance work (#18887)
- perf(core): use jemalloc for V8 array buffer allocator (#18875)
- perf(ext/web): fast path for ws events (#18905)
- perf(ext/websocket): use internal dispatch for msg events (#18904)
- perf: lazily create RootCertStore (#18938)
- perf: lazily retrieve ppid (#18940)
- perf: use jemalloc as global allocator (#18957)

### 1.33.1 / 2023.04.28

- fix(ext/fetch): subview Uint8Array in Req/Resp (#18890)
- fix(ext/websocket): client connect URI (#18892)
- fix(ext/websocket): restore op_ws_send_ping (#18891)
- fix(repl): don't panic on undefined exception (#18888)

### 1.33.0 / 2023.04.27

- BREAKING(unstable): remove "Deno.serve(handler, options)" overload (#18759)
- Revert "chore(ext/websocket): Add autobahn|testsuite fuzzingclient (#…
  (#18856)
- feat(bench): add `--no-run` flag (#18433)
- feat(cli): don't check permissions for statically analyzable dynamic imports
  (#18713)
- feat(cli): flatten deno.json configuration (#17799)
- feat(ext/ffi): support marking symbols as optional (#18529)
- feat(ext/http): Rework Deno.serve using hyper 1.0-rc3 (#18619)
- feat(ext/kv): add more atomic operation helpers (#18854)
- feat(ext/kv): return ok bool from atomic commit (#18873)
- feat(ext/url): `URL.canParse` (#18286)
- feat(lint): add `Deno.run` to `no-deprecated-deno-api` (#18869)
- feat(node/crypto): Elliptic Curve Diffie-Hellman (ECDH) support (#18832)
- feat(node/http): implement ClientRequest.setTimeout() (#18783)
- feat(task): introduce built-in `unset` command to `deno task` (#18606)
- feat: Deprecate Deno.run API in favor of Deno.Command (#17630) (#18866)
- fix(compile): write bytes directly to output file (#18777)
- fix(core): Wrap safe collections' argument of primordials (#18750)
- fix(coverage): exclude test files (#18748)
- fix(dts): `URLPatternComponentResult` groups should have possibly undefined
  key values (#18643)
- fix(ext/node): add crypto.sign|verify methods (#18765)
- fix(ext/node): fix hash.flush (#18818)
- fix(ext/node): implement asymmetric keygen (#18651)
- fix(ext/node): improve vm.runInThisContext (#18767)
- fix(ext/node): prime generation (#18861)
- fix(lsp): show dependency errors for repeated imports (#18807)
- fix(npm): only include top level packages in top level node_modules directory
  (#18824)
- fix(test): allow explicit undefined for boolean test options (#18786)
- fix(test): handle dispatched exceptions from test functions (#18853)
- perf(ext/http): avoid spread arg deopt in op_http_wait (#18850)
- perf(ext/http): optimize away code based on callback length (#18849)
- perf(ext/http): optimize for zero or one-packet response streams (#18834)
- perf(ext/http): use smi for slab IDs (#18848)
- perf(ext/websocket): various performance improvements (#18862)

### 1.32.5 / 2023.04.18

- feat(UNSTABLE/kv): AtomicOperation#sum (#18704)
- fix(core): Use safe primordials wrappers (#18687)
- fix(ext/node): add req.socket.remoteAddress (#18733)
- fix(ext/node): implement crypto.createVerify (#18703)
- fix(ext/node): polyfill response._implicitHeader method (#18738)
- fix(ext/websocket): Avoid write deadlock that requires read_frame to complete
  (#18705)
- fix(lsp): ensure language server status works on unix (#18727)
- fix(npm): eagerly reload package information when version from lockfile not
  found locally (#18673)
- fix(path): Remove non node symbols (#18630)
- fix(test): add process sigint handler for --watch (#18678)
- perf(ext/websocket): make `op_server_ws_next_event` deferred (#18632)
- perf(ops): directly respond for eager ops (#18683)

### 1.32.4 / 2023.04.12

- Revert "fix(cli): don't store blob and data urls in the module cache (#18261)"
  (#18572)
- feat(core): sync io ops in core (#18603)
- feat(ext/http): add an op to perform raw HTTP upgrade (#18511)
- fix(core): preserve syntax error locations in dynamic imports (#18664)
- fix(ext/cache): cache.put overwrites previous call (#18649)
- fix(ext/kv): keys must be arrays (#18655)
- fix(ext/node): add X509Certificate (#18625)
- fix(ext/node): add symmetric keygen (#18609)
- fix(ext/node): fix unable to resolve fraction.js (#18544)
- fix(ext/node): implement hkdf-expand (#18612)
- fix(ext/node): json encode binary command name (#18596)
- fix(npm): cache bust npm specifiers more aggressively (#18636)
- fix(npm): do not "npm install" when npm specifier happens to match
  package.json entry (#18660)
- fix(npm): reload an npm package's dependency's information when version not
  found (#18622)
- perf(ext/io): remove a data copy from File write (#18601)
- perf(ext/websocket): replace tokio_tungstenite server with fastwebsockets
  (#18587)

### 1.32.3 / 2023.04.01

- fix(check): ensure diagnostics caused by changes in other files get
  invalidated between runs (#18541)
- fix(ext/ffi): crash when same reference struct is used in two fields (#18531)
- fix(lsp): add a document preload file system entry limit (#18553)
- fix(repl): disable language server document preloading in the repl (#18543)
- fix(test): don't swallow sanitizer errors with permissions (#18550)
- perf(check): faster source hashing (#18534)

### 1.32.2 / 2023.03.31

- Revert "refactor(ext/node): Use Deno.inspect (#17960)" (#18491)
- feat(core): initialize SQLite off-main-thread (#18401)
- feat(ext/kv): return versionstamp from set/commit (#18512)
- feat(ext/node): add `crypto.checkPrime` API (#18465)
- feat(ext/node): implement crypto.createSecretKey (#18413)
- feat(test): print pending tests on sigint (#18246)
- feat: port node:zlib to rust (#18291)
- fix(cli): add colors to "Module not found" error frame (#18437)
- fix(cli): don't store blob and data urls in the module cache (#18261)
- fix(cli/bench): look for clone3 syscalls for thread count (#18456)
- fix(core): located_script_name macro was using format syntax (#18388)
- fix(core): panic at build time if extension code contains anything other than
  7-bit ASCII (#18372)
- fix(core): restore cache journal mode to TRUNCATE and tweak tokio test in
  CacheDB (#18469)
- fix(coverage): ignore files from npm registry (#18457)
- fix(dts): improve types for the Deno.KV API (#18510)
- fix(ext/kv): add missing `getMany` method (#18410)
- fix(ext/node): add aes-128-ecb algorithm support (#18412)
- fix(ext/node): add missing _preloadModules hook (#18447)
- fix(ext/node): implement crypto.Sign (RSA/PEM/SHA{224,256,384,512}) (#18471)
- fix(ext/node): make cipher/decipher transform stream (#18408)
- fix(lsp): `textDocument/references` should respect `includeDeclaration`
  (#18496)
- fix(lsp): better handling of `data:` urls (#18527)
- fix(lsp): include all diagnosable documents on initialize (#17979)
- fix(ops): fallback when FastApiOneByteString is not utf8 (#18518)
- fix(repl): improve package.json support (#18497)
- fix(streams): add support `Float64Array` to `ReadableStreamByobReader`
  (#18188)
- fix: Add missing `processenv` winapi feature to deno_io (#18485)
- fix: upgrade to TypeScript 5.0.3 (#18532)
- perf(ext/websocket): efficient event kind serialization (#18509)
- perf(ext/websocket): special op for sending binary data frames (#18506)
- perf(ext/websocket): special op for sending text data frames (#18507)
- perf(ext/websocket): use opAsync2 to avoid spread deopt (#18525)
- perf: `const` op declaration (#18288)

### 1.32.1 / 2023.03.23

- fix(core): disable resizable ArrayBuffer and growable SharedArrayBuffer
  (#18395)
- fix(cli): restore `deno run -` to handle stdin as typescript (#18391)
- fix(inspect): ensure non-compact output when object literal has newline in
  entry text (#18366)
- fix(lsp): ensure `enablePaths` works when clients do not provide a trailing
  slash for workspace dir (#18373)

### 1.32.0 / 2023.03.22

- BREAKING(unstable): remove WebGPU (#18094)
- feat(ext/fs): FileInfo.dev is supported on Windows (#18237)
- feat(cli): --ext parameter for run, compile, and bundle (#17172)
- feat(compile): Add support for web workers in standalone mode (#17657)
- feat(compile): Enable multiple roots for a standalone module graph (#17663)
- feat(core): deno_core::extension! macro to simplify extension registration
  (#18210)
- feat(ext/kv): key-value store (#18232)
- feat(ext/net): Add multicasting APIs to DatagramConn (#10706) (#17811)
- feat(ext/url): URLSearchParams.size (#17884)
- feat(repl): add `DENO_REPL_HISTORY` to change history file path (#18047)
- feat(serde_v8): support BigInt serialization (#18225)
- feat: TypeScript 5.0.2 (except decorators) (#18294)
- fix(cli): preserve blob store when resetting file watcher (#18253)
- fix(cli/integration): clippy lints (#18248)
- fix(ext/kv): don't request permissions for ":memory:" (#18346)
- fix(ext/kv): reverse mapping between `AnyValue::Bool` and `KeyPart::Bool`
  (#18365)
- fix(ext/node): add createDecipheriv (#18245)
- fix(ext/node): resource leak in createHmac (#18229)
- fix(ext/node): use Deno.Command from `ext:runtime` (#18289)
- fix(repl): Hide indexable properties in tab completion (#18141)
- fix(runtime): Extract error code for all OS error variants (#17958)
- fix: include error in message about not being able to create the TypeScript
  cache (#18356)
- perf(check): type check local files only when not using `--all` (#18329)
- perf(core) Reduce copying and cloning in extension initialization (#18252)
- perf(core) Reduce script name and script code copies (#18298)
- perf(core): preserve ops between snapshots (#18080)
- perf(core): use static specifier in ExtensionFileSource (#18271)
- perf: disable WAL for transpiled source cache (#18084)
- perf: disable runtime snapshot compression (#18239)

### 1.31.3 / 2023.03.16

- fix(check): regression where config "types" entries caused type checking
  errors (#18124)
- fix(core): Upgrades bytes crate from =1.2.1 to ^1.4.0 (#18123)
- fix(core): `SafePromiseAll` to be unaffected by `Array#@@iterator` (#17542)
- fix(core/internal): fix typo in primordial type definitions (#18125)
- fix(ext/fs): retry if file already exists in makeTempFile (#17787)
- fix(ext/http): abort request signal when response errors (#17822)
- fix(ext/node): add crypto.createCipheriv (#18091)
- fix(ext/node): implement "ascii" encoding for node:fs writeFile() (#18097)
- fix(ext/web): Stop using `globalThis.ReadableStream` in `Blob` (#18187)
- fix(info/doc): add missing `--no-lock` and `--lock` flags (#18166)
- fix(lsp): avoid calling client while holding lock (#18197)
- fix(npm): "not implemented scheme" message should properly show the scheme
  (#18209)
- fix(npm): show a progress bar when initializing the node_modules folder
  (#18136)
- fix(repl): do not panic deleting `Deno` or deleting all its properties
  (#18211)
- fix: ensure no node_modules directory is created when a package.json exists
  and no npm dependencies are used (#18134)
- perf: do not depend on iana-time-zone (#18088)

### 1.31.2 / 2023.03.10

- Revert "perf: disable snapshot compression (#18061)" (#18074)
- deps: bump `regexp` to `^1.7.0` (#17966)
- deps: bump once_cell to ^1.17.1 (#18075)
- feat(core): prevent isolate drop for CLI main worker (#18059)
- feat(ext/ffi): Make External pointers keep reference to V8 buffer (#17955)
- feat(ops): reland fast zero copy string arguments (#17996)
- feat(ops): relational ops (#18023)
- fix(check): include dts files in tsc roots (#18026)
- fix(cli): add space after period in `--v8-flags` (#18063)
- fix(cli,ext/web): Upgrading uuid from =1.1.2 to 1.3.0 (#17963)
- fix(core): introduce `SafeRegExp` to primordials (#17592)
- fix(ext/crypto): correctly limit ECDSA and hash algorithms (#18030)
- fix(ext/ffi): Remove deno_core::OpState qualifiers, fix ops returning pointer
  defaults (#17959)
- fix(ext/node): remove unused _hex module (#18045)
- fix(ext/node): util.types.isSharedArrayBuffer (#17836)
- fix(ext/webstorage): check size of inputs before insert (#18087)
- fix(lockfile): don't touch lockfile is npm specifiers are identical (#17973)
- fix(npm): improve peer dependency resolution with circular dependencies
  (#18069)
- fix(prompt): better output with control chars (#18108)
- fix(runtime): Add `Deno.` prefix for registered symbols (#18086)
- fix(runtime/windows): ensure `Deno.stdin.setRaw(false)` properly disables raw
  mode (#17983)
- fix: Split extension registration and snapshotting (#18098)
- fix: attempt to only allow one deno process to update the node_modules folder
  at a time (#18058)
- fix: lazily surface errors in package.json deps parsing (#17974)
- perf(core): over-allocate in ModuleMap when running from snapshot (#18083)
- perf(ext/node): improve createHash performance (#18033)
- perf: disable snapshot compression (#18061)
- perf: don't add unload event listener (#18082)
- perf: move runtime bootstrap code to snapshot time (#18062)
- perf: move setting up Deno namespace to snapshot time (#18067)
- wpt: unlock nightly with --no-ignore (#17998)

### 1.31.1 / 2023.02.25

- feat: add `DENO_NO_PACKAGE_JSON` env var (#17926)
- fix(npm): lazily install package.json dependencies only when necessary
  (#17931)
- fix(npm): package.json auto-discovery should respect `--no-config` and
  `--no-npm` (#17924)
- fix: ensure concurrent non-statically analyzable dynamic imports do not
  sometimes fail (#17923)
- fix: ignore workspace, git, file, http, https specifiers in package.json
  (#17934, #17938)
- fix: regression remapping remote specifier to local file (#17935)
- fix: remote modules should be allowed to import data urls (#17920)

### 1.31.0 / 2023.02.23

- feat(bench): Add JSON reporter for "deno bench" subcommand (#17595)
- feat(bench): change --json output format (#17888)
- feat(core): allow to specify entry point for snapshotted ES modules (#17771)
- feat(ext/ffi): Replace pointer integers with v8::External objects (#16889)
- feat(ext/http): add 2nd param to handler to get remote address (#17633)
- feat(ext/node): embed std/node into the snapshot (#17724)
- feat(ext/node): implement `node:v8` (#17806)
- feat(install): follow redirects for urls with no path (#17449)
- feat(node): stabilize Node-API (#17553)
- feat(npm): support bare specifiers from package.json in more subcommands and
  language server (#17891)
- feat(npm): support npm specifiers in remote modules without `--unstable`
  (#17889)
- feat(permissions): grant all permission for a group in permission prompt
  (#17140)
- feat(task): add warning about package.json scripts support (#17900)
- feat(task): adjust warning (#17904)
- feat(task): support scripts in package.json (#17887)
- feat: Deprecate 'deno bundle' subcommand (#17695)
- feat: Stabilize Deno.Command API (#17628)
- feat: add more variants to Deno.build.os (#17340)
- feat: add signal option to Deno.resolveDns (#17384)
- feat: auto-discover package.json for npm dependencies (#17272)
- feat: stabilize Deno.osUptime() (#17554)
- feat: start caching npm package version's "bin" entry from npm registry
  (#17881)
- feat: support bare specifier resolution with package.json (#17864)
- feat: wire up ext/node to the Node compatibility layer (#17785)
- fix(cli): Add better error message when powershell is missing during upgrade
  (#17759)
- fix(cli/graph_util): don't append referrer info for root module errors
  (#17730)
- fix(cli/napi): correct name handling in napi property descriptor (#17716)
- fix(cli/napi): handle all property variants in napi_define_properties (#17680)
- fix(core): don't allow to import internal code is snapshot is loaded (#17694)
- fix(core): rebuild when JS sources for snapshotting change (#17876)
- fix(core): remove async op inlining optimization (#17899)
- fix(dts): make Deno.Command accept readonly prop in options.args (#17718)
- fix(ext/console): Only right-align integers in console.table() (#17389)
- fix(ext/ffi): Fix re-ref'ing UnsafeCallback (#17704)
- fix(ext/ffi): improve error messages in FFI module (#17786)
- fix(ext/flash): Always send correct number of bytes when handling HEAD
  requests (#17740)
- fix(ext/flash): wrong order of arguments passed to `http1Response` (#17893)
- fix(ext/node): add support for BYOB streams (#17803)
- fix(ext/node): fix node stream (#17874)
- fix(ext/node): fix npm module resolution when --node-modules-dir specified
  (#17896)
- fix(ext/node): fix process.uptime (#17839)
- fix(ext/node): fix webcrypto export (#17838)
- fix(ext/websocket): extra ws pongs sent (#17762)
- fix(fmt): make fmt options CLI args less verbose (#17550)
- fix(lint): revert no-deprecated-api for Deno.run (#17880)
- fix(npm): allow resolving from package.json when an import map exists (#17905)
- fix(npm): filter out duplicate packages names in resolution (#17857)
- fix(npm): improve peer dependency resolution (#17835)
- fix(npm): resolve node_modules dir relative to package.json instead of cwd
  (#17885)
- fix(npm): support bare specifiers in package.json having a path (#17903)
- fix(ops): Always close cancel handles for read_async/write_async (#17736)
- fix(webgpu): don't default to 0 for setVertexBuffer.size & properly use
  webidl.setlike (#17800)
- fix(runtime): Refactor fs error mapping to use unified format (#17719)
- fix(webgpu): use correct op for GPUDevice.createSampler (#17729)
- fix: add WouldBlock error (#17339)
- fix: loading built-in Node modules embedded in the binary (#17777)
- fix: use static Reflect methods in nodeGlobalThis proxy (#17696)
- perf(core): speed up promise hook dispatch (#17616)
- perf(core, runtime): Further improve startup time (#17860)
- perf(ext/ffi): Revert UTF-8 validity check from getCString (#17741)
- perf(ext/node): move winerror binding to rust (#17792)
- perf(http): remove allocations checking upgrade and connection header values
  (#17727)
- perf: disable fetching graph cache info except for `deno info` (#17698)
- perf: module info cache - avoid MediaType.to_string() allocation (#17699)
- perf: remove `current_dir()` call in `Deno.mainModule` (#17883)
- perf: use ops for node:crypto ciphers (#17819)

### 1.30.3 / 2023.02.07

- fix(ext/console): log class for class constructor (#17615)
- fix(lsp): prevent crash analyzing module (#17642)

### 1.30.2 / 2023.02.03

- Revert "chore(core): remove have_unpolled_ops on rt state (#17601)" (#17631)
- fix(webgpu): specify viewFormats in surface configuration (#17626)

### 1.30.1 / 2023.02.02

- Revert "fix(watch): preserve `ProcState::file_fetcher` between restarts
  (#15466) (#17591)
- fix(core): Add lint check for core (#17223)
- fix(ext): internal `structuredClone` for `ArrayBuffer` and `TypedArray`
  subclasses (#17431)
- fix(fmt): semiColons: false - handle prop with following generator and do
  while with no block body (#17567)
- fix(install): tsconfig.json -> deno.json for config file suffix (#17573)
- fix(lockfile): emit trailing newline (#17618)
- fix(lsp): update document dependencies on configuration change (#17556)
- fix(napi): guard threadsafe function counters behind a mutex (#17552)
- fix(napi): remove wrong length check in napi_create_function (#17614)
- fix(napi): return node globalThis from napi_get_global (#17613)
- fix(repl): handle @types/node not being cached in the repl (#17617)
- fix(upgrade): ensure temp dir cleanup on failure (#17535)
- fix: ensure "fs" -> "node:fs" error/quick fix works when user has import map
  (#17566)
- perf(ops): Remove unnecessary fast call fallback options usage (#17585)

### 1.30.0 / 2023.01.25

- feat(cli): add `DENO_V8_FLAGS` env var (#17313)
- feat(fmt): add ability to configure semicolons (#17292)
- feat(fmt): make semi-colon option a boolean (#17527)
- feat(runtime): add bigint to seek typings (#17314)
- feat(runtime/command): make stdin default to inherit for spawn() (#17334)
- feat(runtime/os): add `Deno.env.has()` (#17315)
- feat(upgrade): link to release notes & blog post (#17073)
- feat: Add sync APIs for "Deno.permissions" (#17019)
- feat: ES module snapshotting (#17460)
- feat: Stabilize Deno.Listener.ref/unref (#17477)
- feat: allow first arg in test step to be a function (#17096)
- feat: allow passing a ReadableStream to Deno.writeFile/Deno.writeTextFile
  (#17329)
- feat: embed import map in the config file (#17478)
- feat: log detection of config file (#17338)
- feat: suggest adding a "node:" prefix for bare specifiers that look like
  built-in Node modules (#17519)
- feat: support node built-in module imports (#17264)
- fix(ext/ffi): disallow empty ffi structs (#17487)
- fix(napi) use c_char instead of hardcoding i8 to avoid incompatibility with
  aarch64 (#17458)
- fix(napi): correctly handle name in napi_create_function (#17489)
- fix(napi): don't hold on to borrow during iteration (#17461)
- fix(napi): handle return value from initializer (#17502)
- fix(napi): improve napi_adjust_external_memory (#17501)
- fix(napi): improve napi_detach_arraybuffer (#17499)
- fix(napi): improve napi_is_detached_arraybuffer (#17498)
- fix(upgrade): don't display release information for canary (#17516)
- fix: remove leftover Deno.spawn references (#17524)
- fix: support import map specified as data uri (#17531)
- fix: update expected output for config auto-discovery debug log (#17514)

### 1.29.4 / 2023.01.16

- feat(core): Reland support for async ops in realms (#17204)
- fix(cli/fmt): show filepath for InvalidData error (#17361)
- fix(core): Add `Generator` and `AsyncGenerator` to primordials (#17241)
- fix(ext/fetch) Fix request clone error in flash server (#16174)
- fix(ext/fetch): remove Response.trailer from types (#17284)
- fix(ext/ffi): use SafeMap in getTypeSizeAndAlignment (#17305)
- fix(ext/flash): Correctly handle errors for chunked responses (#17303)
- fix(ext/flash): Fix panic when JS caller doesn't consume request body (#16173)
- fix(ext/flash): Fix typo in 'chunked' flash ops (#17302)
- fix(napi): allow cleanup hook to remove itself (#17402)
- fix(napi): correct arguments for napi_get_typedarray_info (#17306)
- fix(napi): functions related to errors (#17370)
- fix(napi): update node version to lts (#17399)
- fix(npm): handle an npm package that has itself as a dependency (#17425)
- fix(npm): use original node regex in npm resolution (#17404)
- fix(ops): disallow memory slices as inputs to async ops (#16738)
- fix(repl): improve validator to mark more code as incomplete (#17443)
- fix(runtime/fs): preserve permissions in copyFileSync for macOS (#17412)
- fix(runtime/os): use GetPerformanceInfo for swap info on Windows (#17433)

### 1.29.3 / 2023.01.13

- feat(core): allow specifying name and dependencies of an Extension (#17301)
- feat(ext/ffi): structs by value (#15060)
- fix(cli): uninstall command accept short flags (#17259)
- fix(cli/args): update value_name of inspect args to resolve broken completions
  (#17287)
- fix(core): get v8 console from context extra bindings (#17243)
- fix(ext/web/streams): fix ReadableStream asyncIterator (#16276)
- fix(fmt): better handling of link reference definitions when formatting
  markdown (#17352)
- fix(install): should always include `--no-config` in shim unless `--config` is
  specified (#17300)
- fix(napi): Implement `napi_threadsafe_function` ref and unref (#17304)
- fix(napi): date and unwrap handling (#17369)
- fix(napi): handle static properties in classes (#17320)
- fix(napi): support for env cleanup hooks (#17324)
- fix(npm): allow to read package.json if permissions are granted (#17209)
- fix(npm): handle declaration file resolution where packages incorrectly define
  "types" last in "exports" (#17290)
- fix(npm): panic resolving some dependencies with dist tags (#17278)
- fix(npm): reduce copy packages when resolving optional peer dependencies
  (#17280)
- fix(npm): support old packages and registries with no integrity, but with a
  sha1sum (#17289)
- fix(permissions): lock stdio streams when prompt is shown (#17392)
- fix(watch): preserve `ProcState::file_fetcher` between restarts (#15466)
- fix(webidl): properly implement setlike (#17363)
- fix: check if BroadcastChannel is open before sending (#17366)
- fix: don't panic on resolveDns if unsupported record type is specified
  (#17336)
- fix: don't unwrap in test pipe handling logic (#17341)
- fix: make self and window getters only & make getterOnly ignore setting
  (#17362)
- perf(ext,runtime): remove using `SafeArrayIterator` from `for-of` (#17255)

### 1.29.2 / 2023.01.05

- feat(unstable): Add "Deno.osUptime()" API (#17179)
- feat(unstable): Add Deno.Conn.ref()/unref() (#17170)
- fix(cli): allow for specifying `noErrorTruncation` compiler option (#17127)
- fix(cli): bundle command support shebang file (#17113)
- fix(cli): do not clear screen for non-TTY environments in watch mode (#17129)
- fix(core): Do not print errors prop for non-AggregateError errors (#17123)
- fix(core): Have custom errors be created in the right realm (#17050)
- fix(core): run macrotasks and next ticks after polling dynamic imports
  (#17173)
- fix(declaration): change `Deno.open` example to not use `Deno.close(rid)`
  (#17218)
- fix(ext): Add checks for owning properties in for-in loops (#17139)
- fix(ext/fetch): Guard against invalid URL before its used by reqwest (#17164)
- fix(ext/fetch): handle errors in req body stream (#17081)
- fix(ext/http): close stream on resp body error (#17126)
- fix(ext/net): Remove unstable check from op_node_unstable_net_listen_udp
  (#17207)
- fix(init): update comment style (#17074)
- fix(install): use a hidden file for the lockfile and config (#17084)
- fix(lint): column number for pretty reporting was off by 1 (#17107)
- fix(lsp): handle template literal as first arg in test function (#17076)
- fix(lsp): treat empty string config value as None (#17227)
- fix(lsp): "Add all missing imports" uses correct specifiers (#17216)
- fix(lsp): completions for private variables (#17220)
- fix(lsp): don't error if completionItem/resolve request fails (#17250)
- fix(lsp): less aggressive completion triggers (#17225)
- fix(lsp/format): language formatter used should be based on language id
  (#17148)
- fix(lsp/testing): fallback name for non-analyzable tests in collector (#17120)
- fix(lsp/testing): support not needing to declare first arg function in test
  declaration (#17097)
- fix(node): Add op_node_unstable_os_uptime to allow for node interop (#17208)
- fix(npm): conditional exports with --node-modules-dir (#17111)
- fix(npm): fix require resolution if using --node-modules-dir (#17087)
- fix(npm): improve exports resolution when type checking (#17071)
- fix(npm): resolve npm specifiers when root redirected (#17144)
- fix(permissions): add information about import() API request (#17149)
- fix(permissions): fix italic font in permission prompt (#17249)
- fix(permissions): process `URL` in `Deno.FfiPermissionDescriptor.path` for
  `revoke()` and `request()` (#17094)
- fix(regression): ensure progress information is shown when downloading remote
  modules (#17069)
- fix(repl): doing two history searches exiting with ctrl+c should not exit repl
  (#17079)
- fix(repl): errors shouldn't terminate repl (#17082)
- fix(runtime): `Deno.memoryUsage().rss` should return correct value (#17088)
- fix(runtime): expose `extensions_with_js` from WorkerOptions (#17109)
- fix: add missing verb in description (#17163)
- fix: display URL in invalid URL error (#17128)
- fix: hide progress bars when showing permission prompt (#17130)
- fix: ignore local lockfile for deno install and uninstall (#17145)
- fix: rejected dynamic import should retain error context (#17160)
- fix: upgrade deno_ast to 0.23 (#17269)
- perf(lsp): concurrent reads and exclusive writes (#17135)

### 1.29.1 / 2022.12.15

- Revert "feat(ops): Fast zero copy string arguments (#16777)" (#17063)
- fix: re-add types for Response.json static method (#17061)

### 1.29.0 / 2022.12.14

- feat(cli): support configuring the lock file in the config file (#16781)
- feat(cli): support deno bench in the config file (#16608)
- feat(ext/ffi): better type hints for Deno.dlopen (#16874)
- feat(flags): add `deno check --all` as new preferred alias for `--remote`
  (#16702)
- feat(fmt): improve width calculation (#16982)
- feat(init): Generate deno.json by default (#16389)
- feat(init): Generate main_bench.ts by default (#16786)
- feat(init): Use jsonc for configuration file (#17002)
- feat(napi): improve napi coverage (#16198)
- feat(npm): add support for `NPM_CONFIG_REGISTRY` (#16980)
- feat(ops): Fast zero copy string arguments (#16777)
- feat(repl): run "deno repl" with no permissions (#16795)
- feat(repl): support npm packages (#16770)
- feat: Stabilize Deno.TcpConn.setNoDelay() and Deno.TcpConn.setKeepAlive()
  (#17003)
- feat: add `--inspect-wait` flag (#17001)
- feat: ignore `node_modules` and `.git` folders when collecting files
  everywhere (#16862)
- feat: improve download progress bar (#16984)
- feat: support `createNew` in `Deno.writeFile` (#17023)
- feat: upgrade to TypeScript 4.9.3 (#16973)
- fix(cli/upgrade): properly cleanup after finished (#16930)
- fix(compile): ensure import map is used when specified in deno config file
  (#16990)
- fix(ext/fetch): new Request should soft clone (#16869)
- fix(ext/websocket): Reland make try_send ops infallible (#16968)
- fix(fmt): panic in yaml header with multi-byte characters (#17042)
- fix(napi): respect --quiet flag in unimplemented warnings (#16935)
- fix(npm): ancestor that resolves peer dependency should not include self in id
  (#16693)
- fix(npm): dependency types were sometimes not being resolved when package had
  no types entry (#16958)
- fix(npm): support loose node semver ranges like `>= ^x.x.x` (#17037)
- fix(ops): disallow auto-borrowing OpState across potential await point
  (#16952)
- fix(permissions): Allow ancestor path for --allow-ffi (#16765)
- fix(task): improve word parsing (#16911)
- fix(task): support redirects in pipe sequences (#16903)
- fix(test): handle scenario where --trace-ops would cause an unhandled promise
  rejection (#16970)
- fix(test): improve how `--fail-fast` shuts down when hitting limit (#16956)
- fix(upgrade): respect the `--quiet` flag (#16888)
- fix(upgrade/windows): correct command in windows access denied message
  (#17049)
- fix(upgrade/windows): show informative message on access denied error (#16887)
- fix(vendor): properly handle bare specifiers that start with http (#16885)
- fix(windows): support special key presses in raw mode (#16904)
- fix: always derive http client from cli flags (#17029)
- fix: default to `"inherit"` for `Deno.Command#spawn()`'s `stdout` & `stderr`
  (#17025)
- fix: respect the `--quiet` flag in more cases (#16998)
- npm: ensure runtime exceptions are surfaced when debugger is attached (#16943)
- perf(ext/websocket): skip Events constructor checks (#16365)
- perf: use fast api for io read/write sync (#15863)
- unstable: remove Deno.spawn, Deno.spawnSync, Deno.spawnChild APIs (#16893)

### 1.28.3 / 2022.12.01

- Revert "fix(ext/flash): graceful server startup/shutdown with unsettl…
  (#16839)
- feat(core): send "executionContextDestroyed" notification on program end
  (#16831)
- feat(core): show unresolved promise origin (#16650)
- feat(core): support initializing extensions with and without JS (#16789)
- feat(ops): fast calls for Wasm (#16776)
- feat(ops): support raw pointer arguments (#16826)
- feat(unstable): rework Deno.Command (#16812)
- fix(cli/js): improve resource sanitizer messages (#16798)
- fix(coverage): Error if the emit cache is invalid (#16850)
- fix(ext/ffi): Null buffer pointer value is inconsistent (#16625)
- fix(ext/node): allow absolute path in createRequire (#16853)
- fix(ext/web): fix typings for readable stream readers (#16191)
- fix(fmt/markdown): fix emoji width calculation in tables (#16870)
- fix(inspector): send "isDefault" in aux data (#16836)
- fix(lsp): analyze fs dependencies of dependencies to find npm package
  requirements (#16866)
- fix(npm): allow to inspect npm modules with --inspect-brk (#16841)
- fix(npm): better error message when attempting to use typescript in npm
  packages (#16813)
- fix(npm): don't resolve JS files when resolving types (#16854)
- fix(npm): ensure npm package downloaded once per run when using `--reload`
  (#16842)
- fix(npm): improve package.json exports support for types (#16880)
- fix(ops): circular dependency in deno_ops test (#16809)
- fix(repl): more reliable history handling (#16797)
- fix(repl): respect --quiet flag (#16875)
- fix(runtime): feature-flag snapshot from snapshot (#16843)
- fix(task): output encoding issues on windows (#16794)
- perf(ops): Reenable fast unit result optimization (#16827)

### 1.28.2 / 2022.11.24

- feat(cli): add warning for incorrectly ordered flags (#16734)
- feat(core): Ability to create snapshots from existing snapshots (#16597)
- fix(ext/flash): graceful server startup/shutdown with unsettled promises in
  mind (#16616)
- fix(ext/node): handle URL in createRequire (#16682)
- fix(ext/websocket): uncatchable errors on send (#16743)
- fix(fmt/markdown): scenario where whitespace was being incorrectly stripped in
  inline links (#16769)
- fix(info): handle circular npm dependencies (#16692)
- fix(inspector): ensure console methods provided by inspector are available
  (#16724)
- fix(install): `deno install -f` should overwrite lockfile from previous
  installation (#16744)
- fix(npm): add suggestions to error message when can't find binary entrypoint
  (#16733)
- fix(npm): automatically find binary entrypoint when values are all the same
  (#16735)
- fix(npm): handle directory resolution when resolving declaration files
  (#16706)
- fix(npm): use an http client with connection pool (#16705)
- fix(npm/check): prioritize exports over types entry (#16788)
- fix(npm/types): resolve main entrypoint declaration file when no types entry
  (#16791)
- fix(types/unstable): change interface base for `CommandOutput` (#16696)
- fix: Make npm packages works with import maps (#16754)
- perf(ext/flash): optimize response streaming (#16660)
- perf(npm): make dependency resolution faster (#16694)

### 1.28.1 / 2022.11.16

- fix(bundle): explicit error when using an npm specifier with deno bundle
  (#16637)
- fix(cli): add a jsdoc tag for `UnstableRunOptions` (#16525)
- fix(ext/webstorage): make web storages re-assignable (#16661)
- fix(install): support npm specifiers (#16634)
- fix(lock): ensure npm dependencies are written with --lock-write (#16668)
- fix(npm): don't fail if conditional exports don't contains types (#16651)
- fix(npm): handle peer dep being resolved without resolved dep higher in tree
  and then with (#16640)
- fix(npm): probing for files that have a file stem (#16641)
- fix(npm): properly handle getting `@types` package for scoped packages
  (#16655)
- fix(npm): support dist tags specified in npm package dependencies (#16652)
- fix(npm): support non-all lowercase package names (#16669)
- fix(npm): using types for packages with subpath (#16656)
- perf(runtime/spawn): collect output using `op_read_all` (#16596)

### 1.28.0 / 2022.11.13

- feat(lock): don't require --unstable for auto discovery (#16582)
- feat(npm): require --unstable for npm specifiers in remote modules (#16612)
- feat(ops): implement fast lazy async ops (#16579)
- feat(runtime): support creating workers with custom v8 snapshots (#16553)
- feat(unstable): "Deno.Command()" API (#16516)
- feat(unstable/npm): module graph derived npm specifier resolution order
  (#16602)
- feat: don't require --unstable flag for npm programs (#16520)
- feat: remove --unstable flag requirement for npm: specifiers (#16473)
- feat: stabilize Deno.bench() and 'deno bench' subcommand (#16485)
- feat: stabilize Deno.networkInterfaces() (#16451)
- feat: stabilize Deno.systemMemoryInfo() (#16445)
- feat: stabilize Deno.uid() and Deno.gid() (#16424)
- fix(ext/flash): revert #16284 and add test case (#16576)
- fix(ext/response): make error, json, redirect enumerable (#16497)
- fix(npm): disable npm specifiers in import.meta.resolve() (#16599)
- fix: update latest release version after github release publish (#16603)
- perf(core): minimize trivial heap allocations in `resolve_async_ops` (#16584)
- perf(web): optimize single pass utf8 decoding (#16593)
- perf: more efficient `deno cache` and npm package info usage (#16592)

### 1.27.2 / 2022.11.08

- feat(unstable/npm): support peer dependencies (#16561)
- fix(ext/http): flush chunk when streaming resource (#16536)
- fix(lock): only store integrities for http: and https: imports (#16558)
- fix(npm): fix CJS resolution with local node_modules dir (#16547)
- fix(upgrade): don't prompt if current version has changed (#16542)

### 1.27.1 / 2022.11.03

- feat(core): support creating snapshots from existing snapshots (#14744)
- feat(unstable): support npm specifiers in `deno info` for display text output
  only (#16470)
- feat(unstable/lock): autodiscovery of lockfile (#16498)
- feat(unstable/lock): require --unstable flag to auto discover lockfile
  (#16524)
- feat(unstable/npm): `deno info --json` support for npm specifiers (#16472)
- fix: change default locale value (#16463)
- fix: finish stabilizing Deno.osRelease() (#16447)
- fix: update env to sys permission in jsdoc for Deno.osRelease (#16483)
- fix(cli/dts): add typings for Change Array by copy proposal (#16499)
- fix(core): fix APIs not to be affected by `Promise.prototype.then`
  modification (#16326)
- fix(ext/crypto): fix HMAC jwk import "use" check (#16465)
- fix(ext/websocket): make try_send ops infallible (#16454)
- fix(lock): add --no-lock flag to disable auto discovery of lock file (#16526)
- fix(lock): Additive lock file (#16500)
- fix(lock): error if a referenced package id doesn't exist in list of packages
  (#16509)
- fix(lsp): add ServerCapabilities::encoding (#16444)
- fix(lsp): correct `parameterNames.suppressWhenArgumentMatchesName` and
  `variableTypes.suppressWhenTypeMatchesName` (#16469)
- fix(napi): fix is_detached_arraybuffer (#16478)
- fix(npm): add `console` global for node environment (#16519)
- fix(runtime): fix Deno.hostname on windows (#16530)
- fix(test): add slice method to filename to make them portable (#16482)
- fix(tools): show correct upgrade command for upgrading canary (#16486)
- fix(upgrade): don't prompt if latest version is older than current binary
  (#16464)

### 1.27.0 / 2022.10.27

- feat(core): enable --harmony-change-array-by-copy V8 flag (#16429)
- feat(cli): check for updates in background (#15974)
- feat(cli): show error cause recursion information (#16384)
- feat(ext/ffi): Make op_ffi_ptr_of fast (#16297)
- feat(ext/net): add reuseAddress option for UDP (#13849)
- feat(ext/net): reusePort for TCP on Linux (#16398)
- feat(ext/web): use ArrayBuffer.was_detached() (#16307)
- feat(lint): add a report lint config setting (#16045)
- feat(runtime): make kill signal optional (#16299)
- feat(task): remove warning about being unstable (#16281)
- feat(task): support `sleep` suffixes (#16425)
- feat(unstable/npm): initial type checking of npm specifiers (#16332)
- feat(unstable/task): fail task on async command failure (#16301)
- feat(update): prompt for new version once per day (#16375)
- feat(upgrade): check if user has write access to deno exe (#16378)
- feat: Add new lockfile format (#16349)
- feat: Stabilize Deno.consoleSize() API (#15933)
- feat: Stabilize Deno.osRelease() API (#15973)
- feat: Stabilize Deno.stdin.setRaw() (#16399)
- feat: introduce navigator.language (#12322)
- feat: stabilize Deno.futime() and Deno.futimeSync() (#16415)
- feat: stabilize Deno.loadavg() (#16412)
- feat: stabilize Deno.utime() and Deno.utimeSync() (#16421)
- feat: support inlay hints (#16287)
- fix(build) assume a custom compiler will support --export-dynamic-symbol-list
  linker flag. (#16387)
- fix(cli): Fixed bug where the progress bar did not clear (#16401)
- fix(cli): do not log update checker when log level is quiet (#16433)
- fix(compile): show an error when using npm specifiers (#16430)
- fix(core) Include causes when converting anyhow errors to JS exceptions
  (#16397)
- fix(ext/fetch): fix `size_hint` on response body resource (#16254)
- fix(ext/ffi): Use BufferSource for FFI buffer types (#16355)
- fix(ext/ffi): Use PointerValue in UnsafePointerView and UnsafeFnPointer types
  (#16354)
- fix(ext/net): don't remove sockets on unix listen (#16394)
- fix(ext/net): return an error from `startTls` and `serveHttp` if the original
  connection is captured elsewhere (#16242)
- fix(lsp): allow caching deps in non-saved files (#16353)
- fix(lsp): regression - error when removing file (#16388)
- fix(npm): add support for npm packages in lock files (#15938)
- fix(typescript): allow synthetic default imports when using
  `ModuleKind.ESNext` (#16438)
- fix(upgrade): Added error message when using canary option with M1 (#16382)
- fix(upgrade): put prompt date in the past when creating a file (#16380)
- fix: listenTlsWithReuseAddr test (#16420)
- fix: move generated napi symbols to cli/ (#16330)
- fix: upgrade swc_ecma_parser to 0.122.19 - deno_ast 0.20 (#16406)
- perf(core): avoid creating global handles in `op_queue_microtask` (#16359)
- perf(core): avoid isolate slots for ModuleMap (#16409)
- perf(core): do not drive JsInspector by default (#16410)
- perf(core): don't access isolate slots for JsRuntimeState (#16376)
- perf(ext/ffi): Fast UnsafePointerView read functions (#16351)
- perf(ext/flash): optimize path response streams (#16284)
- perf(ext/streams): fast path when consuming body of tee'd stream (#16329)
- perf(ext/web): add op_encode_binary_string (#16352)
- perf(ext/web): optimize transferArrayBuffer (#16294)
- perf(ext/web/encoding): avoid copy in decode (#16364)
- perf(ext/websocket): optimize `op_ws_next_event` (#16325)
- perf(ext/websocket): optimize socket.send (#16320)
- perf(serde_v8): `serde_v8::StringOrBuffer` return JS ArrayBuffer instead of
  Uint8Array (#16360)

### 1.26.2 / 2022.10.17

- feat(core): Reorder extension initialization (#16136)
- feat(core): add Deno.core.writeAll(rid, chunk) (#16228)
- feat(core): improve resource read & write traits (#16115)
- feat(unstable): add windowsRawArguments to SpawnOptions (#16319)
- feat(unstable/npm): support providing npm dist-tag in npm package specifier
  (#16293)
- feat(unstable/task): add `INIT_CWD` env var (#16110)
- fix sparse array inspection (#16204)
- fix(build) fix linux symbols export list format (#16313)
- fix(cli): allow importMap to be an absolute URL within the deno config file
  (#16234)
- fix(cli): skip removing the latter part if `@` appears at the beginning
  (#16244)
- fix(cli/bench): skip strace table border (#16310)
- fix(docs): Documentation improvements related to `JsRealm`. (#16247)
- fix(ext/cache): illegal constructor (#16205)
- fix(ext/crypto): correct HMAC get key length op (#16201)
- fix(ext/fetch): fix illegal header regex (#16236)
- fix(ext/fetch): reject immediately on aborted signal (#16190)
- fix(ext/fetch): set accept-encoding: identity if range header is present
  (#16197)
- fix(ext/fetch): support empty formdata (#16165)
- fix(ext/fetch): throw TypeError on non-Uint8Array chunk (#16262)
- fix(ext/fetch): throw TypeError on read failure (#16219)
- fix(ext/ffi): Fix UnsafeCallback ref'ing making Deno enter a live-loop
  (#16216)
- fix(ext/ffi): Fix usize and isize FFI callback parameters missing match arm
  (#16172)
- fix(ext/ffi): Invalid 'function' return type check logic, remove U32x2 as
  unnecessary (#16259)
- fix(ext/web/streams): enqueue to second branch before closing (#16269)
- fix(ext/web/streams): resolve cancelPromise in ReadableStreamTee (#16266)
- fix(ext/websocket): panic on no next ws message from an already closed stream
  (#16004)
- fix(lsp): properly handle snippets on completions (#16274)
- fix(lsp): treat empty import map value config as none (#16224)
- fix(napi): move napi symbols file (#16179)
- fix(npm): disable loading native module for "fsevents" package (#16273)
- fix(npm): support compiling on linux/aarch64 (#16208)
- fix(serde_v8): avoid creating unsound slice reference (#16189)
- fix: add error cause in recursive cause tail (#16306)
- perf(ext/cache): set journal_mode=wal (#16231)
- perf(ext/crypto): optimize `getRandomValues` (#16212)
- perf(ext/web): optimize `op_cancel_handle` (#16318)
- perf(ext/web): optimize timer cancellation (#16316)
- perf(napi): optimize primitive napi functions (#16163)
- perf(npm): parallelize caching of npm specifier package infos (#16323)

### 1.26.1 / 2022.10.06

- feat(npm): implement Node API (#13633)
- feat(unstable): add support for npm specifier cli arguments for 'deno cache'
  (#16141)
- fix(build): don't export all symbols to dynamic symbol table (#16171)
- fix(ext/cache): acquire reader lock before async op (#16126)
- fix(ext/cache): close resource on error (#16129)
- fix(ext/cache): prevent cache insert if body is not fully written (#16138)
- fix(ext/crypto): ECDH and X25519 non byte length and 0 length fixes (#16146)
- fix(ext/crypto): curve25519 import export (#16140)
- fix(ext/crypto): deriveBits for ECDH not taking length into account (#16128)
- fix(ext/crypto): ecdh spki key import/export roundtrip (#16152)
- fix(ext/crypto): fix importKey error when leading zeroes (#16009)
- fix(ext/crypto): interoperable import/export (#16153)
- fix(ext/crypto): use correct handle for public keys (#16099)
- fix(ext/fetch): `Body#bodyUsed` for static body (#16080)
- fix(ext/flash): Avoid sending Content-Length when status code is 204 (#15901)
- fix(node): add dns/promises and stream/consumers (#16169)
- fix(npm): better error is version is specified after subpath (#16131)
- fix(npm): handle json files in require (#16125)
- fix(npm): panic on invalid package name (#16123)
- fix(runtime): no FastStream for unrefable streams (#16095)
- fix(serde_v8): Implement MapAccess for StructAccess (#15962)
- fix(serde_v8): serialize objects with numeric keys correctly (#15946)
- fix: move Deno.hostname() from denoNsUnstable to denoNs (#16086)
- lsp: use deno:/asset instead of deno:asset (#16023)
- perf(ext/fetch): consume body using ops (#16038)
- perf: node cjs & esm analysis cache (#16097)

### 1.26.0 / 2022.09.28

- feat: add --allow-sys permission flag (#16028)
- feat: add --no-npm flag to disable npm: imports (#15673)
- feat: Add requesting API name to permission prompt (#15936)
- feat: allow exiting on two consecutive ctrl+c presses (#15981)
- feat: download progress bar (#15814)
- feat: implement Web Cache API (#15829)
- feat: Refresh interactive permission prompt (#15907)
- feat: Stabilize Deno.hostname() API (#15932)
- feat: Stabilize Deno.refTimer() and Deno.unrefTimer() APIs (#16036)
- feat: TypeScript 4.8 update (#16040)
- feat(cli): update to TypeScript 4.8 (#15064)
- feat(core): add Deno.core.setPromiseHooks (#15475)
- feat(ext/crypto): add x25519 and Ed25519 CFRG curves (#14119)
- feat(ext/flash): add `reuseport` option on Linux (#16022)
- feat(info): add information about npm modules cache (#15750)
- feat(lint): add --compact flag for terse output (#15926)
- feat(npm): functionality to support child_process.fork (#15891)
- feat(ops): Fallible fast ops (#15989)
- feat(unstable): Deno.setRaw -> Deno.stdin.setRaw (#15797)
- fix(cli/bench): strace numeric format (#16055)
- fix(cli/vendor): handle assert type json during vendoring (#16059)
- fix(ext/console): fix error when logging a proxied Date (#16018)
- fix(ext/fetch): blob url (#16057)
- fix(ext/flash): reregister socket on partial read on Windows (#16076)
- fix(fmt): keep type args in type queries and keep empty array expr element's
  trailing comma (#16034)
- fix(npm): use ntfs junctions in node_modules folder on Windows (#16061)
- fix(require): tryPackage uses optional chaining (#16020)
- fix(runtime): refresh perm prompt 3 lines instead of 4 (#16049)
- perf: don't re-download package tarball to global cache if local node_modules
  folder exists for package (#16005)
- perf: use fast ops for tty (#15976)
- perf(ext/console): break on iterableLimit & better sparse array handling
  (#15935)
- perf(ext/fetch): use content-length in InnerBody.consume (#15925)

### 1.25.4 / 2022.09.22

- feat(unstable/npm): add flag for creating and resolving npm packages to a
  local node_modules folder (#15971)
- feat(unstable/npm): add support for --reload=npm: and --reload=npm:<package>
  (#15972)
- feat(internal/ops): Automatic fast ops creation (#15527)
- fix(compile): keep non-exe extension in output name on Windows (#15994)
- fix(doc): deno doc should parse modules if they haven't been parsed before
  (#15941)
- fix(ext/node): fix builtin module module (#15904)
- fix(ext/webgpu): make GPUDevice.features SetLike (#15853)
- fix(flash): panic if response if undefined (#15964)
- fix(runtime): better error message with Deno.env.get/set (#15966)
- fix(runtime): fix permission status cache keys (#15899)
- perf(cli): avoid `canonicalize_path` if config file does not exist (#15957)
- perf(cli): avoid `clap::App::clone` (#15951)
- perf(cli): use -O3 instead of -Oz (#15952)
- perf(core): use single ObjectTemplate for ops in `initialize_ops` (#15959)
- perf(ext/console): avoid `wrapConsole` when not inspecting (#15931)
- perf(web): optimize encodeInto() (#15922)
- perf: fs optimizations - part 1 (#15873)

### 1.25.3 / 2022.09.15

- doc(unstable): mention that `signal` input isn't supported in `spawnSync`
  (#15889)
- fix(ext/flash): don't block requests (#15852)
- fix(npm): align Deno importing Node cjs with Node esm importing cjs (#15879)
- fix(npm): align Node esm code importing cjs with Node (#15838)
- fix(npm): binary entrypoint for .js or no extension (#15900)
- fix(npm): remove export binding to match node (#15837)
- fix(npm): support cjs resolution of package subpath with package.json (#15855)
- fix(npm): use shim from deno_node crate for 'module' built-in module (#15881)
- fix(ops): add node.js env variable allowlist (#15893)
- perf(ext/flash): remove string->buffer cache (#15850)
- perf(serde_v8): remove Mutex from ZeroCopyBuf (#15888)
- perf(url): return early if url has no query string (#15856)
- perf: optimize URL serialization (#15663)

### 1.25.2 / 2022.09.09

- BREAKING(unstable): remove --compat mode (#15678)
- feat(ext/ffi): Implement FFI fast-call trampoline with Dynasmrt (#15305)
- feat(ext/ffi): Support bool FFI type (#15754)
- feat(serde_v8): Support StringObject as unit enum variant (#15715)
- fix(bench): make sure bytes/response is equal (#15763)
- fix(cli): Fix panic when providing invalid urls to --reload (#15784)
- fix(cli): allow using file resource synchronously while being used async
  (#15747)
- fix(cli/repl): await Promise.any([])... (#15623)
- fix(core): Register external references for imports to the SnapshotCreator
  (#15621)
- fix(core): make errors more resistant to tampering (#15789)
- fix(core): opAsync leaks a promise on type error (#15795)
- fix(docs): add missing categories for unstable (#15807)
- fix(docs): change category for Deno.Process to "Sub Process" (#15812)
- fix(ext/flash): use utf8 length as Content-Length (#15793)
- fix(ext/timers): create primordial `eval` (#15110)
- fix(init): suppress info logs when using quiet mode (#15741)
- fix(npm): add more context to errors when file doesn't exist (#15749)
- fix(npm): conditional exports in npm: specifiers (#15778)
- fix(npm): correct exact matching of pre-release versions (#15745)
- fix(npm): recursive translation of reexports, remove window global in node
  code (#15806)
- fix(npm): respect `latest` dist tag for getting current version (#15746)
- fix(ops): use qualified borrow in op macro (#15769)
- fix(repl): don't terminate on unhandled error events (#15548)
- fix(test): unflake wasm_unreachable test (#15794)
- fix(watch): ignore unload errors on drop (#15782)
- fix: upgrade deno_ast to 0.19 (#15808)
- perf(ops): inline &[u8] arguments and enable fast API (#15731)
- perf(runtime): flatten arguments for write_file ops (#15776)
- perf(runtime): short-circuit `queue_async_op` for Poll::Ready (#15773)

### 1.25.1 / 2022.09.01

- feat(ops): support `v8::FastApiCallbackOptions` (#15721)
- feat(serde_v8): Serialize integers as BigInt (#15692)
- fix(check): --remote and --no-remote should be mutually exclusive (#14964)
- fix(cli): `deno upgrade --canary` always downloaded latest version even if it
  was already latest (#15639)
- fix(compile): panic when running with a populated dep analysis cache (#15672)
- fix(docs): add missing categories (#15684)
- fix(ext/ffi): Fix pointer types (#15730)
- fix(ext/flash): add missing backticks in server docs (#15644)
- fix(ext/flash): panic on AddrInUse (#15607)
- fix(ext/flash): retry write failures (#15591)
- fix(ext/node): add missing primordial (#15595)
- fix(ext/node): better error for importing ES module via require() call
  (#15671)
- fix(ext/node): fix global in node env (#15622)
- fix(ext/websocket): fix closing of WebSocketStream with unread messages
  (#15632)
- fix(fmt): add the file path to the panic messages when formatting is unstable
  (#15693)
- fix(npm): better node version and version requirement compatibility (#15714)
- fix(npm): conditional exports with wildcards (#15652)
- fix(npm): handle cjs re-exports with the same name as an export (#15626)
- fix(npm): ignore npm cache directory creation errors (#15728)
- fix(npm): ignore the unstable error in the lsp (#15727)
- fix(npm): prefer importing esm from esm (#15676)
- fix(npm): skip extracting pax_global_header from tarballs (#15677)
- fix(npm): translate CJS to ESM with name clashes for files and dirs (#15697)
- fix(serde_v8): no panic on reading large text file (#15494)
- fix(serde_v8): update bytes::Bytes layout assumptions (#15718)
- fix: avoid global declaration collisions in cjs (#15608)
- fix: config file errors should not print specifier with debug formatting
  (#15648)
- fix: typo in deno_ops README (#15606)
- perf(ext/web): flatten op arguments for text_encoding (#15723)
- perf(ops): inline String args (#15681)
- perf(runtime): optimize allocations in read/write checks (#15631)
- perf: use fast api for `core.isProxy` (#15682)
- perf: use fast api for op_now (#15643)
- serde_v8: fix pointer size assumptions (#15613)

### 1.25.0 / 2022.08.24

- BREAKING(ext/ffi): specialized `buffer` type (#15518)
- feat(ext/crypto): deriveBits P-384 (#15138)
- feat(ext/flash): An optimized http/1.1 server (#15405)
- feat(ext/flash): split upgradeHttp into two APIs (#15557)
- feat(ops): V8 Fast Calls (#15291)
- feat(repl): add color to functions for syntax highlighting (#15434)
- feat(runtime): add pre_execute_module_cb (#15485)
- feat(unstable): initial support for npm specifiers (#15484)
- feat: `queueMicrotask()` error handling (#15522)
- feat: add "deno init" subcommand (#15469)
- fix(cache): do not attempt to emit non-emittable files (#15562)
- fix(core/runtime): always cancel termination in exception handling (#15514)
- fix(coverage): ensure coverage is only collected in certain situations
  (#15467)
- fix(ext/fetch): ignore user content-length header (#15555)
- fix(ext/flash): concurrent response streams (#15493)
- fix(ext/flash): fix default onListen callback (#15533)
- fix(ext/flash): fix listening port (#15519)
- fix: Free up JsRuntime state global handles before snapshot (#15491)
- fix: resolve `jsxImportSource` relative to module (#15561)
- perf(runtime): optimize Deno.file open & stream (#15496)
- perf: cache swc dependency analysis and don't hold onto `ParsedSource`s in
  memory (#15502)
- perf: improve performance.now (#15481)

### 1.24.3 / 2022.08.11

- fix(ext/fetch): add socks proxy support (#15372)
- feat(unstable/ext/ffi): add static method variants to Deno.UnsafePointerView
  (#15146)
- fix(cli): allow configurations files to also be json modules (#15444)
- fix(ext/ffi): check CStr for UTF-8 validity on read (#15318)
- fix(ext/ffi): unstable op_ffi_unsafe_callback_ref (#15439)
- fix(permissions): ignore empty values (#15447)
- fix(task): subcommand parser skips global args (#15297)
- fix: allow setting `globalThis.location` when no `--location` is provided
  (#15448)
- fix: update deno_graph to fix importing config as JSON module (#15388)
- fix: various formatting fixes (#15412)
- perf(ops): monomorphic sync op calls (#15337)

### 1.24.2 / 2022.08.04

- feat(ext/ffi): Add support to get ArrayBuffers from UnsafePointerView (#15143)
- feat(ext/ffi): Safe number pointers (#15173)
- fix(compat): use mjs extension for stream/promises (#15341)
- fix(core): BorrowMutError in nested error (#15352)
- fix(ext/webgpu): use correct IDL key name (#15278)
- fix(lsp): remove excessive line breaks in status page (#15364)
- fix(lsp): use correct commit chars for completions (#15366)
- fix(test): output parallel test results independently (#15399)
- fix(test): race condition for cancelled tests (#15233)
- fix(vendor): error on dynamic imports that fail to load instead of panicking
  (#15391)
- fix(vendor): existing import map with bare specifier in remote (#15390)
- fix: increase websocket message size (#15406)
- perf(ext/ffi): support Uint8Array in fast calls (#15319)
- perf(ext/ffi): use fast api calls for 64bit return types (#15313)

### 1.24.1 / 2022.07.28

- Revert "feat(ops): V8 Fast Calls (#15122)" (#15276)
- feat(ops): V8 Fast Calls (#15122)
- fix(cli): unset jsxFragmentFactory & jsxFactory options (#15264)
- fix(ext/fetch): resolve TODOs about WebIDL conversions in body init (#15312)
- fix(lsp): remove CompletionInfo.flags (#15288)
- fix(tools): upgrade to new `Deno.spawn` api (#15265)
- fix: Child.unref() unrefs stdio streams properly (#15275)
- fix: proper typings for unhandledrejection event (#15271)
- fix: unhandledrejection handling for sync throw in top level (#15279)
- perf(ext/ffi): Optimise common pointer related APIs (#15144)
- serde_v8: improvements to avoid hitting unimplemented codepaths (#13915)

### 1.24.0 / 2022.07.20

- BREAKING(unstable): Improve Deno.spawn() stdio API (#14919)
- feat(cli): support configuring the test tool in the config file (#15079)
- feat(cli/lsp): Sort repl completions (#15171)
- feat(cli/test): add `DENO_JOBS` env variable for `test` subcommand (#14929)
- feat(ext/ffi): Support 64 bit parameters in Fast API calls (#15140)
- feat(fmt): do not add a newline between a template and its tag (#15195)
- feat(lsp): provide import map remapping diags and fixes (#15165)
- feat(test): add `--parallel` flag, soft deprecate `--jobs` (#15259)
- feat(unstable): Ability to ref/unref "Child" in "Deno.spawnChild()" API
  (#15151)
- feat(web): add beforeunload event (#14830)
- feat: add "unhandledrejection" event support (#12994, #15211)
- feat: import.meta.resolve() (#15074)
- fix(cli): Improve error message in watch mode (#15184)
- fix(cli): expand tsc roots when using checkJs (#15164)
- fix(cli): synchronize async stdio/file reads and writes (#15092)
- fix(cli/dts): allow passing arguments to `WebAssembly` error constructors
  (#15149)
- fix(core): unhandled rejection in top-level scope (#15204)
- fix(coverage): do not verify emit source hash for coverage (#15260)
- fix(ext/ffi): allow setting a custom lib path for libtcc.a (#15208)
- fix(ext/ffi): i64 arg to C mapping was wrong (#15162)
- fix(ext/web): align DOMException better with spec (#15097)
- fix(fmt): improve curried arrow functions (#15251)
- fix(repl): do not panic for import completions when the import specifier is
  empty (#15177)
- fix(task): do not overflow attempting to parse large number as redirect
  (#15249)
- fix(task): resolve deno configuration file first from specified `--cwd` arg
  (#15257)
- fix: WebSocketStream ping event causes pending promises (#15235)
- fix: fallback to no type checking cache when db file can't be created (#15180)
- fix: revert changes to test output for uncaught errors (#15231)
- perf: emit files on demand and fix racy emit (#15220)
- perf: use emit from swc instead of tsc (#15118)

### 1.23.4 / 2022.07.12

- feat(core): Re-export v8 use_custom_libcxx feature (#14475)
- fix(core): deflake WASM termination test (#15103)
- fix(coverage): better handling of multi-byte characters (#15159)
- fix(ext/console): Fix a typo in a warning when .timeEnd is called on an
  unknown timer (#15135)
- fix(ext/crypto): Adjust typings for `Crypto.getRandomValues()` (#15130)
- fix(ext/ffi): Avoid keeping JsRuntimeState RefCell borrowed for event loop
  middleware calls (#15116)
- fix(ext/ffi): allow opting out of fast ffi calls (#15131)
- fix(ext/ffi): trampoline for fast calls (#15139)
- fix(ext/http) nextRequest return type annotation from ResponseEvent to
  RequestEvent (#15100)
- fix(ext/http): reading headers with ongoing body reader (#15161)
- fix(ext/url): missing primordial (#15096)
- fix(lsp): enable auto imports (#15145)
- fix(net): don't panic on failed UDS removal (#15157)
- fix: upgrade deno_ast to 0.17 (#15152)
- perf(cli/proc_state): Get error source lines from memory (#15031)
- perf(ext/ffi): leverage V8 Fast Calls (#15125)
- perf(ext/http): skip `core.isProxy` check for default ResponseInit (#15077)

### 1.23.3 / 2022.07.05

- Revert "refactor(snapshots): to their own crate (#14794)" (#15076)
- fix(cli): handle collecting a directory with file:// (#15002)
- fix(core): handle exception from Wasm termination (#15014)
- fix(core): remove unsafe in OpsTracker (#15025)
- fix(dts): stop default export type behavior (#14977)
- fix: update to TypeScript 4.7.4 (#15022)
- perf(ext/http): lazy load headers (#15055)
- perf(ext/http): remove accept_encoding interior mutability (#15070)
- perf(ext/http): simplify op_http_accept (#15067)
- perf(ops): fast path for SMI return values (#15033)
- perf(serde_v8): avoid extra is_array_buffer_view check (#15056)

### 1.23.2 / 2022.06.30

- feat(unstable/ffi): thread safe callbacks (#14942)
- fix(core): don't panic on non-existent cwd (#14957)
- fix(docs): --watch arg is stable (#14970)
- fix(dts/ffi): non-exact types break FFI inference (#14968)
- fix(ext/crypto): add EcdhKeyDeriveParams to deriveKey types (#15005)
- fix(ext/ffi): empty buffers error with index out of bounds on FFI (#14997)
- fix(ext/web): remove `ErrorEventInit`'s error default (#14809)
- fix(lsp): restart TS language service when caching dependencies (#14979)
- fix(modules): immediately resolve follow-up dyn imports to a dyn imported
  module (#14958)
- fix(runtime): derive default for deno_runtime::ExitCode (#15017)
- fix(task): remove --no-config as task subcommand argument (#14983)
- fix(test): typo ('finsihed') if text decoder not closed during test (#14996)
- fix(vendor): ignore import map in output directory instead of erroring
  (#14998)
- fix: don't error if Deno.bench() or Deno.test() are used in run subcommand
  (#14946)
- perf(ext/ffi): optimize synchronous calls (#14945)
- perf(ext/web): avoid reallocations in op_base64_atob (#15018)
- perf(ext/web): use base64-simd for atob/btoa (#14992)
- perf(serde_v8): smallvec ByteString (#15008)

### 1.23.1 / 2022.06.23

- BREAKING(unstable/ffi): Remove `Deno.UnsafePointer` indirection (#14915)
- feat(unstable/ffi): Callbacks (#14663)
- fix(check): ignore TS2306 (#14940)
- fix(docs): update description of `--check` flag (#14890)
- fix(ext/fetch): add `accept-language` default header to fetch (#14882)
- fix(ext/web): add EventTarget brand checking (#14637)
- fix(ext/web): handle rid=0 in TextDecoder#decode (#14894)
- fix(fmt): ignore node_modules directory (#14943)
- fix(fmt): should fail `--check` on parse error (#14907)
- fix(repl): accept tab when previous character is whitespace (#14898)
- fix(repl): use spaces for tab handler on windows (#14931)
- fix: do not panic running .d.cts and .d.mts files (#14917)
- fix: make Performance global an EventTarget
- fix: upgrade swc via deno_ast 0.16 (#14930)
- perf(core): Cache source lookups (#14816)
- perf(ext/ffi): Optimize FFI Rust side type checks (#14923)

### 1.23.0 / 2022.06.15

- BREAKING: remove `Intl.v8BreakIterator` (#14864)
- BREAKING: Remove unstable Deno.sleepSync (#14719)
- Deno.exit() is an alias to self.close() in worker contexts (#14826)
- feat(console): pass options and depth to custom inspects (#14855)
- feat(ext/crypto): export elliptic keys as "raw" (#14764)
- feat(ext/ffi): support passing and returning bigints (#14523)
- feat(fmt): remove some unnecessary parens in types (#14841)
- feat(fmt): support formatting cjs, cts, mjs, and mts files (#14837)
- feat(ops): 'hybrid' ops - sync returning future (#14640)
- feat(repl): Add key binding to force a new line (#14536)
- feat(runtime/signal): implement SIGINT and SIGBREAK for windows (#14694)
- feat(task): add `--cwd` flag for configuring the working directory (#14823)
- feat(task): support redirects, cat, xargs (#14859)
- feat(test): update test summary report (#14629)
- feat(vendor): support using an existing import map (#14836)
- feat: make Child.kill argument optional (#14669)
- feat: no type-check by default (#14691)
- feat: update to TypeScript 4.7 (#14242)
- feat(web): enable deflate-raw compression format (#14863)
- fix(check): use "moduleDetection": "force" (#14875)
- fix(cli): add config flag to `deno info` (#14706)
- fix(console): control inspect() indent with option (#14867)
- fix(url): properly indent when inspecting URLs (#14867)
- upgrade: v8 10.4.132.5 (#14874)

### 1.22.3 / 2022.06.09

- fix(ext/fetch): remove deprecation of `URL` in deno `fetch` (#14769)
- fix(http/upgradewebsocket): check for open state for idle timeout (#14813)
- fix(lsp): change glob to watch json and jsonc files (#14828)
- fix(lsp): handle get diagnostic errors better (#14776)
- fix(task): support parsing quotes in a word (#14807)
- fix: Format non-error exceptions (#14604)
- fix: watch dynamic imports in --watch (#14775)

### 1.22.2 / 2022.06.02

- feat(unstable): add Deno.getGid (#14528)
- fix(cli/dts): add `captureStackTrace` to `lib.dom.extras` (#14748)
- fix(ext/crypto): adjust `getRandomValues` types (#14714)
- fix(fmt): do panic for import decl with empty named imports and default import
  (#14773)

### 1.22.1 / 2022.05.27

- fix(bench): update typo in bench summary (#14672)
- fix(cli/dts): change `ChildStatus.signal` from `string` to `Deno.Signal`
  (#14690)
- fix(core): op metrics op_names mismatch (#14716)
- fix(core): rethrow exception during structured cloning serialization (#14671)
- fix(coverage): do not report transpiled files with no lines (#14699)
- fix(ext/websocket): WebSocket dispatch single close event (#13443)
- fix(fmt): prevent infinite loop when formatting certain binary expressions
  (#14725)
- fix(runtime): improve permission descriptor validation (#14676)
- fix(vendor): handle relative imports when mapped local folder name differs
  from remote's (#14465)
- fix: deno task should actually use current exe for `deno` command (#14705)
- fix: prevent Deno.exit to fail when dispatchEvent tampered (#14665)
- fix: read raw stdin to prevent buffering (regression) (#14704)

### 1.22.0 / 2022.05.18

- BREAKING(unstable): Enable Deno namespace in workers by default (#14581)
- BREAKING: Remove unstable Deno.applySourceMap API (#14473)
- BREAKING: Remove unstable Deno.emit and Deno.formatDiagnostics APIs (#14463)
- feat(core): deterministic snapshots (#14037)
- feat(core): Revert "core: don't include_str extension js code (#10786)"
  (#14614)
- feat(ext/net): add "NS" record support in Deno.resolveDns API (#14372)
- feat(ext/net): add `CAA` DNS record support in Deno.resolveDns() API (#14624)
- feat(ext/net): add support for SOA records in Deno.resolveDns() API (#14534)
- feat(ext/net): support NAPTR records in Deno.resolveDns() API (#14613)
- feat(ext/net): support full `SOA` record interface (#14617)
- feat(ext/web): add performance.toJSON (#14548)
- feat(ext/web): implement static `Response.json` (#14566)
- feat(lsp): enable linting by default (#14583)
- feat(ops): #[op(v8)] (#14582)
- feat(ops): allow passing scope handle to ops (#14574)
- feat(ops): infallible / result-free ops (#14585)
- feat(ops): sync Rc<RefCell<OpState>> (#14438)
- feat(runtime/spawn): add `AbortSignal` support (#14538)
- feat(serde_v8): bytes::Bytes support (#14412)
- feat(test): Represent uncaught errors (#14513)
- feat(test): Show Deno.test() call locations for failures (#14484)
- feat(test): change "failures:" headers in test report (#14490)
- feat(test): repeat test name if there's user output (#14495)
- feat(unstable/task): resolve the current executable for the deno command
  (#14462)
- feat(web): add `performance.timeOrigin` (#14489)
- feat: add --no-config flag (#14555)
- feat: add userAgent property to Navigator's prototype (#14415)
- feat: return a signal string instead number on ChildStatus (#14643)
- feat: subcommands type-check only local files by default (#14623)
- fix(core): support classifying ENOTDIR (#14646)
- fix(ext/http): error on invalid headers (#14642)
- fix(ext/http): make serveHttp compress for Accept-Encoding: deflate, gzip
  (#14525)
- fix(ext/http): no response body reader when cancelling during shutdown
  (#14653)
- fix(ext/http): skip auto-compression if content-encoding present (#14641)
- fix(ext/tls): ability to ignore IP-address certificate errors (#14610)
- fix(ext/web): throw if listener and signal are null (#14601)
- fix(lsp): correct positions in some scenarios (#14359)
- fix: base64 encoding of source maps with emojis (#14607)
- perf(core): optimize encode on large strings (#14619)
- perf(ext/http): faster accept-encoding parsing (#14654)
- perf(ext/web): Add fast path for non-streaming TextDecoder (#14217)
- perf(serde_v8): fast path for large strings (#14450)

### 1.21.3 / 2022.05.12

- fix(cli): add deno version to manual links (#14505)
- fix(core): avoid panic on non-string Error.name (#14529)
- fix(ext/tls): finish TLS handshake before shutting down (#14547)
- fix(runtime): stdout and stderr encoding on Windows (#14559)
- fix(task): accept double hyphen arg immediately following task name (#14567)
- fix(test): do not panic on `TestOutputPipe::flush` when receiver dropped
  (#14560)
- fix(workers): make module evaluation result deterministic (#14553)

### 1.21.2 / 2022.05.05

- fix(cli): add dom.extras lib (#14430)
- fix(coverage): exclude .snap files (#14480)
- fix(ext/http): explicitly close resource after reading (#14471)
- fix(runtime): lossy utf8 readTextFile (#14456)
- fix(task): allow hyphen values after task name (#14434)
- fix(task): support forwarding lone double hyphen (#14436)
- fix(test): actually capture stdout and stderr in workers (#14435)
- fix(test/bench): accept file protocol module specifier CLI args (#14429)
- fix(vendor): do not panic on relative specifier with scheme-like folder name
  (#14453)
- fix: improve formatting jsdocs with asterisk as first char on line (#14446)

### 1.21.1 / 2022.04.28

- Reland "feat(ext/http): stream auto resp body compression" (#14345)
- Reland "perf(http): optimize ReadableStreams backed by a resource" (#14346)
- feat(ext/console): Add string abbreviation size option for "Deno.inspect"
  (#14384)
- fix(ext/console): Compact empty iterables when calling Deno.inspect with
  compact false (#14387)
- fix: change shade of "gray" color in eye-catchers (#14309)
- fix(bench): eliminate sanitizeExit overhead (#14361)
- fix(bench): report pending summary before clearing (#14369)
- fix(bench): reset reporter context (#14360)
- fix(cli): wrap long line of the env variables help (#14422)
- fix(ext/http): truncate read bytes when streaming bodies (#14389)
- fix(runtime/js/spawn): Pass stdio options for spawn() and spawnSync() (#14358)
- fix(test): capture inherited stdout and stderr for subprocesses in test output
  (#14395)
- fix(test): capture worker stdout and stderr in test output (#14410)
- fix(watcher): don't clear screen on start (#14351)
- fix(workers): Make `worker.terminate()` not block the current thread (#13941)
- fix: `deno task` forward double hyphen (#14419)
- perf(ext/http): fast path for uncompressed bodies (#14366)
- perf(ext/http): faster is_content_compressible (#14383)
- perf(runtime): read entire files in single ops (#14261)
- perf(serde_v8): zero-copy StringOrBuffer (#14381)

### 1.21.0 / 2022.04.20

- feat(bench): update API, new console reporter (#14305)
- feat(cli/fmt): ignore .git folder when formatting files (#14138)
- feat(core): Add initial support for realms (#14019)
- feat(ext/net): Deno.upgradeHttp handles unix connections (#13987)
- feat(ext/web): add globalThis.reportError() (#13799)
- feat(repl): Don't type check when importing modules (#14112)
- feat(repl): add "--eval-file" flag to execute a script file on startup
  (#14247)
- feat(repl): add global clear() function (#14332)
- feat(runtime): two-tier subprocess API (#11618)
- feat(test): Add "name", "origin" and "parent" to "Deno.TestContext" (#14007)
- feat(test): Improve testing report output (#14255)
- feat(test): format user code output (#14271)
- feat(test): skip internal stack frames for errors (#14302)
- feat(test): use structured data for JavaScript errors in tests (#14287)
- feat: Add "deno check" subcommand for type checking (#14072)
- feat: Add DENO_NO_PROMPT variable (#14209)
- feat: Better formatting for AggregateError (#14285)
- fix(cli/emit): Check JS roots with // @ts-check (#14090)
- fix(cli/tools/test): Prefix test module paths with "./" (#14301)
- fix(fmt): regression where some short if stmt headers being split on multiple
  lines (#14292)
- fix(permissions): fallback to denied access if the permission prompt fails
  (#14235)
- fix: `--watch` was losing items (#14317)
- fix: panic when trying to pledge permissions before restoring previous pledge
  (#14306)
- perf(fmt/lint): incremental formatting and linting (#14314)
- perf(runtime): bypass tokio file and bump op buffer size to 64K (#14319)
- perf: move `Deno.writeTextFile` and like functions to Rust (#14221)
- upgrade: rusty_v8 0.42.0 (#14334)

### 1.20.6 / 2022.04.14

- fix(serde_v8): more robust number deserialization (#14216)
- fix(test): Don't error on missing op details (#14184)
- fix: upgrade to swc_ecmascript 0.143 (#14238)

### 1.20.5 / 2022.04.07

- feat(lsp/unstable): add experimental testing API (#13798)
- feat(lsp/unstable): support tasks in the config file (#14139)
- feat(unstable): add ref/unref to Listener (#13961)
- fix(cli/install): preserve compat flag (#14223)
- fix(ext/crypto): check extractable in exportKey (#14222)

### 1.20.4 / 2022.03.31

- fix(compile): follow redirects when resolving (#14161)
- fix(ext/fetch): extend deprecated fetch() overload with `string | Request`
  (#14134)
- fix(lsp): watch .jsonc files (#14135)
- fix(runtime/ops/signal.rs): Add Solaris signals (#13931)
- fix(task): handle `PATHEXT` with trailing semi-colon (#14140)
- perf: micro-optimize core.encode (#14120)

### 1.20.3 / 2022.03.25

- fix(ext/fetch): deprecate URL as the first arg in types (#14113)
- fix(ext/ffi): enforce unstable check on ops (#14115)
- fix(runtime): do not modify user provided `cmd` array in `Deno.run` (#14109)

### 1.20.2 / 2022.03.24

- feat(lsp): support deno.enablePaths setting (#13978)
- fix(bench): require --unstable flag in JavaScript (#14091)
- fix(test): don't error on missing op details (#14074)
- fix(compat): Changes an instance of collect::<Vec<_>>().join("") to
  collect::<String>() (#14082)
- fix(tests): do not use global env vars in install tests (#14078)
- fix(ext/fetch): Connect async error stack with user code (#13899)
- fix(unstable): upgrade deno_task_shell to 0.2 (#14073)
- fix: upgrade to swc_ecmascript 0.137.0 (#14067)
- fix(fetch): Fix uncaught rejection panic with
  `WebAssembly.instantiateStreaming` (#13925)
- fix(core): variadic opSync/opAsync (#14062)
- fix(runtime): actually don't inherit runtime permissions (#14024)
- fix(ext/console): fix error with a Proxy of a Map (#14032)
- fix(ops): throw TypeError on op return failure (#14033)
- fix(cli): improve `deno compile` error messages (#13944)
- fix(cli): add support for DENO_CERT in upgrade command (#13862)
- fix(config-file): fix config-file.v1.json schema to allow colons in the task
  name (#14013)
- perf(http): avoid Set.has() when closing connection resource (#14085)
- perf(http): avoid checking promise every request (#14079)
- perf(http): avoid per header alloc (#14051)

### 1.20.1 / 2022.03.16

- BREAKING: don't inherit permissions by default (#13668)
- feat(cli): support data url (#13667)
- feat(cli): update to TypeScript 4.6.2 (#13474)
- feat(compat): CJS/ESM interoperability (#13553)
- feat(core): Event loop middlewares for Extensions (#13816)
- feat(core): codegen ops (#13861)
- feat(ext/crypto): AES-GCM support for 128bit IVs (#13805)
- feat(ext/fetch): Allow Response status 101 (#13969)
- feat(ext/http): auto-compression of fixed response bodies (#13769)
- feat(ext/net): Use socket2 crate to create TcpListener (#13808)
- feat(ext/net): support cert, key options in listenTls (#13740)
- feat(ext/web): Add `AbortSignal.timeout()` (#13687)
- feat(net): add Deno.UnixConn interface (#13787)
- feat(ops): custom arity (#13949)
- feat(ops): optional OpState (#13954)
- feat(unstable): Add Deno.upgradeHttp API (#13618)
- feat: "deno bench" subcommand (#13713)
- feat: "deno task" subcommand (#13725)
- feat: Add Deno.TcpConn class, change return type from Deno.connect (#13714)
- feat: allow specification of import map in config file (#13739)
- feat: deno test --trace-ops (#13770)
- fix(compat): cjs/esm interop for dynamic imports (#13792)
- fix(core): Don't override structured clone error messages from V8 (#13942)
- fix(core): nuke Deno.core.ops pre-snapshot (#13970)
- fix(ext/crypto): handle JWK import with "use" (#13912)
- fix(ext/crypto): use EcKeyImportParams dictionary (#13894)
- fix(ext/http): drop content-length header on compression (#13866)
- fix(info): print deno info paths with unescaped backslashes on windows
  (#13847)
- fix(test): skip typechecking for blocks inside HTML comments (#13889)
- fix: shell completion hints (#13876)
- fix: upgrade reqwest to 0.11.10 (#13951)
- perf(web): Optimize `TextDecoder` by adding a new `U16String` type (#13923)
- perf(web): optimize Blob.text and Blob.arrayBuffer (#13981)
- perf(web): use DOMString for BlobParts (#13979)
- perf: opt-level-3 all of ext (#13940)

Note 1.20.0 was dead on arrival, see https://github.com/denoland/deno/pull/13993

### 1.19.3 / 2022.03.10

- fix(ci): restore compatibility with older glibc (#13846)
- fix(test): typecheck blocks annotated with long js/ts notations (#13785)
- perf(core): micro-optimize OpsTracker (#13868)
- perf(ext/web): optimize atob/btoa (#13841)
- perf(serde_v8): avoid SerializablePkg allocs (#13860)
- perf(serde_v8): optimize ByteString deserialization (#13853)

### 1.19.2 / 2022.03.03

- fix(cli): disable config discovery for remote script (#13745)
- fix(repl): fix null eval result (#13804)
- fix(runtime): disable console color for non tty stdout (#13782)
- fix(test): use --no-prompt by default (#13777)

### 1.19.1 / 2022.02.24

- feat(ext/ffi): Support read only global statics (#13662)
- fix(compile): Support import maps (#13756)
- fix(upgrade): move the file permission check to the beginning of the upgrade
  process (#13726)
- fix(vendor): do not add absolute specifiers to scopes (#13710)

### 1.19.0 / 2022.02.17

- feat: Add Deno.FsFile, deprecate Deno.File (#13660)
- feat: Add hint to permission prompt to display allow flag (#13695)
- feat: deno vendor (#13670)
- feat: never prompt for hrtime permission (#13696)
- feat: permission prompt by default (#13650)
- feat(compat): support --compat in web workers (#13629)
- feat(compile): Replace bundling with eszip in deno compile (#13563)
- feat(coverage): add "--output" flag (#13289)
- feat(ext/console): better circular information in object inspection (#13555)
- feat(ext/http): add support for unix domain sockets (#13628)
- feat(ext/net): Add Conn.setNoDelay and Conn.setKeepAlive (#13103)
- feat(ext/web): add CompressionStream API (#11728)
- feat(lsp): add redirect diagnostic and quick fix (#13580)
- feat(lsp): provide completions from import map if available (#13624)
- feat(lsp): support linking to symbols in JSDoc on hover (#13631)
- feat(runtime): stabilize addSignalListener API (#13438)
- feat(runtime): web streams in fs & net APIs (#13615)
- feat(test): better errors for resource sanitizer (#13296)
- feat(test): improved op sanitizer errors + traces (#13676)
- feat(watch): add "--no-clear-screen" flag (#13454)
- fix(compat): ESM resolver for package subpath (#13599)
- fix(ext/console): fix uncaught TypeError in css styling (#13567)
- fix(ext/console): print circular ref indicator in cyan (#13684)
- fix(ext/crypto): optional additionalData in encrypt/decrypt (#13669)
- fix(ext/crypto): support EC p256 private key material in exportKey (#13547)
- fix(lsp): do not panic getting root_uri to auto discover configuration file
  (#13603)
- fix(lsp): independent diagnostic publishing should include all diagnostic
  sources on each publish (#13483)
- fix(lsp): op_exists handles bad specifiers (#13612)

### 1.18.2 / 2022.02.03

- feat(unstable): add Deno.getUid (#13496)
- fix: don't crash when $HOME is a relative path (#13581)
- fix(cli): handle extensionless imports better (#13548)
- fix(cli): handle local files with query params on emit (#13568)
- fix(cli/dts/webgpu): make GPUBlendComponent properties optional (#13574)
- fix(ext/crypto): enforce 128bits tagLength for AES-GCM decryption (#13536)
- fix(ext/crypto): utf16 jwk encoding (#13535)
- fix(lsp): properly display x-deno-warning with redirects (#13554)
- fix(lsp): regression where certain diagnostics were showing for disabled files
  (#13530)
- fix(repl): tab completions (#13540)
- perf(lsp): cancellable TS diagnostics (#13565)

### 1.18.1 / 2022.01.27

- feat(unstable): add Deno.networkInterfaces (#13475)
- fix(ext/crypto): duplicate RsaHashedImportParams types (#13466)
- fix(lsp): respect DENO_CERT and other options related to TLS certs (#13467)
- perf(lsp): improve some tsc op hot paths (#13473)
- perf(lsp): independent diagnostic source publishes (#13427)

### 1.18.0 / 2022.01.20

- feat: auto-discover config file (#13313)
- feat: output `cause` on JS runtime errors (#13209)
- feat: stabilize test steps API (#13400)
- feat(cli, runtime): compress snapshots (#13320)
- feat(cli): add ignore directives to bundled code (#13309)
- feat(compat) preload Node.js built-in modules in global vars REPL (#13127)
- feat(ext/crypto): implement AES-GCM decryption (#13319)
- feat(ext/crypto): implement AES-GCM encryption (#13119)
- feat(ext/crypto): implement AES-KW for wrapKey/unwrapKey (#13286)
- feat(ext/crypto): implement pkcs8/JWK for P-384 curves (#13154)
- feat(ext/crypto): implement pkcs8/spki/jwk exportKey for ECDSA and ECDH
  (#13104)
- feat(ext/crypto): JWK support for unwrapKey/wrapKey (#13261)
- feat(ext/crypto): support AES-CTR encrypt/decrypt (#13177)
- feat(ext/crypto): support importing raw EC keys (#13079)
- feat(ext/ffi): infer symbol types (#13221)
- feat(ext/ffi): support alias names for symbol definitions (#13090)
- feat(ext/ffi): UnsafeFnPointer API (#13340)
- feat(ext/websocket): add header support to WebSocketStream (#11887)
- feat(ext/websocket): server automatically handle ping/pong for incoming
  WebSocket (#13172)
- feat(lsp): provide registry details on hover if present (#13294)
- feat(runtime): add op_network_interfaces (#12964)
- feat(serde_v8): deserialize ArrayBuffers (#13436)
- feat(streams): reject pending reads when releasing reader (#13375)
- feat(test): Add support for "deno test --compat" (#13235)
- fix(cli): Don't strip shebangs from modules (#13220)
- fix(cli): fix `deno install --prompt` (#13349)
- fix(cli/dts): add NotSupported error type (#13432)
- fix(ext/console): don't depend on globalThis present (#13387)
- fix(ext/crypto): validate maskGenAlgorithm asn1 in importKey (#13421)
- fix(ext/ffi): `pointer` type can accept `null` (#13335)
- fix(fmt): markdown formatting should not remove backslashed backslash at start
  of paragraph (#13429)
- fix(lsp): better handling of registry config errors (#13418)
- fix(runtime): don't crash when window is deleted (#13392)
- fix(streams): update TypeError message for pending reads when releasing reader
  (#13376)
- fix(tsc): Add typings for `Intl.ListFormat` (#13301)

### 1.17.3 / 2022.01.12

- fix: Get lib.deno_core.d.ts to parse correctly (#13238)
- fix: expose "Deno.memoryUsage()" in worker context (#13293)
- fix: install shim with `--allow-all` should not output each permission
  individually (#13325)
- fix(compile): fix output flag behaviour on compile command (#13299)
- fix(coverage): don't type check (#13324)
- fix(coverage): merge coverage ranges (#13334)
- fix(ext/web): handle no arguments in atob (#13341)
- fix(serde_v8): support #[serde(default)] (#13300)

### 1.17.2 / 2022.01.05

- fix(cli): include JSON modules in bundle (#13188)
- fix(core): inspector works if no "Runtime.runIfWaitingForDebugger" message is
  sent (#13191)
- fix(coverage): use only string byte indexes and 0-indexed line numbers
  (#13190)
- fix(doc): Make private types which show up in the rustdocs public (#13230)
- fix(ext/console): map basic css color keywords to ansi (#13175)
- fix(ext/crypto) - exportKey JWK for AES/HMAC must use base64url (#13264)
- fix(ext/crypto) include AES-CTR for deriveKey (#13174)
- fix(ext/crypto): use forgiving base64 encoding for JWK (#13240)
- fix(ext/ffi): throw errors instead of panic (#13283)
- fix(lsp): add code lens for tests just using named functions (#13218)
- fix(lsp): better handling of folders in registry completions (#13250)
- fix(lsp): handle repeating patterns in registry correctly (#13275)
- fix(lsp): properly generate data URLs for completion items (#13246)
- fix(signals): prevent panic when listening to forbidden signals (#13273)
- fix: support `mts`, `cjs` & `cts` files for `deno test` & `deno fmt` (#13274)
- fix: upgrade swc_ecmascript to 0.103 (#13284)

### 1.17.1 / 2021.12.22

- feat(lsp, unstable): add code lens for debugging tests (#13138)
- feat(lsp, unstable): supply accept header when fetching registry config
  (#13159)
- fix: inspector prompts (#13123)
- fix(coverage): Split sources by char index (#13114)
- fix(ext/ffi): use `c_char` instead of `i8` for reading strings (#13118)
- fix(ext/websocket): WebSocketStream don't error with "sending after closing"
  when closing (#13134)
- fix(repl): support assertions on import & export declarations (#13121)

### 1.17.0 / 2021.12.16

- feat: add `--no-check=remote` flag (#12766)
- feat: Add support for import assertions and JSON modules (#12866)
- feat: REPL import specifier auto-completions (#13078)
- feat: support abort reasons in Deno APIs and `WebSocketStream` (#13066)
- feat: support compat mode in REPL (#12882)
- feat(cli): update to TypeScript 4.5 (#12410)
- feat(core): Add ability to "ref" and "unref" pending ops (#12889)
- feat(core): intercept unhandled promise rejections (#12910)
- feat(ext/crypto): implement unwrapKey (#12539)
- feat(ext/crypto): support `importKey` in SPKI format (#12921)
- feat(ext/crypto): support exporting RSA JWKs (#13081)
- feat(ext/crypto): support importing ECSDA and ECDH (#13088)
- feat(ext/crypto): support importing exporting AES JWK keys (#12444)
- feat(ext/crypto): support importing RSA JWKs (#13071)
- feat(ext/fetch): Support `WebAssembly.instantiateStreaming` for file fetches
  (#12901)
- feat(ext/fetch): support abort reasons in fetch (#13106)
- feat(ext/ffi): implement UnsafePointer and UnsafePointerView (#12828)
- feat(ext/net): ALPN support in `Deno.connectTls()` (#12786)
- feat(ext/net): enable sending to broadcast address (#12860)
- feat(ext/timers): add refTimer, unrefTimer API (#12953)
- feat(ext/web): implement `AbortSignal.prototype.throwIfAborted()` (#13044)
- feat(lsp): add type definition provider (#12789)
- feat(lsp): add workspace symbol provider (#12787)
- feat(lsp): improve registry completion suggestions (#13023)
- feat(lsp): registry suggestion cache respects cache headers (#13010)
- feat(repl): add --unsafe-ignore-certificate-errors flag (#13045)
- feat(runtime): add op_set_exit_code (#12911)
- feat(streams): support abort reasons in streams (#12991)
- feat(test): Add more overloads for "Deno.test" (#12749)
- feat(watch): clear screen on each restart (#12613)
- feat(watch): support watching external files (#13087)
- fix: support "other" event type in FSWatcher (#12836)
- fix(cli): config file should resolve paths relative to the config file
  (#12867)
- fix(cli): don't add colors for non-tty outputs (#13031)
- fix(cli): don't cache .tsbuildinfo unless emitting (#12830)
- fix(cli): fix slow test, unbreak ci (#12897)
- fix(cli): skip bundling for pre-bundled code in "compile" (#12687)
- fix(ext/crypto): throw on key & op algo mismatch (#12838)
- fix(ext/crypto): various cleanup in JWK imports (#13092)
- fix(ext/net): make unix and tcp identical on close (#13075)
- fix(ext/timers): fix flakiness of `httpConnAutoCloseDelayedOnUpgrade` test
  (#13017)
- fix(ext/web): set location undefined when `--location` is not specified
  (#13046)
- fix(lsp): handle import specifier not having a trailing quote (#13074)
- fix(lsp): lsp should respect include/exclude files in format config (#12876)
- fix(lsp): normalize urls in did_change_watched_files (#12873)
- fix(lsp): provide diagnostics for import assertions (#13105)
- fix(workers): Make `worker.terminate()` not immediately kill the isolate
  (#12831)

### 1.16.4 / 2021.12.03

- fix(core): Wake up the runtime if there are ticks scheduled (#12933)
- fix(core): throw on invalid callConsole args (#12973) (#12974)
- fix(ext/crypto): throw on key & op algo mismatch (#12838)
- fix(test): Improve reliability of `deno test`'s op sanitizer with timers
  (#12934)
- fix(websocket): bad rid on WebSocketStream abort (#12913)
- fix(workers): Make `worker.terminate()` not immediately kill the isolate
  (#12831)

### 1.16.3 / 2021.11.24

- fix(cli): config file should resolve paths relative to the config file
  (#12867)
- fix(cli): don't cache .tsbuildinfo unless emitting (#12830)
- fix(cli/compile): skip bundling for pre-bundled code (#12687)
- fix(core): don't panic when evaluating module after termination (#12833)
- fix(core): keep event loop alive if there are ticks scheduled (#12814)
- fix(ext/crypto): don't panic on decryption failure (#12840)
- fix(ext/fetch): HTTP/1.x header case got discarded on the wire (#12837)
- fix(fmt): markdown formatting was incorrectly removing some non-breaking space
  html entities (#12818)
- fix(lsp): lsp should respect include/exclude files in format config (#12876)
- fix(lsp): normalize urls in did_change_watched_files (#12873)
- fix(lsp): tag deprecated diagnostics properly (#12801)
- fix(lsp): use lint exclude files list from the config file (#12825)
- fix(runtime): support "other" event type in FSWatcher (#12836)
- fix(runtime): support reading /proc using readFile (#12839)
- fix(test): do not throw on error.errors.map (#12810)

### 1.16.2 / 2021.11.17

- feat(unstable/test): include test step pass/fail/ignore counts in final report
  (#12432)
- fix(cli): short-circuit in prepare_module_load() (#12604)
- fix(lsp): retain module dependencies when parse is invalid (#12782)
- fix(test): support typechecking docs with CRLF line endings (#12748)
- fix(transpile): do not panic on `swc_ecma_utils::HANDLER` diagnostics (#12773)

### 1.16.1 / 2021.11.11

- feat(core): streams (#12596)
- fix(crypto): handling large key length in HKDF (#12692)
- fix: add typings for AbortSignal.reason (#12730)
- fix(http): non ascii bytes in response (#12728)
- fix: update unstable Deno props for signal API (#12723)

### 1.16.0 / 2021.11.09

- BREAKING(ext/web): remove `ReadableStream.getIterator` (#12652)
- feat(cli): support React 17 JSX transforms (#12631)
- feat(compat): add .code to dyn import error (#12633)
- feat(compat): integrate import map and classic resolutions in ESM resolution
  (#12549)
- feat(ext/console): Display error.cause in console (#12462)
- feat(ext/fetch): support fetching local files (#12545)
- feat(ext/net): add TlsConn.handshake() (#12467)
- feat(ext/web): BYOB support for ReadableStream (#12616)
- feat(ext/web): WritableStreamDefaultController.signal (#12654)
- feat(ext/web): add `AbortSignal.reason` (#12697)
- feat(ext/webstorage): use implied origin when --location not set (#12548)
- feat(runtime): add Deno.addSignalListener API (#12512)
- feat(runtime): give OS errors .code attributes (#12591)
- feat(test): better formatting for test elapsed time (#12610)
- feat(runtime): Stabilize Deno.TestDefinition.permissions (#12078)
- feat(runtime): stabilize Deno.startTls (#12581)
- feat(core): update to V8 9.7 (#12685)
- fix(cli): do not cache emit when diagnostics present (#12541)
- fix(cli): don't panic when mapping unknown errors (#12659)
- fix(cli): lint/format all discovered files on each change (#12518)
- fix(cli): linter/formater watches current directory without args (#12550)
- fix(cli): no-check respects inlineSources compiler option (#12559)
- fix(cli/upgrade): nice error when unzip is missing (#12693)
- fix(encoding): support additional encoding labels (#12586)
- fix(ext/fetch): Replace redundant local variable with inline return statement
  (#12583)
- fix(ext/http): allow multiple values in upgrade header for websocket (#12551)
- fix(ext/net): expose all tls ops (#12699)
- fix(fetch): set content-length for empty POST/PUT (#12703)
- fix(fmt): reduce likelihood of deno fmt panic for file with multi-byte chars
  (#12623)
- fix(fmt/lint): strip unc paths on Windows when displaying file paths in lint
  and fmt (#12606)
- fix(lint): use recommended tag if there is no tags in config file or flags
  (#12644)
- fix(lint): use recommended tags when no tags specified in config, but includes
  or excludes are (#12700)
- fix(lsp): cache unsupported import completion origins (#12661)
- fix(lsp): display module types only dependencies on hover (#12683)
- fix(lsp): display signature docs as markdown (#12636)
- fix(runtime): require full read and write permissions to create symlinks
  (#12554)
- fix(tls): Make TLS clients support HTTP/2 (#12530)
- fix(webidl): Don't throw when converting a detached buffer source (#12585)
- fix(workers): Make `importScripts()` use the same HTTP client as `fetch`
  (#12540)
- fix: Deno.emit crashes with BorrowMutError (#12627)
- fix: support verbatim UNC prefixed paths on Windows (#12438)
- fix: typings for BYOB stream readers (#12651)
- perf(core): optimize waker capture in AsyncRefCell (#12332)
- perf(encoding): avoid copying the input data in `TextDecoder` (#12573)
- perf(http): encode string bodies in op-layer (#12451)
- perf: optimize some important crates more aggressively (#12332)

### 1.15.3 / 2021.10.25

- feat(serde_v8): StringOrBuffer (#12503)
- feat(serde_v8): allow all values to deserialize to unit type (#12504)
- fix(cli/dts): update std links for deprecations (#12496)
- fix(cli/tests): flaky Deno.watchFs() tests (#12485)
- fix(core): avoid op_state.borrow_mut() for OpsTracker (#12525)
- fix(core/bindings): use is_instance_of_error() instead of is_native_error()
  (#12479)
- fix(ext/net): fix TLS bugs and add 'op_tls_handshake' (#12501)
- fix(ext/websocket): prevent 'closed normally' panic (#12437)
- fix(lsp): formatting should error on certain additional swc diagnostics
  (#12491)
- fix: declare web types as global (#12497)

### 1.15.2 / 2021.10.18

- feat(unstable): Node CJS and ESM resolvers for compat mode (#12424)
- fix(cli): re-enable allowSyntheticDefaultImports for tsc (#12435)
- fix(cli/fmt_errors): don't panic on source line formatting errors (#12449)
- fix(cli/tests): move worker test assertions out of message handlers (#12439)
- fix(console): fix display of primitive wrapper objects (#12425)
- fix(core): avoid polling future after cancellation (#12385)
- fix(core): poll async ops eagerly (#12385)
- fix(fmt): keep parens for JS doc type assertions (#12475)
- fix(fmt): should not remove parens around sequence expressions (#12461)
- fix(runtime/ops/worker_host): move permission arg parsing to Rust (#12297)

### 1.15.1 / 2021.10.13

- fix: `--no-check` not properly handling code nested in TS expressions (#12416)
- fix: bundler panic when encountering export specifier with an alias (#12418)

### 1.15.0 / 2021.10.12

- feat: add --compat flag to provide built-in Node modules (#12293)
- feat: provide ops details for ops sanitizer failures (#12188)
- feat: Show the URL of streaming WASM modules in stack traces (#12268)
- feat: Stabilize Deno.kill and Deno.Process.kill (#12375)
- feat: stabilize Deno.resolveDns (#12368)
- feat: stabilize URLPattern API (#12256)
- feat: support serializing `WebAssembly.Module` objects (#12140)
- feat(cli/uninstall): add uninstall command (#12209)
- feat(ext/crypto): decode RSAES-OAEP-params with default values (#12292)
- feat(ext/crypto): export spki for RSA (#12114)
- feat(ext/crypto): implement AES-CBC encryption & decryption (#12123)
- feat(ext/crypto): implement deriveBits for ECDH (p256) (#11873)
- feat(ext/crypto): implement deriveKey (#12117)
- feat(ext/crypto): implement wrapKey (#12125)
- feat(ext/crypto): support importing raw ECDSA keys (#11871)
- feat(ext/crypto): support importing/exporting raw AES keys (#12392)
- feat(ext/ffi): add support for buffer arguments (#12335)
- feat(ext/ffi): Non-blocking FFI (#12274)
- feat(ext/net): relevant errors for resolveDns (#12370)
- feat(lint): add support for --watch flag (#11983)
- feat(runtime): allow passing extensions via Worker options (#12362)
- feat(runtime): improve error messages of runtime fs (#11984)
- feat(tls): custom in memory CA certificates (#12219)
- feat(unstable/test): imperative test steps API (#12190)
- feat(web): Implement `DOMException`'s `stack` property. (#12294)
- fix: Don't panic when a worker is closed in the reactions to a wasm operation.
  (#12270)
- fix: worker environment permissions should accept an array (#12250)
- fix(core/runtime): sync_ops_cache if nuked Deno ns (#12302)
- fix(ext/crypto): decode id-RSASSA-PSS with default params (#12147)
- fix(ext/crypto): key generation based on AES key length (#12146)
- fix(ext/crypto): missing Aes key typings (#12307)
- fix(ext/crypto): use NotSupportedError for importKey() (#12289)
- fix(ext/fetch): avoid panic when header is invalid (#12244)
- fix(ext/ffi): don't panic in dlopen (#12344)
- fix(ext/ffi): formatting dlopen errors on Windows (#12301)
- fix(ext/ffi): missing "buffer" type definitions (#12371)
- fix(ext/ffi): types for nonblocking FFI (#12345)
- fix(ext/http): merge identical if/else branches (#12269)
- fix(ext/net): should not panic when listening to unix abstract address
  (#12300)
- fix(ext/web): Format DOMException stack property (#12333)
- fix(http): don't expose body on GET/HEAD requests (#12260)
- fix(lsp): lint diagnostics respect config file (#12338)
- fix(repl): avoid panic when assigned to globalThis (#12273)
- fix(runtime): Declare `Window.self` and `DedicatedWorkerGlobalScope.name` with
  `util.writable()` (#12378)
- fix(runtime): don't equate SIGINT to SIGKILL on Windows (#12356)
- fix(runtime): Getting `navigator.hardwareConcurrency` on workers shouldn't
  throw (#12354)
- fix(runtime/js/workers): throw errors instead of using an op (#12249)
- fix(runtime/testing): format aggregate errors (#12183)
- perf(core): use opcall() directly (#12310)
- perf(fetch): fast path Uint8Array in extractBody() (#12351)
- perf(fetch): optimize fillHeaders() key iteration (#12287)
- perf(web): ~400x faster http header trimming (#12277)
- perf(web): optimize byteLowerCase() (#12282)
- perf(web/Event): move last class field to constructor (#12265)
- perf(webidl): fix typo from #12286 (#12336)
- perf(webidl): inline ResponseInit converter (#12285)
- perf(webidl): optimize createDictionaryConverter() (#12279)
- perf(webidl): optimize createRecordConverter() (#12286)
- perf(webidl/DOMString): don't wrap string primitives (#12266)

### 1.14.3 / 2021.10.04

- feat(core): implement Deno.core.isProxy() (#12288)
- fix(core/runtime): sync_ops_cache if nuked Deno ns (#12302)
- fix(ext/crypto): decode id-RSASSA-PSS with default params (#12147)
- fix(ext/crypto): missing Aes key typings (#12307)
- fix(ext/crypto): use NotSupportedError for importKey() (#12289)
- fix(ext/fetch): avoid panic when header is invalid (#12244)
- fix(ext/http): merge identical if/else branches (#12269)
- fix(ext/net): should not panic when listening to unix abstract address
  (#12300)
- fix(repl): avoid panic when assigned to globalThis (#12273)
- fix(runtime/js/workers): throw errors instead of using an op (#12249)
- fix(runtime/testing): format aggregate errors (#12183)
- fix: Don't panic when a worker is closed in the reactions to a wasm operation.
  (#12270)
- fix: worker environment permissions should accept an array (#12250)
- perf(core): use opcall() directly (#12310)
- perf(fetch): optimize fillHeaders() key iteration (#12287)
- perf(web): optimize byteLowerCase() (#12282)
- perf(web): ~400x faster http header trimming (#12277)
- perf(web/Event): move last class field to constructor (#12265)
- perf(webidl): optimize createDictionaryConverter() (#12279)
- perf(webidl): optimize createRecordConverter() (#12286)
- perf(webidl/DOMString): don't wrap string primitives (#12266)

### 1.14.2 / 2021.09.28

- feat(cli/fmt): support more markdown extensions (#12195)
- fix(cli/permissions): ensure revoked permissions are no longer granted
  (#12159)
- fix(ext/http): fortify "is websocket?" check (#12179)
- fix(ext/http): include port number in h2 urls (#12181)
- fix(ext/web): FileReader error messages (#12218)
- fix(ext/webidl): correctly apply [SymbolToStringTag] to interfaces (#11851)
- fix(http): panic when responding to a closed conn (#12216)
- fix(workers): Don't panic when a worker's parent thread stops running (#12156)
- fix: subprocess kill support on windows (#12134)
- perf(ext/fetch): Use the WebIDL conversion to DOMString rather than USVString
  for Response constructor (#12201)
- perf(ext/fetch): skip USVString webidl conv on string constructor (#12168)
- perf(fetch): optimize InnerBody constructor (#12232)
- perf(fetch): optimize newInnerRequest blob url check (#12245)
- perf(fetch/Response): avoid class fields (#12237)
- perf(fetch/headers): optimize appendHeader (#12234)
- perf(ops): optimize permission check (#11800)
- perf(web): optimize Event constructor (#12231)
- perf(webidl/ByteString): 3x faster ASCII check (#12230)
- quickfix(ci): only run "Build product size info" on main/tag (#12184)
- upgrade serde_v8 and rusty_v8 (#12175)

### 1.14.1 / 2021.09.21

- fix(cli): don't ignore diagnostics about for await (#12116)
- fix(cli): move Deno.flock and Deno.funlock to unstable types (#12138)
- fix(cli/fmt_errors): Abbreviate long data URLs in stack traces (#12127)
- fix(config-schema): correct default value of "lib" (#12145)
- fix(core): prevent multiple main module loading (#12128)
- fix(ext/crypto): don't use core.decode for encoding jwk keys (#12088)
- fix(ext/crypto): use DataError in importKey() (#12071)
- fix(lsp): align filter text to vscode logic (#12081)
- fix(runtime/ops/signal.rs): Add FreeBSD signal definitions (#12084)
- perf(ext/web): optimize EventTarget (#12166)
- perf(runtime/fs): optimize readFile by using a single large buffer (#12057)
- perf(web): optimize AbortController (#12165)

### 1.14.0 / 2021.09.14

- BREAKING(unstable): Fix casing in FfiPermissionDescriptor (#11659)
- BREAKING(unstable): Remove Deno.Signals enum, Deno.signals.* (#11909)
- feat(cli): Support Basic authentication in DENO_AUTH_TOKENS (#11910)
- feat(cli): Update to TypeScript 4.4 (#11678)
- feat(cli): add --ignore flag to test command (#11712)
- feat(cli): close test worker once all tests complete (#11727)
- feat(core): facilitate op-disabling middleware (#11858)
- feat(ext/crypto): AES key generation (#11869)
- feat(ext/crypto): export RSA keys as pkcs#8 (#11880)
- feat(ext/crypto): generate ECDH keys (#11870)
- feat(ext/crypto): implement HKDF operations (#11865)
- feat(ext/crypto): implement encrypt, decrypt & generateKey for RSA-OAEP
  (#11654)
- feat(ext/crypto): implement importKey and deriveBits for PBKDF2 (#11642)
- feat(ext/crypto): import RSA pkcs#8 keys (#11891)
- feat(ext/crypto): support JWK export for HMAC (#11864)
- feat(ext/crypto): support JWK import for HMAC (#11716)
- feat(ext/crypto): verify ECDSA signatures (#11739)
- feat(extensions/console): right align numeric columns in table (#11748)
- feat(fetch): mTLS client certificates for fetch() (#11721)
- feat(fmt): add basic JS doc formatting (#11902)
- feat(fmt): add support for configuration file (#11944)
- feat(lint): add support for config file and CLI flags for rules (#11776)
- feat(lsp): ignore specific lint for entire file (#12023)
- feat(unstable): Add file locking APIs (#11746)
- feat(unstable): Support file URLs in Deno.dlopen() (#11658)
- feat(unstable): allow specifying gid and uid for subprocess (#11586)
- feat(workers): Make the `Deno` namespace configurable and unfrozen (#11888)
- feat: ArrayBuffer in structured clone transfer (#11840)
- feat: add URLPattern API (#11941)
- feat: add option flags to 'deno fmt' (#12060)
- feat: stabilise Deno.upgradeWebSocket (#12024)
- fix(cli): better handling of source maps (#11954)
- fix(cli): dispatch unload event on watch drop (#11696)
- fix(cli): retain path based test mode inference (#11878)
- fix(cli): use updated names in deno info help text (#11989)
- fix(doc): fix rustdoc bare_urls warning (#11921)
- fix(ext/crypto): KeyAlgorithm typings for supported algorithms (#11738)
- fix(ext/crypto): add HkdfParams and Pkdf2Params types (#11991)
- fix(ext/fetch): Properly cancel upload stream when aborting (#11966)
- fix(ext/http): resource leak if request body is not consumed (#11955)
- fix(ext/http): websocket upgrade header check (#11830)
- fix(ext/web): Format terminal DOMExceptions properly (#11834)
- fix(ext/web): Preserve stack traces for DOMExceptions (#11959)
- fix(lsp): correctly parse registry patterns (#12063)
- fix(lsp): support data urls in `deno.importMap` option (#11397)
- fix(runtime): return error instead of panicking for windows signals (#11940)
- fix(test): propagate join errors in deno test (#11953)
- fix(typings): fix property name in DiagnosticMessageChain interface (#11821)
- fix(workers): don't drop messages from workers that have already been closed
  (#11913)
- fix: FileReader onevent attributes don't conform to spec (#11908)
- fix: FileReader.readAsText compat (#11814)
- fix: Query string percent-encoded in import map (#11976)
- fix: a `Request` whose URL is a revoked blob URL should still fetch (#11947)
- fix: bring back Deno.Signal to unstable props (#11945)
- fix: change assertion in httpServerIncompleteMessage test (#12052)
- fix: exit process on panic in a tokio task (#11942)
- fix: move unstable declarations to deno.unstable (#11876)
- fix: permission prompt stuffing (#11931)
- fix: permission prompt stuffing on Windows (#11969)
- fix: remove windows-only panic when calling `Deno.kill` (#11948)
- fix: worker_message_before_close was flaky (#12019)
- perf(ext/http): optimize auto cleanup of request resource (#11978)

Release notes for std version 0.107.0:
https://github.com/denoland/deno_std/releases/tag/0.107.0

### 1.13.2 / 2021.08.23

- fix(cli/flags): require a non zero usize for concurrent jobs (#11802)
- fix(ext/crypto): exportKey() for HMAC (#11737)
- fix(ext/crypto): remove duplicate Algorithm interface definition (#11807)
- fix(ext/ffi): don't panic on invalid enum values (#11815)
- fix(ext/http): resource leak on HttpConn.close() (#11805)
- fix(lsp): better handling of languageId (#11755)
- fix(runtime): event loop panics in classic workers (#11756)
- fix(ext/fetch): Headers constructor error message (#11778)
- perf(ext/url): cleanup and optimize url parsing op args (#11763)
- perf(ext/url): optimize UrlParts op serialization (#11765)
- perf(ext/url): use DOMString instead of USVString as webidl converter for URL
  parsing (#11775)
- perf(url): build with opt-level 3 (#11779)

Release notes for std version 0.106.0:
https://github.com/denoland/deno_std/releases/tag/0.106.0

### 1.13.1 / 2021.08.16

- fix: Blob#slice arguments should be optional (#11665)
- fix: correct spelling of certificate in `--unsafely-ignore-certificate-errors`
  warning message (#11634)
- fix: don't statically type name on Deno.errors (#11715)
- fix: parse error when transpiling code with BOM (#11688)
- fix(cli): allow specifiers of unknown media types with test command (#11652)
- fix(cli): explicitly scan for ignore attribute in inline tests (#11647)
- fix(cli): retain input order of remote specifiers (#11700)
- fix(cli/lint): don't use gray in diagnostics output for visibility (#11702)
- fix(cli/tools/repl): don't highlight candidate when completion is list
  (#11697)
- fix(ext/crypto): enable non-extractable keys (#11705)
- fix(ext/crypto): fix copying buffersource (#11714)
- fix(ext/crypto): handle idlValue not being present (#11685)
- fix(ext/crypto): importKey() SecurityError on non-extractable keys (#11662)
- fix(ext/crypto): take a copy of keyData bytes (#11666)
- fix(ext/fetch): better error if no content-type
- fix(ext/fetch): don't use global Deno object
- fix(ext/http): remove unwrap() when HTTP conn errors (#11674)
- fix(ext/web): use Array primordials in MessagePort (#11680)
- fix(http/ws): support multiple options in connection header (#11675)
- fix(lint): add links to help at lint.deno.land (#11667)
- fix(test): dispatch load event before tests are run (#11708)
- fix(test): sort file module specifiers (#11656)
- perf: improve localStorage throughput (#11709)
- perf(ext/http): faster req_url string assembly (#11711)
- perf(wpt/crypto): optimize num-bigint-dig for debug builds (#11681)

Release notes for std version 0.105.0:
https://github.com/denoland/deno_std/releases/tag/0.105.0

### 1.13.0 / 2021.08.10

- BREAKING(unstable): Rename Deno.WebSocketUpgrade::websocket to socket (#11542)
- feat: Add --unsafely-treat-insecure-origin-as-secure flag to disable SSL
  verification (#11324)
- feat: add experimental WebSocketStream API (#10365)
- feat: FFI API replacing native plugins (#11152)
- feat: stabilize Deno.serveHttp() (#11544)
- feat: support AbortSignal in writeFile (#11568)
- feat: support client certificates for connectTls (#11598)
- feat: type check codeblocks in Markdown file with "deno test --doc" (#11421)
- feat(extensions/crypto): implement importKey and exportKey for raw HMAC keys
  (#11367)
- feat(extensions/crypto): implement verify() for HMAC (#11387)
- feat(extensions/tls): Optionally support loading native certs (#11491)
- feat(extensions/web): add structuredClone function (#11572)
- feat(fmt): format top-level JSX elements/fragments with parens when multi-line
  (#11582)
- feat(lsp): ability to set DENO_DIR via settings (#11527)
- feat(lsp): implement refactoring code actions (#11555)
- feat(lsp): support clients which do not support disabled code actions (#11612)
- feat(repl): add --eval flag for evaluating code when the repl starts (#11590)
- feat(repl): support exports in the REPL (#11592)
- feat(runtime): allow URL for permissions (#11578)
- feat(runtime): implement navigator.hardwareConcurrency (#11448)
- feat(unstable): clean environmental variables for subprocess (#11571)
- fix: support windows file specifiers with import maps (#11551)
- fix: Type `Deno.errors.*` as subclasses of `Error` (#10702)
- fix(doc): panic on invalid url (#11536)
- fix(extensions/fetch): Add Origin header to outgoing requests for fetch
  (#11557)
- fix(extensions/websocket): allow any close code for server (#11614)
- fix(lsp): do not output to stderr before exiting the process (#11562)

Release notes for std version 0.104.0:
https://github.com/denoland/deno_std/releases/tag/0.104.0

### 1.12.2 / 2021.07.26

- feat(lsp, unstable): add workspace config to status page (#11459)
- fix: panic for non-WS connections to inspector (#11466)
- fix: support --cert flag for TLS connect APIs (#11484)
- fix(cli): info now displays type reference deps (#11478)
- fix(cli): normalize test command errors (#11375)
- fix(cli): rebuild when environment variables change (#11471)
- fix(cli): side-load test modules (#11515)
- fix(extensions/fetch): close fetch response body on GC (#11467)
- fix(extensions/http): support multiple options in connection header for
  websocket (#11505)
- fix(extensions/websocket): case insensitive connection header (#11489)
- fix(lsp): do not populate maybe_type slot with import type dep (#11477)
- fix(lsp): handle importmaps properly (#11496)

Release notes for std version 0.103.0:
https://github.com/denoland/deno_std/releases/tag/0.103.0

### 1.12.1 / 2021.07.19

- fix: Big{U|}Int64Array in crypto.getRandomValues (#11447)
- fix(extensions/http): correctly concat cookie headers (#11422)
- fix(extensions/web): aborting a FileReader should not affect later reads
  (#11381)
- fix(repl): output error without hanging when input is invalid (#11426)
- fix(tsc): add .at() types manually to tsc (#11443)
- fix(workers): silently ignore non-existent worker IDs (#11417)

Release notes for std version 0.102.0:
https://github.com/denoland/deno_std/releases/tag/0.102.0

### 1.12.0 / 2021.07.13

- feat: Add `MessageChannel` and `MessagePort` APIs (#11051)
- feat: Deno namespace configurable and unfrozen (#11062)
- feat: Enable WebAssembly.instantiateStreaming and WebAssembly.compileStreaming
  (#11200)
- feat: Support "types" option when type checking (#10999)
- feat: Support SharedArrayBuffer sharing between workers (#11040)
- feat: Transfer MessagePort between workers (#11076)
- feat(extensions/crypto): Implement generateKey() and sign() (#9614)
- feat(extensions/crypto): Implement verify() for RSA (#11312)
- feat(extensions/fetch): Add programmatic proxy (#10907)
- feat(extensions/http): Server side websocket support (#10359)
- feat(inspector): Improve inspector prompt in Chrome Devtools (#11187)
- feat(inspector): Pipe console messages between terminal and inspector (#11134)
- feat(lsp): Dependency hover information (#11090)
- feat(repl): Show list completion (#11001)
- feat(repl): Support autocomplete on declarations containing a primitive
  (#11325)
- feat(repl): Support import declarations in the REPL (#11086)
- feat(repl): Type stripping in the REPL (#10934)
- feat(test): Add "--shuffle" flag to randomize test ordering (#11163)
- feat(test): Add support for "--fail-fast=N" (#11316)
- fix: Align DedicatedWorkerGlobalScope event handlers to spec (#11353)
- fix: Move stable/unstable types/APIs to their correct places (#10880)
- fix(core): Fix concurrent loading of dynamic imports (#11089)
- fix(extensions/console): Eliminate panic inspecting event classes (#10979)
- fix(extensions/console): Inspecting prototypes of built-ins with custom
  inspect implementations should not throw (#11308)
- fix(extensions/console): Left align table entries (#11295)
- fix(extensions/crypto): Hash input for RSASSA-PKCS1-v1_5 before signing
  (#11314)
- fix(extensions/fetch): Consumed body with a non-stream source should result in
  a disturbed stream (#11217)
- fix(extensions/fetch): Encode and decode headers as byte strings (#11070)
- fix(extensions/fetch): Filter out custom HOST headers (#11020)
- fix(extensions/fetch): OPTIONS should be allowed a non-null body (#11242)
- fix(extensions/fetch): Proxy body for requests created from other requests
  (#11093)
- fix(extensions/http): Encode and decode headers as byte strings in the HTTP
  server (#11144)
- fix(extensions/http): Panic in request body streaming (#11191)
- fix(extensions/http): Specify AbortSignal for native http requests (#11126)
- fix(extensions/timers): Spec conformance for performance API (#10887)
- fix(extensions/url): Use USVStrings in URLSearchParams constructor (#11101)
- fix(extensions/web): AddEventListenerOptions.signal shouldn't be nullable
  (#11348)
- fix(extensions/webgpu): Align error scopes to spec (#9797)
- fix(lsp): Handle invalid config setting better (#11104)
- fix(lsp): Reload import registries should not error when the module registries
  directory does not exist (#11123)
- fix(repl): Panic when Deno.inspect throws (#11292)
- fix(runtime): Fix signal promise API (#11069)
- fix(runtime): Ignored tests should not cause permission changes (#11278)

Release notes for std version 0.101.0:
https://github.com/denoland/deno_std/releases/tag/0.101.0

### 1.11.3 / 2021.06.29

- fix(#10761): graph errors reported as diagnostics for `Deno.emit()` (#10767)
- fix(core): don't panic on stdout/stderr write failures in Deno.core.print
  (#11039)
- fix(core): top-level-await is now always enabled (#11082)
- fix(extensions/fetch): Filter out custom HOST headers (#11020)
- fix(fetch): proxy body for requests created from other requests (#11093)
- fix(http): remove unwrap() in HTTP bindings (#11130)
- fix(inspect): eliminate panic inspecting event classes (#10979)
- fix(lsp): reload import registries should not error when the module registries
  directory does not exist (#11123)
- fix(runtime): fix signal promise API (#11069)
- fix(runtime/signal): use op_async_unref for op_signal_poll (#11097)
- fix(url): use USVStrings in URLSearchParams constructor (#11101)
- fix(webstorage): increase localStorage limit to 10MB (#11081)
- fix: make readonly `Event` properties readonly (#11106)
- fix: specify AbortSignal for native http requests (#11126)
- chore: upgrade crates (#11007)
- chore: use lsp to get parent process id (#11083)

Release notes for std version 0.100.0:
https://github.com/denoland/deno_std/releases/tag/0.100.0

### 1.11.2 / 2021.06.21

- feat(unstable, lsp): quick fix actions to ignore lint errors (#10627)
- fix: add support for module es2020 to Deno.emit (#11065)
- fix: align Console to spec (#10983)
- fix: align URL / URLSearchParams to spec (#11005)
- fix: align Websocket to spec (#11010)
- fix: closing / aborting WritableStream is racy (#10982)
- fix: fetch with method HEAD should not have body (#11003)
- fix: Worker accepts specifier as URL (#11038)
- fix(lsp): do not rename in strings and comments (#11041)

### 1.11.1 / 2021.06.15

- feat(unstable): add additional logging information in LSP (#10890)
- fix: Deno.inspect should inspect the object the proxy represents rather than
  the target of the proxy (#10977)
- fix: early binding to dispatchEvent in workers (#10904)
- fix: hang in Deno.serveHttp() (#10923)
- fix: improve worker types (#10965)
- fix: make WHATWG streams more compliant (#10967, #10970)
- fix: poll connection after writing response chunk in Deno.serveHttp() (#10961)
- fix: set minimum timeout to be 4 milliseconds (#10972)
- fix(repl): Complete declarations (#10963)
- fix(repl): Fix `undefined` result colour in cmd (#10964)

Release notes for std version 0.99.0:
https://github.com/denoland/deno_std/releases/tag/0.99.0

### 1.11.0 / 2021.06.08

- feat: Add FsWatcher interface (#10798)
- feat: Add origin data dir to deno info (#10589)
- feat: Initialize runtime_compiler ops in `deno compile` (#10052)
- feat: Make 'deno lint' stable (#10851)
- feat: Support data uri dynamic imports in `deno compile` (#9936)
- feat: upgrade to TypeScript 4.3 (#9960)
- feat(extensions): add BroadcastChannel
- feat(extensions/crypto): implement randomUUID (#10848)
- feat(extensions/crypto): implement subtle.digest (#10796)
- feat(extensions/fetch): implement abort (#10863)
- feat(extensions/web): Implement TextDecoderStream and TextEncoderStream
  (#10842)
- feat(lsp): add test code lens (#10874)
- feat(lsp): registry auto discovery (#10813)
- fix: change Crypto to interface (#10853)
- fix: Support the stream option to TextDecoder#decode (#10805)
- fix(extensions/fetch): implement newline normalization and escapes in the
  multipart/form-data serializer (#10832)
- fix(runtime/http): Hang in `Deno.serveHttp` (#10836)
- fix(streams): expose ReadableByteStreamController &
  TransformStreamDefaultController (#10855)

Release notes for std version 0.98.0:
https://github.com/denoland/deno_std/releases/tag/0.98.0

### 1.10.3 / 2021.05.31

- feat(lsp): diagnostics for deno types and triple-slash refs (#10699)
- feat(lsp): provide X-Deno-Warning as a diagnostic (#10680)
- feat(lsp): show hints from `deno_lint` in addition to messages (#10739)
- feat(lsp): support formatting json and markdown files (#10180)
- fix(cli): always allow documentation modules to be checked (#10581)
- fix(cli): canonicalize coverage dir (#10364)
- fix(cli): don't statically error on dynamic unmapped bare specifiers (#10618)
- fix(cli): empty tsconfig.json file does not cause error (#10734)
- fix(cli): support source maps with Deno.emit() and bundle (#10510)
- fix(cli/dts): fix missing error class (NotSupported) in types (#10713)
- fix(cli/install): support `file:` scheme URLs (#10562)
- fix(cli/test): don't use reserved symbol `:` in specifier (#10751)
- fix(cli/test): ensure coverage dir exists (#10717)
- fix(cli/upgrade): modify download size paddings (#10639)
- fix(runtime/http): expose nextRequest() errors in respondWith() (#10384)
- fix(runtime/http): fix empty blob response (#10689)
- fix(serde_v8): remove intentional deserialization error on non-utf8 strings
  (#10156)
- fix(ext/fetch): fix error message of Request constructor (#10772)
- fix(ext/fetch): make prototype properties writable (#10769)
- fix(ext/fetch): remove unimplemented Request attributes (#10784)
- fix(ext/file): update File constructor following the spec (#10760)
- fix(ext/webstorage): use opstate for sqlite connection (#10692)
- fix(lsp): deps diagnostics include data property (#10696)
- fix(lsp): ignore type definition not found diagnostic (#10610)
- fix(lsp): local module import added by code action now includes the file
  extension (#10778)
- fix(lsp): make failed to load config error descriptive (#10685)
- fix(lsp): memoize script versions per tsc request (#10601)
- fix(lsp): re-enable the per resource configuration without a deadlock (#10625)

### 1.10.2 / 2021.05.17

- fix: static import permissions in dynamic imports
- fix(lsp): remove duplicate cwd in config path (#10620)
- fix(cli): ignore x-typescript-types header when media type is not js/jsx
  (#10574)
- chore: upgrade Tokio to 1.6.0 (#10637)

Release notes for std version 0.97.0:
https://github.com/denoland/deno_std/releases/tag/0.97.0

### 1.10.1 / 2021.05.11

- fix(#10603): Disable lsp workspaces, resolve deadlock bug

### 1.10.0 / 2021.05.11

- feat: "deno test" prompts number of tests and origin (#10428)
- feat: "Worker.postMessage()" uses structured clone algorithm (#9323)
- feat: add "deno test --doc" (#10521)
- feat: add "deno test --jobs" (#9815)
- feat: add "deno test --watch" (#9160)
- feat: add test permissions to Deno.test (#10188)
- feat: add WebStorage API (#7819)
- feat: align plugin api with "deno_core::Extension" (#10427)
- feat: support deno-fmt-ignore-file for markdown formatting (#10191)
- feat(core): enable WASM shared memory (#10116)
- feat(core): introduce Extension (#9800)
- feat(lsp): add internal debugging logging (#10438)
- feat(lsp): support workspace folders configuration (#10488)
- fix: invalid types for asynchronous and synchronous `File#truncate` (#10353)
- fix: rename Deno.emit() bundle options to "module" and "classic" (#10332)
- fix: sleepSync doesn't return a Promise (#10358)
- fix: TextEncoder#encodeInto spec compliance (#10129)
- fix: typings for `Deno.os.arch` (#10541)
- fix(extensions/fetch): infinite loop on fill headers (#10406)
- fix(extensions/fetch): Prevent throwing when inspecting a request (#10335)
- fix(installer): allow remote import maps (#10499)
- fix(lsp): remove code_action/diagnostics deadlock (#10555)
- fix(tls): flush send buffer in the background after closing TLS stream
  (#10146)
- fix(tls): throw meaningful error when hostname is invalid (#10387)

Release notes for std version 0.96.0:
https://github.com/denoland/deno_std/releases/tag/0.96.0

### 1.9.2 / 2021.04.23

- fix: parse websocket messages correctly (#10318)
- fix: standalone bin corruption on M1 (#10311)
- fix: don't gray-out internal error frames (#10293)
- fix(op_crates/fetch): Response inspect regression (#10295)
- fix(runtime): do not panic on not found cwd (#10238)
- fix(op_crates/webgpu): move non-null op buffer arg check when needed (#10319)
- fix(lsp): document symbol performance mark (#10264)

Release notes for std version 0.95.0:
https://github.com/denoland/deno_std/releases/tag/0.95.0

### 1.9.1 / 2021.04.20

- feat(lsp, unstable): Implement textDocument/documentSymbol (#9981)
- feat(lsp, unstable): implement textDocument/prepareCallHierarchy (#10061)
- feat(lsp, unstable): Implement textDocument/semanticTokens/full (#10233)
- feat(lsp, unstable): improve diagnostic status page (#10253)
- fix: revert changes to Deno.Conn type (#10255)
- fix(lsp): handle x-typescript-types header on type only imports properly
  (#10261)
- fix(lsp): remove documents when closed (#10254)
- fix(runtime): correct URL in Request (#10256)
- fix(runtime): handle race condition in postMessage where worker has terminated
  (#10239)
- fix(runtime): hang during HTTP server response (#10197)
- fix(runtime): include HTTP ops in WebWorker scope (#10207)

Release notes for std version 0.94.0:
https://github.com/denoland/deno_std/releases/tag/0.94.0

### 1.9.0 / 2021.04.13

- feat: blob URL support (#10045)
- feat: blob URL support in fetch (#10120)
- feat: data URL support in fetch (#10054)
- feat: native HTTP bindings (#9935)
- feat: raise file descriptor limit on startup (#10162)
- feat: set useDefineForClassFields to true (#10119)
- feat: stabilize Deno.ftruncate and Deno.ftruncateSync (#10126)
- feat: stricter typings for Listener & Conn (#10012)
- feat(lsp): add import completions (#9821)
- feat(lsp): add registry import auto-complete (#9934)
- feat(lsp): implement textDocument/foldingRange (#9900)
- feat(lsp): implement textDocument/selectionRange (#9845)
- feat(permissions): allow env permission to take values (#9825)
- feat(permissions): allow run permission to take values (#9833)
- feat(runtime): add stat and statSync methods to Deno.File (#10107)
- feat(runtime): add truncate and truncateSync methods to Deno.File (#10130)
- feat(runtime): stabilize Deno.fstat and Deno.fstatSync (#10108)
- feat(runtime/permissions): prompt fallback (#9376)
- feat(unstable): Add Deno.memoryUsage() (#9986)
- feat(unstable): ALPN config in listenTls (#10065)
- fix: include deno.crypto in "deno types" (#9863)
- fix: Properly await already evaluating dynamic imports (#9984)
- fix(lsp): don't error on tsc debug failures for code actions (#10047)
- fix(lsp): ensure insert_text is passed back on completions (#9951)
- fix(lsp): folding range adjustment panic (#10030)
- fix(lsp): normalize windows file URLs properly (#10034)
- fix(lsp): properly handle encoding URLs from lsp client (#10033)
- fix(op_crates/console): console.table value misalignment with varying keys
  (#10127)
- fix(permissions): don't panic when no input is given (#9894)
- fix(runtime/js/timers): Use (0, eval) instead of eval() (#10103)
- fix(runtime/readFile*): close resources on error during read (#10059)
- fix(websocket): ignore resource close error (#9755)

Release notes for std version 0.93.0:
https://github.com/denoland/deno_std/releases/tag/0.93.0

### 1.8.3 / 2021.04.02

- feat(lsp): add import completions (#9821)
- feat(lsp): implement textDocument/selectionRange (#9845)
- fix(websocket): ignore resource close error (#9755)
- fix(lsp): ensure insert_text is passed back on completions (#9951)
- fix(web): add AbortController.abort() (#9907)
- fix(crypto): include deno.crypto in `deno types` (#9863)
- fix(cli): re-add dom.asynciterable lib (#9888)

Release notes for std version 0.92.0:
https://github.com/denoland/deno_std/releases/tag/0.92.0

### 1.8.2 / 2021.03.21

- fix: fallback to default UA and CA data for Deno.createHttpClient() (#9830)
- fix: getBindGroupLayout always illegal invocation (#9684)
- fix(cli/bundle): display anyhow error chain (#9822)
- fix(core): don't panic on invalid arguments for Deno.core.print (#9834)
- fix(doc): update example for sub processes (#9798)
- fix(fmt): Correctly format hard breaks in markdown (#9742)
- fix(lsp): allow on disk files to change (#9746)
- fix(lsp): diagnostics use own thread and debounce (#9572)
- fix(op_crates/webgpu): create instance only when required (#9771)
- fix(runtime): do not require deno namespace in workers for crypto (#9784)
- refactor: enforce type ResourceId across codebase (#9837, #9832)
- refactor: Clean up permission handling (#9367)
- refactor: Move bin ops to deno_core and unify logic with json ops (#9457)
- refactor: Move Console to op_crates/console (#9770)
- refactor: Split web op crate (#9635)
- refactor: Simplify icu data alignment (#9766)
- refactor: Update minimal ops & rename to buffer ops (#9719)
- refactor: Use serde ops more (#9817, #9828)
- refactor(lsp): refactor completions and add tests (#9789)
- refactor(lsp): slightly reorganize diagnostics debounce logic (#9796)
- upgrade: rusty_v8 0.21.0 (#9725)
- upgrade: tokio 1.4.0 (#9842)

Release notes for std version 0.91.0:
https://github.com/denoland/deno_std/releases/tag/0.91.0

### 1.8.1 / 2021.03.09

- fix(cli/ast): Pass importsNotUsedAsValues to swc (#9714)
- fix(cli/compile): Do not append .exe depending on target (#9668)
- fix(cli/coverage): Ensure single line functions don't yield false positives
  (#9717)
- fix(core): Shared queue assertion failure in case of js error (#9721)
- fix(runtime): Add navigator interface objects (#9685)
- fix(runtime/web_worker): Don't block self.onmessage with TLA (#9619)
- fix(webgpu): Add Uint32Array type for code in ShaderModuleDescriptor (#9730)
- fix(webgpu): Add webidl records and simple unions (#9698)

Release notes for std version 0.90.0:
https://github.com/denoland/deno_std/releases/tag/0.90.0

### 1.8.0 / 2021.03.02

https://deno.land/posts/v1.8

- feat: Align import map to spec and stabilize (#9616, #9526)
- feat: Deno.emit supports bundling as IIFE (#9291)
- feat: Use top user frame for error source lines (#9604)
- feat: WebGPU API (#7977)
- feat: add "deno coverage" subcommand (#8664)
- feat: add --ext flag to deno eval (#9295)
- feat: add exit sanitizer to Deno.test (#9529)
- feat: add json(c) support to deno fmt (#9292)
- feat: add structured cloning to Deno.core (#9458)
- feat: per op metrics (unstable) (#9240)
- feat: represent type dependencies in info (#9630)
- feat: stabilize Deno.permissions (#9573)
- feat: stabilize Deno.link and Deno.linkSync (#9417)
- feat: stabilize Deno.symlink and Deno.symlinkSync (#9226)
- feat: support auth tokens for accessing private modules (#9508)
- feat: support loading import map from URL (#9519)
- feat: use type definitions "deno doc" if available (#8459)
- fix(core): Add stacks for dynamic import resolution errors (#9562)
- fix(core): Fix dynamic imports for already rejected modules (#9559)
- fix(lsp): improve exception handling on tsc snapshots (#9628)
- fix(repl): filter out symbol candidates (#9555)
- fix(runtime): do not panic on irregular dir entries (#9579)
- fix(runtime/testing): false positive for timers when an error is thrown
  (#9553)
- fix(websocket): default to close code 1005 (#9339)
- fix: lint and fmt error if no target files are found (#9527)
- fix: panic caused by Deno.env.set("", "") (#9583)
- fix: typo in coverage exit_unstable (#9626)
- upgrade: TypeScript 4.2 (#9341)
- upgrade: rusty_v8 (V8 9.0.257.3) (#9605)

Release notes for std version 0.89.0:
https://github.com/denoland/deno_std/releases/tag/0.89.0

### 1.7.5 / 2021.02.19

- fix: align btoa to spec (#9053)
- fix: Don't use file names from source maps (#9462)
- fix: Make dynamic import async errors catchable (#9505)
- fix: webidl utils and align `Event` to spec (#9470)
- fix(lsp): document spans use original range (#9525)
- fix(lsp): handle cached type dependencies properly (#9500)
- fix(lsp): handle data URLs properly (#9522)

Release notes for std version 0.88.0:
https://github.com/denoland/deno_std/releases/tag/0.88.0

### 1.7.4 / 2021.02.13

- feat(unstable, lsp): add deno cache code actions (#9471)
- feat(unstable, lsp): add implementations code lens (#9441)
- fix(cli): check for inline source maps before external ones (#9394)
- fix(cli): fix WebSocket close (#8776)
- fix(cli): import maps handles data URLs (#9437)
- fix(console): log function object properties / do not log non-enumerable props
  by default (#9363)
- fix(lsp): handle code lenses for non-documents (#9454)
- fix(lsp): handle type deps properly (#9436)
- fix(lsp): prepare diagnostics when the config changes (#9438)
- fix(lsp): properly handle static assets (#9476)
- fix(lsp): support codeAction/resolve (#9405)
- fix(op_crates): Don't use `Deno.inspect` in op crates (#9332)
- fix(runtime/tls): handle invalid host for connectTls/startTls (#9453)
- upgrade: rusty_v8 0.17.0, v8 9.0.123 (#9413)
- upgrade: deno_doc, deno_lint, dprint, swc_ecmascript, swc_bundler (#9474)

Release notes for std version 0.87.0:
https://github.com/denoland/deno_std/releases/tag/0.87.0

v1.7.3 was released but quickly removed due to bug #9484.

### 1.7.2 / 2021.02.05

- feat(lsp, unstable): add references code lens (#9316)
- feat(lsp, unstable): add TS quick fix code actions (#9396)
- fix: improve http client builder error message (#9380)
- fix(cli): fix handling of non-normalized specifier (#9357)
- fix(cli/coverage): display mapped instrumentation line counts (#9310)
- fix(cli/lsp): fix using jsx/tsx when not emitting via tsc (#9407)
- fix(repl): prevent symbol completion panic (#9400)
- refactor: rewrite Blob implementation (#9309)
- refactor: rewrite File implementation (#9334)

Release notes for std version 0.86.0:
https://github.com/denoland/deno_std/releases/tag/0.86.0

### 1.7.1 / 2021.01.29

- feat(lsp, unstable): add performance measurements (#9209)
- fix(cli): IO resource types, fix concurrent read/write and graceful close
  (#9118)
- fix(cli): Move WorkerOptions::deno types to unstable (#9163)
- fix(cli): add lib dom.asynciterable (#9288)
- fix(cli): correctly determine emit state with redirects (#9287)
- fix(cli): early abort before type checking on missing modules (#9285)
- fix(cli): enable url wpt (#9299)
- fix(cli): fix panic in Deno.emit (#9302)
- fix(cli): fix panic in op_dns_resolve (#9187)
- fix(cli): fix recursive dispatches of unload event (#9207)
- fix(cli): fmt command help message (#9280)
- fix(cli): use DOMException in Performance#measure (#9142)
- fix(cli/flags): don't panic on invalid location scheme (#9202)
- fix(compile): fix panic when cross-compiling between windows and unix (#9203)
- fix(core): Handle prepareStackTrace() throws (#9211)
- fix(coverage): ignore comments (#8639)
- fix(coverage): use source maps when printing pretty reports (#9278)
- fix(lsp): complete list of unused diagnostics (#9274)
- fix(lsp): fix deadlocks, use one big mutex (#9271)
- fix(lsp): handle mbc documents properly (#9151)
- fix(lsp): handle mbc properly when formatting (#9273)
- fix(lsp): reduce deadlocks with in memory documents (#9259)
- fix(op_crates/fetch): fix ReadableStream.pipeThrough() (#9265)
- fix(op_crates/web): Add gb18030 and GBK encodings (#9242)
- fix(op_crates/web): Improve customInspect for Location (#9290)
- chore: new typescript WPT runner (#9269)

Changes in std version 0.85.0:

- feat(std/node): Add support for process.on("exit") (#8940)
- fix(std/async): make pooledMap() errors catchable (#9217)
- fix(std/node): Stop callbacks being called twice when callback throws error
  (#8867)
- fix(std/node): replace uses of `window` with `globalThis` (#9237)

### 1.7.0 / 2021.01.19

- BREAKING(unstable): Use hosts for net allowlists (#8845)
- BREAKING(unstable): remove CreateHttpClientOptions.caFile (#8928)
- feat(installer): Add support for MSYS on Windows (#8932)
- feat(unstable): add Deno.resolveDns API (#8790)
- feat(unstable): runtime compiler APIs consolidated to Deno.emit() (#8799,
  #9139)
- feat: Add WorkerOptions interface to type declarations (#9147)
- feat: Add configurable permissions for Workers (#8215)
- feat: Standalone lite binaries and cross compilation (#9141)
- feat: add --location=<href> and globalThis.location (#7369)
- feat: add global tls session cache (#8877)
- feat: add markdown support to deno fmt (#8887)
- feat: add utf-16 and big5 to TextEncoder/TextDecoder (#8108)
- feat: denort binary (#9041)
- feat: stabilize Deno.shutdown() and Conn#closeWrite()(#9181)
- feat: support data urls (#8866)
- feat: support runtime flags for deno compile (#8738)
- feat: upload release zips to dl.deno.land (#9090)
- fix(cli): dispatch unload on exit (#9088)
- fix(cli): print a newline after help and version (#9158)
- fix(coverage): do not store source inline in raw reports (#9025)
- fix(coverage): merge duplicate reports (#8942)
- fix(coverage): report partial lines as uncovered (#9033)
- fix(inspector): kill child process after test (#8986)
- fix(install): escape % symbols in windows batch files (#9133)
- fix(install): fix cached-only flag (#9169)
- fix(lsp): Add textDocument/implementation (#9071)
- fix(lsp): Respect client capabilities for config and dynamic registration
  (#8865)
- fix(lsp): support specifying a tsconfig file (#8926)
- fix(op_crate/fetch): add back ReadableStream.getIterator and deprecate (#9146)
- fix(op_crate/fetch): align streams to spec (#9103)
- fix(op_crate/web): fix atob to throw spec aligned DOMException (#8798)
- fix(op_crates/fetch): correct regexp for fetch header (#8927)
- fix(op_crates/fetch): req streaming + 0-copy resp streaming (#9036)
- fix(op_crates/web) let TextEncoder#encodeInto accept detached ArrayBuffers
  (#9143)
- fix(op_crates/web): Use WorkerLocation for location in workers (#9084)
- fix(op_crates/websocket): respond to ping with pong (#8974)
- fix(watcher): keep working even when imported file has invalid syntax (#9091)
- fix: Use "none" instead of false to sandbox Workers (#9034)
- fix: Worker hangs when posting "undefined" as message (#8920)
- fix: align DOMException API to the spec and add web platform testing of it.
  (#9106)
- fix: don't error on version and help flag (#9064)
- fix: don't swallow customInspect exceptions (#9095)
- fix: enable WPT tests (#9072, #9087, #9013, #9016, #9047, #9012, #9007, #9004,
  #8990)
- fix: full commit hash in canary compile download (#9166)
- fix: ignore "use asm" (#9019)
- fix: implement DOMException#code (#9015)
- fix: incremental build for deno declaration files (#9138)
- fix: panic during `deno compile` with no args (#9167)
- fix: panic on invalid file:// module specifier (#8964)
- fix: race condition in file watcher (#9105)
- fix: redirect in --location relative fetch (#9150)
- fix: stronger input checking for setTimeout; add function overload (#8957)
- fix: use inline source maps when present in js (#8995)
- fix: use tokio for async fs ops (#9042)
- refactor(cli): remove 'js' module, simplify compiler snapshot (#9020)
- refactor(op_crates/crypto): Prefix ops with "op_crypto_" (#9067)
- refactor(op_crates/websocket): refactor event loop (#9079)
- refactor: Print cause chain when downcasting AnyError fails (#9059)
- refactor: make Process#kill() throw sensible errors on Windows (#9111)
- refactor: move WebSocket API to an op_crate (#9026)
- upgrade: Rust 1.49.0 (#8955)
- upgrade: deno_doc, deno_lint, dprint, swc_ecmascript, swc_bundler (#9003)
- upgrade: deno_lint to 0.2.16 (#9127)
- upgrade: rusty_v8 0.16.0, v8 8.9.255.3 (#9180)
- upgrade: swc_bundler 0.19.2 (#9085)
- upgrade: tokio 1.0 (#8779)

Changes in std version 0.84.0:

- BREAKING(std/wasi): make implementation details private (#8996)
- BREAKING(std/wasi): return exit code from start (#9022)
- feat(std/wasi): allow stdio resources to be specified (#8999)
- fix(std): Don't use JSDoc syntax for browser-compatibility headers (#8960)
- fix(std/http): Use ES private fields in server (#8981)
- fix(std/http): parsing of HTTP version header (#8902)
- fix(std/node): resolve files in symlinked directories (#8840)

### 1.6.3 / 2020.12.30

- feat(lsp): Implement textDocument/rename (#8910)
- feat(lsp): Add cache command (#8911)
- feat(unstable): collect coverage from the run command (#8893)
- fix: fetch bad URL will not panic (#8884)
- fix: info does not panic on missing modules (#8924)
- fix(core): Fix incorrect index in Promise.all error reporting (#8913)
- fix(lsp): handle ts debug errors better (#8914)
- fix(lsp): provide diagnostics for unresolved modules (#8872)
- upgrade: dprint, swc_bundler, swc_common, swc_ecmascript (#8901)
- upgrade: rusty_v8 0.15.0, v8 8.8.294 (#8898)

Changes in std version 0.83.0:

- feat(std/node): adds fs.mkdtemp & fs.mkdtempSync (#8604)
- fix(std/http): Don't expose ServerRequest::done as Deferred (#8919)

### 1.6.2 / 2020.12.22

- feat(lsp): support the unstable setting (#8851)
- feat(unstable): record raw coverage into a directory (#8642)
- feat(unstable): support in memory certificate data for Deno.createHttpClient
  (#8739)
- fix: atomically write files to $DENO_DIR (#8822)
- fix: implement ReadableStream fetch body handling (#8855)
- fix: make DNS resolution async (#8743)
- fix: make dynamic import errors catchable (#8750)
- fix: respect enable flag for requests in lsp (#8850)
- refactor: rename runtime/rt to runtime/js (#8806)
- refactor: rewrite lsp to be async (#8727)
- refactor: rewrite ops to use ResourceTable2 (#8512)
- refactor: optimise static assets in lsp (#8771)
- upgrade TypeScript to 4.1.3 (#8785)

Changes in std version 0.82.0:

- feat(std/node): Added os.type (#8591)

### 1.6.1 / 2020.12.14

- feat(lsp): support import maps (#8683)
- fix: show canary string in long version (#8675)
- fix: zsh completions (#8718)
- fix(compile): error when the output path already exists (#8681)
- fix(lsp): only resolve sources with supported schemas (#8696)
- fix(op_crates/fetch): support non-ascii response headers value (#8600)
- fix(repl): recover from invalid input (#8759)
- refactor: deno_runtime crate (#8640)
- upgrade: swc_ecmascript to 0.15.0 (#8688)

Changes in std version 0.81.0:

- fix(std/datetime): partsToDate (#8553)
- fix(std/wasi): disallow multiple starts (#8712)

### 1.6.0 / 2020.12.08

- BREAKING: Make "isolatedModules" setting non-configurable (#8482)
- feat: Add mvp language server (#8515, #8651)
- feat: deno compile (#8539, #8563, #8581)
- feat: Update to TypeScript 4.1 (#7573)
- feat: EventTarget signal support (#8616)
- feat: Add canary support to upgrade subcommand (#8476)
- feat(unstable): Add cbreak option to Deno.setRaw (#8383)
- fix: "onload" event order (#8376)
- fix: Add file URL support for Deno.readLink (#8423)
- fix: Add hygiene pass to transpile pipeline (#8586)
- fix: Require allow-write permissions for unixpackets datagrams & unix socket
  (#8511)
- fix: Highlight `async` and `of` in REPL (#8569)
- fix: Make output of deno info --json deterministic (#8483)
- fix: Panic in worker when closing at top level (#8510)
- fix: Support passing cli arguments under `deno eval` (#8547)
- fix: `redirect: "manual"` fetch should return `type: "default"` response
  (#8353)
- fix: close() calls sometimes prints results in REPL (#8558)
- fix: watcher doesn't exit when module resolution fails (#8521)
- fix: Fix PermissionDenied error being caught in Websocket constructor (#8402)
- fix: Set User-Agent header in Websocket (#8502, #8470)
- perf: Use minimal op with performance.now() (#8619)
- core: Implement new ResourceTable (#8273)
- core: Add FsModuleLoader that supports loading from filesystem (#8523)
- upgrade rusty_v8 to 0.14.0 (#8663)
- upgrade: deno_doc, deno_lint, dprint, swc (#8552, #8575, #8588)

Changes in std version 0.80.0:

- BREAKING(std/bytes): Adjust APIs based on std-wg discussion (#8612)
- feat(std/encoding/csv): Add stringify functionality (#8408)
- feat(std/fs): Re-enable `followSymlinks` on `walk()` (#8479)
- feat(std/http): Add Cookie value validation (#8471)
- feat(std/node): Add "setImmediate" and "clearImmediate" to global scope
  (#8566)
- feat(std/node): Port most of node errors (#7934)
- feat(std/node/stream): Add Duplex, Transform, Passthrough, pipeline, finished
  and promises (#7940)
- feat(std/wasi): Add return on exit option (#8605)
- feat(std/wasi): Add support for initializing reactors (#8603)
- feat(std/ws): protocol & version support (#8505)
- fix(std/bufio): Remove '\r' at the end of Windows lines (#8447)
- fix(std/encoding): Rewrite toml parser not to use eval() (#8624)
- fix(std/encoding/csv): Correct readme formatting due to dprint issues (#8503)
- fix(std/http): Prevent path traversal (#8474)
- fix(std/node): Inline default objects to ensure correct prototype (#8513)

### 1.5.4 / 2020.11.23

- feat(unstable): Add deno test --no-run (#8093)
- feat(unstable): Support --watch flag for bundle and fmt subcommands (#8276)
- fix: Support "deno run --v8-flags=--help" without script (#8110)
- fix(tsc): Allow non-standard extensions on imports (#8464)
- refactor: Improve Deno.version type declaration (#8391)
- refactor: Rename --failfast to --fail-fast for test subcommand (#8456)
- upgrade: rusty_v8 0.13.0, v8 8.8.278.2 (#8446)

Changes in std version 0.79.0:

- feat(std/hash): Add HmacSha1 (#8418)
- feat(std/http): Check if cookie property is valid (#7189)
- feat(std/http): Validate cookie path value (#8457)
- feat(std/io): ReadableStream from AsyncIterator & WritableStream from Writer
  (#8378)
- feat(std/log): Log error stack (#8401)
- feat(std/node): Add os.totalmem, os.freemem (#8317)
- feat(std/node): Add ReadableStream, WritableStream, errors support (#7569)
- feat(std/node): Add util.deprecate (#8407)
- feat(std/node): Add process.nextTick (#8386)
- fix(std/http): Fix error handling in the request iterator (#8365)
- fix(std/node) Fix event extendability (#8409)
- fix(std/node): Correct typings for global, globalThis, window (#8363)

### 1.5.3 / 2020.11.16

- feat(unstable): support deno lint --rules --json (#8384)
- fix: fix various global objects constructor length (#8373)
- fix: allow declaration emits for Deno.compile() (#8303)
- fix: allow root modules be .mjs/.cjs (#8310)
- fix: allow setting of importsNotUsedAsValues in Deno.compile() (#8306)
- fix: do not write tsbuildinfo when diagnostics are emitted (#8311)
- fix: don't walk the subdirectory twice when using the `--ignore` flag (#8040,
  #8375)
- fix: local sources are not cached in memory (#8328)
- fix: Use safe shell escaping in `deno install` (#7613)
- fix: DOM handler order in Websocket and Worker (#8320, #8334)
- fix(op_crates/web) make isTrusted not constructable (#8337)
- fix(op_crates/web): FileReader event handler order (#8348)
- fix(op_crates/web): handler order when reassign (#8264)
- refactor: deno_crypto op crate (#7956)

Changes in std version 0.78.0:

- feat(std/node): consistent Node.js builtin shapes (#8274)
- fix(std/http): flush body chunks for HTTP chunked encoding (#8349)
- refactor(std/fs): moved isCopyFolder to options (#8319)

### 1.5.2 / 2020.11.09

- fix(core/error): Remove extra newline from JsError::fmt() (#8145)
- fix(op_crates/web): make TextEncoder work with forced non-strings (#8206)
- fix(op_crates/web): fix URLSearchParams, malformed url handling (#8092)
- fix(op_crates/web): define abort event handler on prototype (#8230)
- fix(cli/repl): Fixing syntax highlighting (#8202)
- fix: inject helpers when transpiling via swc (#8221)
- fix: add commit hash and target to long_version output (#8133)
- fix: correct libs sent to tsc for unstable worker (#8260)
- fix: properly handle type checking root modules with type definitions (#8263)
- fix: allow remapping to locals for import map (#8262)
- fix: ensure that transitory dependencies are emitted (#8275)
- fix: make onabort event handler web compatible (#8225)
- fix: display of non-ASCII characters on Windows (#8199)
- refactor: Cleanup Flags to Permissions conversion (#8213)
- refactor: migrate runtime compile/bundle to new infrastructure (#8192)
- refactor: cleanup compiler snapshot and tsc/module_graph (#8220)
- refactor: remove ProgramState::permissions (#8228)
- refactor: refactor file_fetcher (#8245)
- refactor: rewrite permission_test to not depend on Python (#8291)
- refactor: auto detect target triples for upgrade (#8286)
- build: migrate to dlint (#8176)
- build: remove eslint (#8232)
- build: rewrite tools/ scripts to deno (#8247)
- build: full color ci logs (#8280)
- upgrade: TypeScript to 4.0.5 (#8138)
- upgrade: deno_doc, deno_lint, dprint, swc (#8292)

Changes in std version 0.77.0:

- feat(std/node/fs): add realpath and realpathSync (#8169)
- feat(std/wasi): add start method to Context (#8141)
- fix(std/flags): Fix parse incorrectly parsing alias flags with equals (#8216)
- fix(std/node): only define Node.js globals when loading std/node/global
  (#8281)

### 1.5.1 / 2020.10.31

- fix: Accept Windows line breaks in prompt/confirm/alert (#8149)
- fix: Deno.fdata(), Deno.fdatasync() added to stable (#8193)
- fix: Strip "\\?\" prefix when displaying Windows paths (#8135)
- fix: Make hashes of tsconfig deterministic (#8167)
- fix: Module graph handles redirects properly (#8159)
- fix: Restore tripleslash lib refs support (#8157)
- fix: Panic in bundler (#8168)
- fix(repl): Don't hang on unpaired braces (#8151)
- refactor: Don't spin up V8 for `deno cache` (#8186)
- refactor: Create a single watcher for whole process (#8083)
- upgrade: deno_doc, deno_lint, dprint, swc (#8197)

Changes in std version 0.76.0:

- feat(std/node/crypto): Add randomBytes and pbkdf2 (#8191)
- fix(std/wasi): Remove stray console.log call (#8156)

### 1.5.0 / 2020.10.27

- BREAKING: Enable isolatedModules by default (#8050)
- feat(bundle): Add support for --no-check (#8023)
- feat(console): Inspect with colors regardless of Deno.noColor (#7778)
- feat(doc): Support --import-map flag (#7821)
- feat(fmt): Make --ignore flag stable (#7922)
- feat(install): Add missing flags for deno install (#7601)
- feat(repl): Add regex based syntax highlighter (#7811)
- feat(repl): Add tab completion (#7827)
- feat(test): Pass script args to test command (#8121)
- feat(unstable): Add Deno.sleepSync() (#7974)
- feat(unstable): Add Deno.systemCpuInfo() (#7774)
- feat: Add alert, confirm, and prompt (#7507)
- feat: Add types for WeakRef/FinalizationRegistry (#8056)
- feat: Stabilize Deno.fsync and Deno.fdatasync (#8038)
- fix(console): Fix the test cases of function inspections (#7965)
- fix(console): Only inspect getters with option (#7830)
- fix(core): Indicate exceptions in promises (#8124)
- fix(core): Top Level Await module execution (#7946)
- fix(op_crates/fetch): Body.body should be stream of Uint8Array (#8030)
- fix(op_crates/fetch): Ensure Request.method is a string (#8100)
- fix(op_crates/web): Better TextEncoder error message (#8005)
- fix(op_crates/web): Expose event properties in console output (#8103)
- fix(op_crates/web): TextEncoder should throw RangeError (#8039)
- fix(op_crates/web): URL.pathname backslash replacement (#7937)
- fix(repl): Ignore pair matching inside literals (#8037)
- fix(repl): Keyboard interrupt should continue (#7960)
- fix(repl): Unterminated string literal should invalidate (#7896)
- fix(repl): Write all results to stdout (#7893)
- fix(rt/main): Add global interface objects (#7875)
- fix(rt/performance): Check for object props in startOrMeasureOptions (#7884)
- fix(rt/websockets): Only add Sec-WebSocket-Protocol if not empty (#7936)
- fix(test): Return error when awaiting unresolved promise (#7968)
- fix: Do not throw on empty typescript files (#8143)
- fix: Fix inspection of Function (#7930)
- fix: Handle URL paths in Deno.mkdir() (#8140)
- fix: Handling of relative importmaps while using watch (#7950)
- fix: Print error stacks from the origin Worker (#7987)
- fix: Restore permission check on workers (#8123)
- fix: Use -rw-r--r-- for cache files (#8132)
- fix: Use rid getter for stdio (#8014)
- fix: handle roots with extensions that don't match media type (#8114)
- refactor(core): more control over isolate creation (#8000)
- refactor: New TSC infrastructure (#7996, #7981, #7892)
- refactor: Rename --importmap to --import-map (#7032)
- refactor: Rewrite Deno.transpileOnly() to use SWC (#8090)
- upgrade: deno_doc, deno_lint, dprint, swc (#8009, #8077)
- upgrade: rusty_v8 and v8 8.7.220.3 (#8017)

Changes in std version 0.75.0:

- feat(std/fs/node): Add more APIs (#7921)
- feat(std/path): Add toFileUrl() (#7971)
- feat(std/testing): Add assertExists assertion (#7874)
- feat(std/testing): Add assertObjectMatch assertion (#8001)
- fix(std/http): Path traversal in file_server.ts (#8134)
- fix(std/toml): Parsing inline arrays of inline tables (#7902)
- fix(std/encoding): base64 properly encodes mbc and handles Uint8Arrays (#7807)
- fix(std/http/file_server): File server should ignore query params (#8116)
- fix(std/node): Buffer.copy doesn't work as expected (#8125)
- fix(std/wasi): Disallow path_open outside of pre-opened dirfd (#8078)
- refactor(std/testing): Rename assert_Contains to assert_Includes (#7951)

### 1.4.6 / 2020.10.10

- fix: 100% CPU idling problem by reverting #7672 (#7911)
- fix(op_crate/web): add padding on URLSearchParam (#7905)
- fix(op_crates/fetch): Stringify and parse Request URLs (#7838)
- refactor(core): Implement Serialize for ModuleSpecifier (#7900)
- upgrade: Rust 1.47.0 (#7886)

### 1.4.5 / 2020.10.08

- feat(unstable): Revert "enable importsNotUsedAsValues by default #7413"
  (#7800)
- fix: Update worker types to better align to lib.dom.d.ts (#7843)
- fix(cli/ops/fs): Preserve Windows path separators in Deno.realPath() (#7833)
- fix(cli/rt/console): Don't require a prototype to detect a class instance
  (#7869)
- fix(cli/rt/error_stack): Improve message line formatting (#7860)
- fix(core): Handle unregistered errors in core better (#7817)
- fix(core): Module execution with top level await (#7672)
- perf(cli/console): Don't add redundant ANSI codes (#7823)
- refactor(cli): Remove TextDocument (#7850)
- refactor(cli/inspector): Use &str for post_message (#7851)
- refactor(cli/repl): Tightly integrate event loop (#7834)
- refactor(core): Cleanup JsRuntime (#7853, #7855, #7825, #7846)
- upgrade: deno_doc, deno_lint, dprint, swc (#7862)
- upgrade: rusty_v8 0.11.0, V8 8.7.220.3 (#7859)

Changes in std version 0.74.0:

- chore(std/http): Rename http_bench.ts -> bench.ts (#7509)
- feat(std/node/fs): Adding readdir, rename, and some others (#7666)
- fix(std/node/fs): Allow appendFileSync to accept Uint8Array as type for data
  (#7835)

### 1.4.4 / 2020.10.03

- fix(cli): Update type definitions to align to TS dom (#7791)
- fix(cli/repl): Fix hot loop in REPL (#7804)
- fix(cli/repl): Enable colors on inspected values (#7798)

### 1.4.3 / 2020.10.02

- feat(unstable): Add module specifier to deno info --json output (#7725)
- fix: Bundle loader returns exported value (#7764)
- fix: Check cached versions during transpile (#7760)
- fix: Net listen crashes on explicit undefined hostname (#7706)
- fix: --no-check recognizes require (#7720)
- fix: Use $deno$test.ts instead of .deno.test.ts (#7717)
- fix: Use global_state file_fetcher when using SpecifierHandler (#7748)
- fix(console): Catch and format getter errors (#7766)
- fix(dts): Use var instead of const and let for globals (#7680)
- fix(inspector): Shutdown server gracefully on drop (#7716)
- fix(repl): Enable await and let re-declarations (#7784)
- fix(repl): Use a default referrer when empty (#7794)
- fix(test): Do not start inspector server when collecting coverage (#7718)
- fix(websocket): Add missing close events and remove extra error event (#7606)
- refactor: Add concept of 'legacy' compiler to enable non-breaking refactoring
  (#7762)
- refactor: Combine MainWorker::new and MainWorker::create (#7693)
- refactor: Extract inspector session (#7756, #7763)
- refactor: Factor out check_unstable op helper (#7695)
- refactor: Improve graph and tsc_config (#7747)
- refactor: Improve op crate interfaces for other consumers (#7745)
- refactor: Move op state registration to workers (#7696)
- refactor: Use JsRuntime to implement TSC (#7691)
- refactor: Add Deno.InspectOptions::colors (#7742)
- upgrade: swc, deno_doc, deno_lint, dprint (#7711, #7793)

Changes in std version 0.72.0:

- BREAKING(std/encoding/csv): Improve the definition of ParseOptions (#7714)
- feat(std/path): Align globToRegExp() with bash glob expansion (#7209)
- fix(std/datetime): Add timezone to date strings in tests (#7675)
- refactor(std/example): Inconsistencies in the example tests (#7684)
- refactor(std/testing): Get rid of default export and make std/testing/diff.ts
  private (#7592)

### 1.4.2 / 2020.09.25

- fix: Better formatting in console (#7642, #7641, #7553)
- fix: Change log level to which prefix added (#7582)
- fix: Change the Console class declaration to an interface (#7646)
- fix: Clearing timers race condition (#7617)
- fix: customInspect works on functions (#7670)
- fix: Ignore fileExists in tsc host (#7635)
- fix: Make --unstable a global flag (#7585)
- fix: Make --watch and --inspect conflicting args (#7610)
- fix: Make some web API constructors illegal at runtime (#7468)
- fix: Replaced legacy chrome-devtools:// scheme. (#7659)
- fix: Response.arrayBuffer() doesn't return promise (#7618)
- fix: Update supported text encodings (#7668)
- fix: Use class instead of var+interface in d.ts #7514
- fix(coverage): print lines with no coverage to stdout (#7640)
- fix(fmt,lint): do not print number of checked files when `--quiet` is enabled
  (#7579)
- fix(info): add --importmap flag (#7424)
- fix(installer): Don't reload by default (#7596)
- fix(repl): interpret object literals as expressions (#7591)
- fix(watch): watch importmap file for changes (#7580)
- refactor(core): support error stack, remove js_check (#7629, #7636)
- refactor(coverage): Harden coverage collection (#7584, #7616, #7577)
- upgrade: TypeScript to 4.0.3 (#7637)
- example(core): Add hello world example (#7611)

Changes in std version 0.71.0:

- feat(std/node): implement getSystemErrorName() (#7624)
- fix(std/datetime): 12 and 24 support (#7661)
- fix(std/fs): mark createWalkEntry(Sync) as internal (#7643)
- chore(std/hash): update crates (#7631)

### 1.4.1 / 2020.09.18

- fix(cli/console): escape special characters in strings and property names
  (#7546, #7533, #7550)
- fix(cli/fmt): canonicalize files in current dir (#7508)
- fix(cli/fmt): make fmt output more readable (#7534)
- fix(cli/install): revert "bundle before installation" (#7522)
- fix(cli/js): disable URL.createObjectUrl (#7543)
- fix(cli/js): use Buffer.writeSync in MultipartBuilder (#7542)
- fix(cli/repl): disable rustyline logs (#7535)
- fix(cli/repl): format evaluation results with the object specifier (#7561)
- fix(cli/bundle,eval,repl): add missing flags (#7414)
- refactor(cli): move fetch() implementation to op_crates/fetch (#7524, #7529)
- refactor(cli): move FileReader and URL to op_crates/web (#7554, #7544)
- refactor(cli): move op_resources and op_close to deno_core (#7539)
- refactor(cli/info,unstable): deno info --json output (#7417)
- refactor(cli/js): simplify global properties (#7502)
- refactor(cli/js): use Symbol.for instead of Symbol (#7537)
- refactor(core): remove JsRuntime::set_js_error_create_fn (#7478)
- refactor(core): use the 'anyhow' crate instead of ErrBox (#7476)
- upgrade: rust crates (#7454)
- benchmark: add no_check_hello benchmark (#7458)

Changes in std version 0.70.0:

- feat(std/node): add AssertionError class (#7210)
- fix(std/datetime): timezone bug (#7466)
- fix(std/testing): assertion diff color (#7499)

### 1.4.0 / 2020.09.13

- feat: Implement WebSocket API (#7051, #7437)
- feat(console): print proxy details (#7139)
- feat(console): support CSS styling with "%c" (#7357)
- feat(core): Add JSON ops (#7336)
- feat(fmt, lint): show number of checked files (#7312)
- feat(info): Dependency count and sizes (#6786, #7439)
- feat(install): bundle before installation (#5276)
- feat(op_crates/web): Add all single byte encodings to TextDecoder (#6178)
- feat(unstable): Add Deno.systemMemoryInfo() (#7350)
- feat(unstable): deno run --watch (#7382)
- feat(unstable): deno test --coverage (#6901)
- feat(unstable): enable importsNotUsedAsValues by default (#7413)
- feat(unstable): enable isolatedModules by default (#7327)
- fix: Empty Response body returns 0-byte array (#7387)
- fix: panic on process.kill() after run (#7405)
- fix: colors mismatch (#7367)
- fix: compiler config resolution using relative paths (#7392)
- fix(core): panic on big string allocation (#7395)
- fix(op_crates/web): Use "deno:" URLs for internal script specifiers (#7383)
- refactor: Improve placeholder module names (#7430)
- refactor: improve tsc diagnostics (#7420)
- refactor(core): merge CoreIsolate and EsIsolate into JsRuntime (#7370, #7373,
  #7415)
- refactor(core): Use gotham-like state for ops (#7385)
- upgrade: deno_doc, deno_lint, dprint, swc (#7381, #7391, #7402, #7434)
- upgrade: rusty_v8 0.10.0 / V8 8.7.75 (#7429)

Changes in std version 0.69.0:

- BREAKING(std/fs): remove writeJson and writeJsonSync (#7256)
- BREAKING(std/fs): remove readJson and readJsonSync (#7255)
- BREAKING(std/ws): remove connect method (#7403)

### 1.3.3 / 2020.09.04

- feat(unstable): Add Deno.futime and Deno.futimeSync (#7266)
- feat(unstable): Allow deno lint to read from stdin (#7263)
- fix: Don't expose globalThis.__bootstrap (#7344)
- fix: Handle bad redirects more gracefully (#7342)
- fix: Handling of + character in URLSearchParams (#7314)
- fix: Regex for TS references and deno-types (#7333)
- fix: Set maximum size of thread pool to 31 (#7290)
- fix: Support missing features in --no-check (#7289)
- fix: Use millisecond precision for Deno.futime and Deno.utime (#7299)
- fix: Use upstream type definitions for WebAssembly (#7216)
- refactor: Compiler config in Rust (#7228)
- refactor: Support env_logger / RUST_LOG (#7142)
- refactor: Support multiline diagnostics in linter (#7303)
- refactor: Use dependency analyzer from SWC (#7334)
- upgrade: rust 1.46.0 (#7251)
- upgrade: swc, deno_doc, deno_lint, dprint (#7276, #7332)

Changes in std version 0.68.0:

- refactor(std/uuid): remove dependency on isString from std/node (#7273)

### 1.3.2 / 2020.08.29

- fix(cli): revert "never type check deno info #6978" (#7199)
- fix(console): handle escape sequences when logging objects (#7171)
- fix(doc): stack overflow for .d.ts files (#7167)
- fix(install): Strip "@..." suffixes from inferred names (#7223)
- fix(lint): use recommended rules set (#7222)
- fix(url): Add missing part assignment (#7239)
- fix(url): Don't encode "'" in non-special query strings (#7152)
- fix(web): throw TypeError on invalid input types in TextDecoder.decode()
  (#7179)
- build: Move benchmarks to Rust (#7134)
- upgrade: swc, dprint, deno_lint, deno_doc (#7162, #7194)
- upgrade: rusty_v8 0.9.1 / V8 8.6.334 (#7243)
- upgrade: TypeScript 4.0 (#6514)

Changes in std version 0.67.0:

- BREAKING(std/wasi): rename Module to Context (#7110)
- BREAKING(std/wasi): use record for exports (#7109)
- feat(std/fmt): add bright color variations (#7241)
- feat(std/node): add URL export (#7132)
- feat(std/testing): add assertNotMatch (#6775)
- fix(std/encoding/toml): Comment after arrays causing incorrect output (#7224)
- fix(std/node): "events" and "util" modules (#7170)
- fix(std/testing): invalid dates assertion equality (#7230)
- fix(std/wasi): always capture syscall exceptions (#7116)
- fix(std/wasi): ignore lint errors (#7197)
- fix(std/wasi): invalid number to bigint conversion in fd_tell (#7215)
- fix(std/wasi): return flags from fd_fdstat_get (#7112)

### 1.3.1 / 2020.08.21

- fix: Allow isolated "%"s when parsing file URLs (#7108)
- fix: Blob.arrayBuffer returns Uint8Array (#7086)
- fix: CLI argument parsing with dash values (#7039)
- fix: Create Body stream from any valid bodySource (#7128)
- fix: Granular permission requests/revokes (#7074)
- fix: Handling of multiple spaces in URLSearchParams (#7068)
- core: Enable WebAssembly.instantiateStreaming (#7043)
- core: Add missing export of HeapLimits (#7047)
- upgrade: swc_ecmascript, deno_lint, dprint (#7098)

Changes in std version 0.66.0:

- BREAKING(std/datetime): Remove currentDayOfYear (#7059)
- feat(std/node): Add basic asserts (#7091)
- feat(std/datetime): Generalise parser, add formatter (#6619)
- fix(std/node): Misnamed assert exports (#7123)
- fix(std/encoding/toml): Stop TOML parser from detecting numbers in strings.
  (#7064)
- fix(std/encoding/csv): Improve error message on ParseError (#7057)

### 1.3.0 / 2020.08.13

Changes in the CLI:

- feat: Add "--no-check" flag to deno install (#6948)
- feat: Add "--ignore" flag to deno lint (#6934)
- feat: Add "--json" flag to deno lint (#6940)
- feat: Add "--reload" flag to deno bundle (#6996)
- feat: Add "--reload" flag to deno info (#7009)
- feat: FileReader API (#6673)
- feat: Handle imports in deno doc (#6987)
- feat: Stabilize Deno.mainModule (#6993)
- feat: Support file URLs in Deno.run for executable (#6994)
- fix: console.log should see color codes when grouping occurs (#7000)
- fix: URLSearchParams.toString() behaviour is different from browsers (#7017)
- fix: Remove @ts-expect-error directives (#7024)
- fix(unstable): Add missing globals to diagnostics (#6988)
- refactor(doc): Remove detailed / summary distinction (#6818)
- core: Memory limits & callbacks (#6914)
- upgrade: TypeScript to 3.9.7 (#7036)
- upgrade: Rust crates (#7034, #7040)

Changes in std version 0.65.0:

- feat(std/http): Add TLS serve abilities to file_server (#6962)
- feat(std/http): Add --no-dir-listing flag to file_server (#6808)
- feat(std/node): Add util.inspect (#6833)
- fix: Make std work with isolatedModules (#7016)

### 1.2.3 / 2020.08.08

Changes in the CLI:

- fix: Never type check in deno info (#6978)
- fix: add missing globals to unstable diagnostics (#6960)
- fix: add support for non-UTF8 source files (#6789)
- fix: hash file names in gen cache (#6911)
- refactor: Encode op errors as strings instead of numbers (#6977)
- refactor: Op crate for Web APIs (#6906)
- refactor: remove repeated code in main.rs (#6954)
- upgrade to rusty_v8 0.8.1 / V8 8.6.334 (#6980)
- upgrade: deno_lint v0.1.21 (#6985)
- upgrade: swc_ecmascript (#6943)
- feat(unstable): custom http client for fetch (#6918)

Changes in std version 0.64.0:

- fix(std/toml): parser error with inline comments (#6942)
- fix(std/encoding/toml): Add boolean support to stringify (#6941)
- refactor: Rewrite globToRegExp() (#6963)

### 1.2.2 / 2020.07.31

Changes in the CLI:

- fix: Change release build flags to optimize for size (#6907)
- fix: Fix file URL to path conversion on Windows (#6920)
- fix: deno-types, X-TypeScript-Types precedence (#6761)
- fix: downcast from SwcDiagnosticBuffer to OpError (#6909)
- perf: Use SWC to strip types for "--no-check" flag (#6895)
- upgrade: deno_lint, dprint, swc (#6928, #6869)
- feat(unstable): add "--ignore" flag to deno fmt (#6890)

Changes in std version 0.63.0:

- feat(std/async): add pooledMap utility (#6898)
- fix(std/json): Add newline at the end of json files (#6885)
- fix(std/path): Percent-decode in fromFileUrl() (#6913)
- fix(std/tar): directory type bug (#6905)

### 1.2.1 / 2020.07.23

Changes in the CLI:

- fix: IPv6 hostname should be compressed (#6772)
- fix: Ignore polling errors caused by return() in watchFs (#6785)
- fix: Improve URL compatibility (#6807)
- fix: ModuleSpecifier removes relative path parts (#6762)
- fix: Share reqwest client between fetch calls (#6792)
- fix: add icon and metadata to deno.exe on Windows (#6693)
- fix: panic for runtime error in TS compiler (#6758)
- fix: providing empty source code for missing compiled files (#6760)
- refactor: Make OpDispatcher a trait (#6736, #6742)
- refactor: Remove duplicate code and allow filename overwrite for DomFile
  (#6817, #6830)
- upgrade: Rust 1.45.0 (#6791)
- upgrade: rusty_v8 0.7.0 (#6801)
- upgrade: tokio 0.2.22 (#6838)

Changes in std version 0.62.0:

- BREAKING(std/fs): remove readFileStr and writeFileStr (#6848, #6847)
- feat(std/encoding): add ascii85 module (#6711)
- feat(std/node): add string_decoder (#6638)
- fix(std/encoding/toml): could not parse strings with apostrophes/semicolons
  (#6781)
- fix(std/testing): assertThrows inheritance (#6623)
- fix(std/wasi): remove number overload from rights in path_open (#6768)
- refactor(std/datetime): improve weekOfYear (#6741)
- refactor(std/path): enrich the types in parse_format_test (#6803)

### 1.2.0 / 2020.07.13

Changes in the CLI:

- feat(cli): Add --cert option to "deno upgrade" (#6609)
- feat(cli): Add --config flag to "deno install" (#6204)
- feat(cli): Add --json option to "deno info" (#6372)
- feat(cli): Add --no-check option (#6456)
- feat(cli): Add --output option to "deno upgrade" (#6352)
- feat(cli): Add DENO_CERT environment variable (#6370)
- feat(cli): Add lockfile support to bundle (#6624)
- feat(cli/js): Add WriteFileOptions to writeTextFile & writeTextFileSync
  (#6280)
- feat(cli/js): Add copy argument to Buffer.bytes (#6697)
- feat(cli/js): Add performance user timing APIs (#6421)
- feat(cli/js): Add sorted, trailingComma, compact and iterableLimit to
  InspectOptions (#6591)
- feat(cli/js): Deno.chown() make uid, gid args optional (#4612)
- feat(doc): Improve terminal printer (#6594)
- feat(test): Add support for regex in filter flag (#6343)
- feat(unstable): Add Deno.consoleSize() (#6520)
- feat(unstable): Add Deno.ppid (#6539, #6717)
- fix(cli): Don't panic when no "HOME" env var is set (#6728)
- fix(cli): Harden pragma and reference parsing in module analysis (#6702)
- fix(cli): Panic when stdio is null on windows (#6528)
- fix(cli): Parsing of --allow-net flag (#6698)
- fix(cli/js): Allow Buffer to store MAX_SIZE bytes (#6570)
- fix(cli/js): Definition of URL constructor (#6653)
- fix(cli/js): Deno.setRaw shouldn't panic on ENOTTY (#6630)
- fix(cli/js): Fix process socket types (#6676)
- fix(cli/js): Fix relative redirect in fetch API (#6715)
- fix(cli/js): Implement IPv4 hostname parsing in URL (#6707)
- fix(cli/js): Implement spec-compliant host parsing for URL (#6689)
- fix(cli/js): Response constructor default properties in fetch API (#6650)
- fix(cli/js): Update timers to ignore Date Override (#6552)
- perf(cli): Improve .arrayBuffer() speed in fetch API (#6669)
- refactor(core): Remove control slice from ops (#6048)

Changes in std version 0.61.0:

- BREAKING(std/encoding/hex): Simplify API (#6690)
- feat(std/datetime): Add weekOfYear (#6659)
- feat(std/log): Expose Logger type and improve public interface for get & set
  log levels (#6617)
- feat(std/node): Add buf.equals() (#6640)
- feat(std/wasi): Implement fd_readdir (#6631)
- fix(std): base64 in workers (#6681)
- fix(std): md5 in workers (#6662)
- fix(std/http): Properly return port 80 in \_parseAddrFromStr (#6635)
- fix(std/mime): Boundary random hex values (#6646)
- fix(std/node): Add encoding argument to Buffer.byteLength (#6639)
- fix(std/testing/asserts): AssertEquals/NotEquals should use milliseconds in
  Date (#6644)
- fix(std/wasi): Return errno::success from fd_tell (#6636)

### 1.1.3 / 2020.07.03

Changes in the CLI:

- fix(cli): Change seek offset type from i32 to i64 (#6518)
- fix(cli/body): Maximum call stack size exceeded error (#6537)
- fix(cli/doc): Doc printer missing [] around tuple type (#6523)
- fix(cli/js): Buffer.bytes() ArrayBuffer size (#6511)
- fix(cli/js): Fix conditional types for process sockets (#6275)
- fix(cli/upgrade): Upgrade fails on Windows with space in temp path (#6522)
- fix: Lock file for dynamic imports (#6569)
- fix: Move ImportMeta to deno.ns lib (#6588)
- fix: Net permissions didn't account for default ports (#6606)
- refactor: Improvements to TsCompiler and its tests (#6576)
- upgrade: deno_lint 0.1.15 (#6580, #6614)
- upgrade: dprint-plugin-typescript 0.19.5 (#6527, #6614)

Changes in std version 0.60.0:

- feat(std/asserts): Allow assert functions to specify type parameter (#6413)
- feat(std/datetime): Add is leap and difference functions (#4857)
- feat(std/io): Add fromStreamReader, fromStreamWriter (#5789, #6535)
- feat(std/node): Add Buffer.allocUnsafe (#6533)
- feat(std/node): Add Buffer.isEncoding (#6521)
- feat(std/node): Support hex/base64 encoding in fs.readFile/fs.writeFile
  (#6512)
- feat(std/wasi) Implement fd_filestat_get (#6555)
- feat(std/wasi) Implement fd_filestat_set_size (#6558)
- feat(std/wasi): Implement fd_datasync (#6556)
- feat(std/wasi): Implement fd_sync (#6560)
- fix(std/http): Catch errors on file_server response.send (#6285)
- fix(std/http): Support ipv6 parsing (#5263)
- fix(std/log): Print "{msg}" when log an empty line (#6381)
- fix(std/node): Add fill & encoding args to Buffer.alloc (#6526)
- fix(std/node): Do not use absolute urls (#6562)
- fix(std/wasi): path_filestat_get padding (#6509)
- fix(std/wasi): Use lookupflags for path_filestat_get (#6530)
- refactor(std/http): Cookie types to not require full ServerRequest object
  (#6577)

### 1.1.2 / 2020.06.26

Changes in the CLI:

- fix(web/console): Improve string quoting behaviour (#6457)
- fix(web/url): Support UNC paths on Windows (#6418)
- fix(web/url): Support URLSearchParam as Body (#6416)
- fix: 'Compile' messages changed to 'Check' messages (#6504)
- fix: Panic when process stdio rid is 0 or invalid (#6405)
- fix: enable experimental-wasm-bigint (#6443)
- fix: ipv6 parsing for --allow-net params (#6453, #6472)
- fix: panic when demanding permissions for hostless URLs (#6500)
- fix: strings shouldn't be interpreted as file URLs (#6412)
- refactor: Add ability to output compiler performance information (#6434)
- refactor: Incremental compilation for TypeScript (#6428, #6489)
- upgrade: rusty_v8 0.4.2 / V8 8.5.216 (#6503)

Changes in unstable APIs:

- Add Deno.fdatasyncSync and fdatasync (#6403)
- Add Deno.fstatSync and fstat (#6425)
- Add Deno.fsyncSync and fsync (#6411)
- Add Deno.ftruncate and ftruncateSync (#6243)
- Remove Deno.dir (#6385)

Changes in std version 0.59.0:

- BREAKING(std/encoding/hex): reorder encode & decode arguments (#6410)
- feat(std/node): support hex / base64 encoding in Buffer (#6414)
- feat(std/wasi): add wasi_snapshot_preview1 (#6441)
- fix(std/io): Make BufWriter/BufWriterSync.flush write all chunks (#6269)
- fix(std/node): fix readFile types, add encoding types (#6451)
- fix(std/node): global process should usable (#6392)
- fix(std/node/process): env, argv exports (#6455)
- fix(std/testing) assertArrayContains should work with any array-like (#6402)
- fix(std/testing): assertThrows gracefully fails if non-Error thrown (#6330)
- refactor(std/testing): Remove unuseful statement (#6486)
- refactor: shift copyBytes and tweak deps to reduce dependencies (#6469)

### 1.1.1 / 2020.06.19

- fix: "deno test" should respect NO_COLOR=true (#6371)
- fix: Deno.bundle supports targets < ES2017 (#6346)
- fix: decode path properly on win32 (#6351)
- fix: improve failure message for deno upgrade (#6348)
- fix: apply http redirection limit for cached files (#6308)
- fix: JSX compilation bug and provide better error message (#6300)
- fix: DatagramConn.send (unstable) should return bytes sent (#6265, #6291)
- upgrade: v8 to 8.5.104, rusty_v8 0.5.1 (#6377)
- upgrade: crates (#6378)

Changes in std version 0.58.0:

- feat(std/log): expose logger name to LogRecord (#6316)
- fix(std/async): MuxAsyncIterator throws muxed errors (#6295)
- fix(std/io): BufWriter/StringWriter bug (#6247)
- fix(std/io): Use Deno.test in writers_test (#6273)
- fix(std/node): added tests for static methods of Buffer (#6276)
- fix(std/testing): assertEqual so that it handles URL objects (#6278)
- perf(std/hash): reimplement all hashes in WASM (#6292)

### 1.1.0 / 2020.06.12

Changes in the CLI:

- feat: "deno eval -p" (#5682)
- feat: "deno lint" subcommand (#6125, #6208, #6222, #6248, #6258, #6264)
- feat: Add Deno.mainModule (#6180)
- feat: Add Deno.env.delete() (#5859)
- feat: Add TestDefinition::only (#5793)
- feat: Allow reading the entry file from stdin (#6130)
- feat: Handle .mjs files in "deno test" and "deno fmt" (#6134, #6122)
- feat: URL support in Deno filesystem methods (#5990)
- feat: make rid on Deno.Listener public (#5571)
- feat(core): Add unregister op (#6214)
- feat(doc): Display all overloads in cli details view (#6186)
- feat(doc): Handle detail output for enum (#6078)
- feat(fmt): Add diff for "deno fmt --check" (#5599)
- fix: Handle @deno-types in export {} (#6202)
- fix: Several regressions in TS compiler (#6177)
- fix(cli): 'deno upgrade' doesn't work on Windows 8.1/PowerShell 4.0 (#6132)
- fix(cli): WebAssembly runtime error propagation (#6137)
- fix(cli/js/buffer): Remove try-catch from Buffer.readFrom, readFromSync
  (#6161)
- fix(cli/js/io): Deno.readSync on stdin (#6126)
- fix(cli/js/net): UDP BorrowMutError (#6221)
- fix(cli/js/process): Always return a code in ProcessStatus (#5244)
- fix(cli/js/process): Strengthen socket types based on pipes (#4836)
- fix(cli/js/web): IPv6 hostname support in URL (#5766)
- fix(cli/js/web/worker): Disable relative module specifiers (#5266)
- fix(cli/web/fetch): multipart/form-data request body support for binary files
  (#5886)
- fix(core): ES module snapshots (#6111)
- revert: "feat: format deno bundle output (#5139)" (#6085)
- upgrade: Rust 1.44.0 (#6113)
- upgrade: swc_ecma_parser 0.24.5 (#6077)

Changes in std version 0.57.0:

- feat(std/encoding/binary): Add varnumBytes(), varbigBytes() (#5173)
- feat(std/hash): Add sha3 (#5558)
- feat(std/log): Inline and deferred statement resolution logging (#5192)
- feat(std/node): Add util.promisify (#5540)
- feat(std/node): Add util.types (#6159)
- feat(std/node): Buffer (#5925)
- feat(std/testing): Allow non-void promises in assertThrowsAsync (#6052)
- fix(http/server): Flaky test on Windows (#6188)
- fix(std/archive): Untar (#6217) cleanup std/tar (#6185)
- fix(std/http): Don't use assert() for user input validation (#6092)
- fix(std/http): Prevent crash on UnexpectedEof and InvalidData (#6155)
- fix(std/http/file_server): Args handling only if invoked directly (#5989)
- fix(std/io): StringReader implementation (#6148)
- fix(std/log): Revert setInterval log flushing as it prevents process
  completion (#6127)
- fix(std/node): Emitter.removeAllListeners (#5583)
- fix(std/testing/bench): Make progress callback async (#6175)
- fix(std/testing/bench): Clock assertions without --allow-hrtime (#6069)
- refactor(std): Remove testing dependencies from non-test code (#5838)
- refactor(std/http): Rename delCookie to deleteCookie (#6088)
- refactor(std/testing): Rename abbreviated assertions (#6118)
- refactor(std/testing/bench): Remove differentiating on runs count (#6084)

### 1.0.5 / 2020.06.03

Changes in the CLI:

- fix(fetch): Support 101 status code (#6059)
- fix: REPL BorrowMutError panic (#6055)
- fix: dynamic import BorrowMutError (#6065)
- upgrade: dprint 0.19.1 and swc_ecma_parser 0.24.3 (#6068)
- upgrade: rusty_v8 0.5.0 (#6070)

Changes in std version 0.56.0:

- feat(std/testing): benching progress callback (#5941)
- feat(std/encoding): add base64url module (#5976)
- fix(std/testing/asserts): Format values in assertArrayContains() (#6060)

### 1.0.4 / 2020.06.02

Changes in the CLI:

- feat(core): Ops can take several zero copy buffers (#4788)
- fix(bundle): better size output (#5997)
- fix(cli): Deno.remove() fails to remove unix socket (#5967)
- fix(cli): compile TS dependencies of JS files (#6000)
- fix(cli): ES private fields parsing in SWC (#5964)
- fix(cli): Better use of @ts-expect-error (#6038)
- fix(cli): media type for .cjs and application/node (#6005)
- fix(doc): remove JSDoc comment truncation (#6031)
- fix(cli/js/web): Body.bodyUsed should use IsReadableStreamDisturbed
- fix(cli/js/web): formData parser for binary files in fetch() (#6015)
- fix(cli/js/web): set null body for null-body status in fetch() (#5980)
- fix(cli/js/web): network error on multiple redirects in fetch() (#5985)
- fix(cli/js/web): Headers.name and FormData.name (#5994)
- upgrade: Rust crates (#5959, #6032)

Changes in std version 0.55.0:

- feat(std/hash): add Sha512 and HmacSha512 (#6009)
- feat(std/http) support code 103 Early Hints (#6021)
- feat(std/http): add TooEarly status code (#5999)
- feat(std/io): add LimitedReader (#6026)
- feat(std/log): buffered file logging (#6014)
- feat(std/mime/multipart): Added multiple FormFile input (#6027)
- feat(std/node): add util.type.isDate (#6029)
- fix(std/http): file server not closing files (#5952)
- fix(std/path): support browsers (#6003)

### 1.0.3 / 2020.05.29

Changes in the CLI:

- fix: Add unstable checks for Deno.dir and Diagnostics (#5750)
- fix: Add unstable checks for unix transport (#5818)
- fix: Create HTTP cache lazily (#5795)
- fix: Dependency analysis in TS compiler (#5817, #5785, #5870)
- fix: Expose Error.captureStackTrace (#5254)
- fix: Improved typechecking error for unstable props (#5503)
- fix: REPL evaluates in strict mode (#5565)
- fix: Write lock file before running any code (#5794)
- fix(debugger): BorrowMutError when evaluating expression in inspector console
  (#5822)
- fix(doc): Handle comments at the top of the file (#5891)
- fix(fmt): Handle formatting UTF-8 w/ BOM files (#5881)
- fix(permissions): Fix CWD and exec path leaks (#5642)
- fix(web/blob): DenoBlob name (#5879)
- fix(web/console): Hide `values` for console.table if display not necessary
  (#5914)
- fix(web/console): Improve indentation when displaying objects with console.log
  (#5909)
- fix(web/encoding): atob should throw dom exception (#5730)
- fix(web/fetch): Make Response constructor standard (#5787)
- fix(web/fetch): Allow ArrayBuffer as Fetch request body (#5831)
- fix(web/formData): Set default filename for Blob to <blob> (#5907)
- upgrade: dprint to 0.19.0 (#5899)

Changes in std version 0.54.0:

- feat(std/encoding): Add base64 (#5811)
- feat(std/http): Handle .wasm files in file_server (#5896)
- feat(std/node): Add link/linkSync polyfill (#5930)
- feat(std/node): fs.writeFile/sync path can now be an URL (#5652)
- feat(std/testing): Return results in benchmark promise (#5842)
- fix(std/http): readTrailer evaluates header names by case-insensitive (#4902)
- fix(std/log): Improve the calculation of byte length (#5819)
- fix(std/log): Fix FileHandler test with mode 'x' on non-English systems
  (#5757)
- fix(std/log): Use writeAllSync instead of writeSync (#5868)
- fix(std/testing/asserts): Support browsers (#5847)

### 1.0.2 / 2020.05.22

Changes in the CLI:

- fix: --inspect flag working like --inspect-brk (#5697)
- fix: Disallow http imports for modules loaded over https (#5680)
- fix: Redirects handling in module analysis (#5726)
- fix: SWC lexer settings and silent errors (#5752)
- fix: TS type imports (#5733)
- fix(fmt): Do not panic on new expr with no parens. (#5734)
- fix(cli/js/streams): High water mark validation (#5681)

Changes in std version 0.53.0:

- fix(std/http): file_server's target directory (#5695)
- feat(std/hash): add md5 (#5719)
- refactor: Move std/fmt/sprintf.ts to std/fmt/printf.ts (#4567)

### 1.0.1 / 2020.05.20

Changes in the CLI:

- fix(doc): crash on formatting type predicate (#5651)
- fix: Implement Deno.kill for windows (#5347)
- fix: Implement Deno.symlink() for windows (#5533)
- fix: Make Deno.remove() work with directory symlinks on windows (#5488)
- fix: Mark Deno.pid and Deno.noColor as const (#5593)
- fix: Remove debug prints introduced in e18aaf49c (#5356)
- fix: Return error if more than one listener calls `WorkerHandle::get_event()`
  (#5461)
- fix: Simplify fmt::Display for ModuleResolutionError (#5550)
- fix: URL utf8 encoding (#5557)
- fix: don't panic on Deno.close invalid argument (#5320)
- fix: panic if DENO_DIR is a relative path (#5375)
- fix: setTimeout and friends have too strict types (#5412)
- refactor: rewrite TS dependency analysis in Rust (#5029, #5603)
- update: dprint 0.18.4 (#5671)

Changes in std version 0.52.0:

- feat(std/bytes): add hasSuffix and contains functions, update docs (#4801)
- feat(std/fmt): rgb24 and bgRgb24 can use numbers for color (#5198)
- feat(std/hash): add fnv implementation (#5403)
- feat(std/node) Export TextDecoder and TextEncoder from util (#5663)
- feat(std/node): Add fs.promises.readFile (#5656)
- feat(std/node): add util.callbackify (#5415)
- feat(std/node): first pass at url module (#4700)
- feat(std/node): fs.writeFileSync polyfill (#5414)
- fix(std/hash): SHA1 hash of Uint8Array (#5086)
- fix(std/http): Add .css to the MEDIA_TYPES. (#5367)
- fix(std/io): BufReader should not share the internal buffer across reads
  (#4543)
- fix(std/log): await default logger setup (#5341)
- fix(std/node) improve fs.close compatibility (#5649)
- fix(std/node): fs.readFile should take string as option (#5316)
- fix(std/testing): Provide message and diff for assertStrictEq (#5417)

### 1.0.0 / 2020.05.13

Read more about this release at https://deno.land/v1

- fix: default to 0.0.0.0 for Deno.listen (#5203)
- fix: Make --inspect-brk pause on the first line of _user_ code (#5250)
- fix: Source maps in inspector for local files (#5245)
- upgrade: TypeScript 3.9 (#4510)

### 1.0.0-rc3 / 2020.05.12

- BREAKING: Remove public Rust API for the "deno" crate (#5226)
- feat(core): Allow starting isolate from snapshot bytes on the heap (#5187)
- fix: Check permissions in SourceFileFetcher (#5011)
- fix: Expose ErrorEvent globally (#5222)
- fix: Remove default --allow-read perm for deno test (#5208)
- fix: Source maps in inspector (#5223)
- fix(std/encoding/yaml): Correct exports (#5191)
- fix(plugins): prevent segfaults on windows (#5210)
- upgrade: dprint 0.17.2 (#5195)

### 1.0.0-rc2 / 2020.05.09

- BREAKING(std): Reorg modules, mark as unstable (#5087, #5177)
- BREAKING(std): Revert "Make WebSocket Reader/Writer" (#5002, #5141)
- BREAKING: Deno.execPath should require allow-read (#5109)
- BREAKING: Make Deno.hostname unstable #5108
- BREAKING: Make Worker with Deno namespace unstable (#5128)
- BREAKING: Remove support for .wasm imports (#5135)
- feat(bundle): Add --config flag (#5130)
- feat(bundle): Format output (#5139)
- feat(doc): Handle default exports (#4873)
- feat(repl): Add hint on how to exit REPL (#5143)
- feat(std/fmt): add 8bit and 24bit ANSI colors (#5168)
- feat(std/node): add fs.writefile / fs.promises.writeFile (#5054)
- feat(upgrade): Allow specifying a version (#5156)
- feat(workers): "crypto" global accessible in Worker scope (#5121)
- feat: Add support for X-Deno-Warning header (#5161)
- fix(imports): Fix panic on unsupported scheme (#5131)
- fix(inspector): Fix inspector hanging when task budget is exceeded (#5083)
- fix: Allow multiple Set-Cookie headers (#5100)
- fix: Better error message when DENO_DIR can't be created (#5120)
- fix: Check destination length in encodeInto in TextEncoder (#5078)
- fix: Correct type error text (#5150)
- fix: Remove unnecessary ProcessStdio declaration (#5092)
- fix: unify display of errors from Rust and JS (#5183)
- upgrade: rust crates (#5104)
- upgrade: to rusty_v8 0.4.2 / V8 8.4.300 (#5113)

### v1.0.0-rc1 / 2020.05.04

- BREAKING: make WebSocket directly implement AsyncIterable (#5045)
- BREAKING: remove CLI 'deno script.ts' alias to 'deno run script.ts' (#5026)
- BREAKING: remove support for JSON imports (#5037)
- BREAKING: remove window.location and self.location (#5034)
- BREAKING: reorder std/io/utils copyBytes arguments (#5022, #5021)
- feat(URL): Support drive letters for file URLs on Windows (#5074)
- feat(deno install): simplify CLI flags (#5036)
- feat(deno fmt): Add `deno-fmt-ignore` and `deno-fmt-ignore-file` comment
  support #5075
- feat(std): Add sha256 and sha224 support (along with HMAC variants) (#5066)
- feat(std/node): ability add to path argument to be URL type (#5055)
- feat(std/node): make process global (#4985)
- feat(std/node): toString for globals (#5013)
- feat: Add WritableStreams, TransformStream, TransformStreamController (#5042,
  #4980)
- feat: Make WebSocket Reader/Writer (#5002)
- feat: make Deno.cwd stable (#5068)
- fix(console): Formatting misalignment on console.table (#5046)
- fix(deno doc): Better repr for object literal types (#4998)
- fix(deno fmt): Format `abstract async` as `abstract async` (#5020)
- fix(std): Use fromFileUrl (#5005)
- fix(std/http): Hang when content-length unhandled (#5024)
- fix: Deno.chdir Should require allow-read not allow-write (#5033)
- fix: Respect NO_COLOR for stack frames (#5051)
- fix: URL constructor throws confusing error on invalid scheme (#5057)
- fix: Disallow static import of local modules from remote modules (#5050)
- fix: Misaligned error reporting on tab char (#5032)
- refactor(core): Add "prepare_load" hook to ModuleLoader trait (#4866)
- refactor: Don't expose unstable APIs to runtime (#5061 #4957)

### v0.42.0 / 2020.04.29

- BREAKING: "address" renamed to "path" in
  UnixAddr/UnixConnectOptions/UnixListenOptions (#4959)
- BREAKING: Change DirEntry to not require extra stat syscall (#4941)
- BREAKING: Change order of args in Deno.copy() (#4885)
- BREAKING: Change order of copyN arguments (#4900)
- BREAKING: Change return type of Deno.resources() (#4893)
- BREAKING: Deno.chdir() should require --allow-write (#4889)
- BREAKING: Factor out Deno.listenDatagram(), mark as unstable (#4968)
- BREAKING: Make shutdown unstable and async (#4940)
- BREAKING: Make unix sockets require allow-write (#4939)
- BREAKING: Map-like interface for Deno.env (#4942)
- BREAKING: Mark --importmap as unstable (#4934)
- BREAKING: Mark Deno.dir() unstable (#4924)
- BREAKING: Mark Deno.kill() as unstable (#4950)
- BREAKING: Mark Deno.loadavg() and osRelease() as unstable (#4938)
- BREAKING: Mark Deno.setRaw() as unstable (#4925)
- BREAKING: Mark Deno.umask() unstable (#4935)
- BREAKING: Mark Deno.utime() as unstable (#4955)
- BREAKING: Mark runtime compile ops as unstable (#4912)
- BREAKING: Mark signal APIs as unstable (#4926)
- BREAKING: Remove Conn.closeRead (#4970)
- BREAKING: Remove Deno.EOF, use null instead (#4953)
- BREAKING: Remove Deno.OpenMode (#4884)
- BREAKING: Remove Deno.runTests() API (#4922)
- BREAKING: Remove Deno.symbols namespace (#4936)
- BREAKING: Remove combined io interface like ReadCloser (#4944)
- BREAKING: Remove overload of Deno.test() taking named function (#4951)
- BREAKING: Rename Deno.fsEvents() to Deno.watchFs() (#4886)
- BREAKING: Rename Deno.toAsyncIterator() to Deno.iter() (#4848)
- BREAKING: Rename FileInfo time fields and represent them as Date objects
  (#4932)
- BREAKING: Rename SeekMode variants to camelCase and stabilize (#4946)
- BREAKING: Rename TLS APIs to camel case (#4888)
- BREAKING: Use LLVM target triple for Deno.build (#4948)
- BREAKING: introduce unstable flag; mark Deno.openPlugin, link, linkSync,
  symlink, symlinkSync as unstable (#4892)
- BREAKING: make camel case readDir, readLink, realPath (#4995)
- BREAKING: remove custom implementation of Deno.Buffer.toString() (#4992)
- BREAKING: std/node: require\_ -> require (#4828)
- feat(fmt): parallelize formatting (#4823)
- feat(installer): Add DENO_INSTALL_ROOT (#4787)
- feat(std/http): Improve parseHTTPVersion (#4930)
- feat(std/io): Increase copyN buffer size to match go implementation (#4904)
- feat(std/io): synchronous buffered writer (#4693)
- feat(std/path): Add fromFileUrl() (#4993)
- feat(std/uuid): Implement uuid v5 (#4916)
- feat(test): add quiet flag (#4894)
- feat: Add Deno.readTextFile(), Deno.writeTextFile(), with sync counterparts
  (#4901)
- feat: Add buffer size argument to copy (#4907)
- feat: Add close method to Plugin (#4670) (#4785)
- feat: Change URL.port implementation to match WHATWG specifications (#4954)
- feat: Deno.startTLS() (#4773, #4965)
- feat: Make zero a valid port for URL (#4963)
- feat: add help messages to Deno.test() sanitizers (#4887)
- feat: support Deno namespace in Worker API (#4784)
- fix(core): Op definitions (#4814)
- fix(core): fix top-level-await error handling (#4911)
- fix(core/js_errors): Get error's name and message from JS fields (#4808)
- fix(format): stdin not formatting JSX (#4971)
- fix(installer): handle case-insensitive uri (#4909)
- fix(std): existsFile test
- fix(std/fs): move dest if not exists and overwrite (#4910)
- fix(std/io): Make std/io copyN write the whole read buffer (#4978)
- fix(std/mime): MultipartReader for big files (#4865)
- fix(std/node): bug fix and tests fs/mkdir (#4917)
- fix: bug in Deno.copy (#4977)
- fix: don't throw RangeError when an invalid date is passed (#4929)
- fix: make URLSearchParams more standardized (#4695)
- refactor(cli): Improve source line formatting (#4832)
- refactor(cli): Move resource_table from deno::State to deno_core::Isolate
  (#4834)
- refactor(cli): Remove bootstrap methods from global scope after bootstrapping
  (#4869)
- refactor(cli/doc): Factor out AstParser from DocParser (#4923)
- refactor(cli/inspector): Store debugger url on DenoInspector (#4793)
- refactor(cli/js): Rewrite streams (#4842)
- refactor(cli/js/io): Change type of stdio handles in JS api (#4891, #4952)
- refactor(cli/js/io): Rename sync io interfaces (#4945)
- refactor(cli/js/net): Deno.listener closes when breaking out of async iterator
  (#4976)
- refactor(cli/js/permissions): Split read and write permission descriptors
  (#4774)
- refactor(cli/js/testing): Rename disableOpSanitizer to sanitizeOps (#4854)
- refactor(cli/js/web): Change InspectOptions, mark Deno.inspect as stable
  (#4967)
- refactor(cli/js/web): Decouple Console implementation from stdout (#4899)
- refactor(cli/ops): Replace block_on in net interfaces (#4796)
- refactor(cli|std): Add no-async-promise-executor lint rule (#4809)
- refactor(core): Modify op dispatcher to include &mut Isolate argument (#4821)
- refactor(core): Remove core/plugin.rs (#4824)
- refactor(core): Rename deno_core::Isolate to deno_core::CoreIsolate (#4851)
- refactor(core): add id field to RecursiveModuleLoad (#4905)
- refactor(std/log): support enum log level (#4859)
- refactor(std/node): proper Node polyfill directory iteration (#4783)
- upgrade: Rust 1.43.0 (#4871)
- upgrade: dprint 0.13.0 (#4816)
- upgrade: dprint 0.13.1 (#4853)
- upgrade: rusty_v8 v0.4.0 (#4856)
- chore: Mark Deno.Metrics and Deno.RunOptions as stable (#4949)

### v0.41.0 / 2020.04.16

- BREAKING: Improve readdir() and FileInfo interfaces (#4763)
- BREAKING: Remove deprecated APIs for mkdir and mkdirSync (#4615)
- BREAKING: Make fetch API more web compatible (#4687)
- BREAKING: Remove std/testing/format.ts (#4749)
- BREAKING: Migrate std/types to deno.land/x/types/ (#4713, #4771)
- feat(doc): support for runtime built-ins (#4635)
- feat(std/log): rotating handler, docs improvements (#4674)
- feat(std/node): Add isPrimitive method (#4673)
- feat(std/node/fs): Add copyFile and copyFileSync methods (#4726)
- feat(std/signal): Add onSignal method (#4696)
- feat(std/testing): Change output of diff (#4697)
- feat(std/http): Verify cookie name (#4685)
- feat(std/multipart): Make readForm() type safe (#4710)
- feat(std/uuid): Add UUID v1 (#4758)
- feat(install): Honor log level arg (#4714)
- feat(workers): Make Worker API more web compatible (#4684, #4734, #4391,
  #4737, #4746)
- feat: Add AbortController and AbortSignal API (#4757)
- fix(install): Clean up output on Windows (#4764)
- fix(core): Handle SyntaxError during script compilation (#4770)
- fix(cli): Async stack traces and stack formatting (#4690, #4706, #4715)
- fix(cli): Remove unnecessary namespaces in "deno types" (#4683, #4698, #4718,
  #4719, #4736, #4741)
- fix(cli): Panic on invalid UTF-8 string (#4704)
- fix(cli/js/net): Make generator return types iterable (#4661)
- fix(doc): Handle optional and extends fields (#4738, #4739)
- refactor: Event and EventTarget implementation (#4707)
- refactor: Use synchronous syscalls where applicable (#4762)
- refactor: Remove calls to futures::executor::block_on (#4760, #4775)
- upgrade: Rust crates (#4742)

### v0.40.0 / 2020.04.08

- BREAKING: Rename 'deno fetch' subcommand to 'deno cache' (#4656)
- BREAKING: Remove std/testing/runner.ts (#4649)
- feat(std/flags): Pass key and value to unknown (#4637)
- feat(std/http): Respond with 400 on request parse failure (#4614)
- feat(std/node): Add exists and existsSync (#4655)
- feat: Add File support in FormData (#4632)
- feat: Expose ReadableStream and make Blob more standardized (#4581)
- feat: add --importmap flag to deno bundle (#4651)
- fix(#4546): Added Math.trunc to toSecondsFromEpoch to conform the result to
  u64 (#4575)
- fix(file_server): use text/typescript instead of application/typescript
  (#4620)
- fix(std/testing): format bigint (#4626)
- fix: Drop headers with trailing whitespace in header name (#4642)
- fix: Fetch reference types for JS files (#4652)
- fix: Improve deno doc (#4672, #4625)
- fix: On init create disk_cache directory if it doesn't already exists (#4617)
- fix: Remove unnecessary namespaces in "deno types" (#4677, #4675, #4669,
  #4668, #4665, #4663, #4662)
- upgrade: Rust crates (#4679)

### v0.39.0 / 2020.04.03

- BREAKING CHANGE: Move encode, decode helpers to /std/encoding/utf8.ts, delete
  /std/strings/ (#4565)
- BREAKING CHANGE: Remove /std/media_types (#4594)
- BREAKING CHANGE: Remove old release files (#4545)
- BREAKING CHANGE: Remove std/strings/pad.ts because String.prototype.padStart
  exists (#4564)
- feat: Add common to std/path (#4527)
- feat: Added colors to doc output (#4518)
- feat: Expose global state publicly (#4572)
- feat: Make inspector more robust, add --inspect-brk support (#4552)
- feat: Publish deno types on release (#4583)
- feat: Support dynamic import in bundles. (#4561)
- feat: deno test --filter (#4570)
- feat: improve console.log serialization (#4524, #4472)
- fix(#4550): setCookie should append cookies (#4558)
- fix(#4554): use --inspect in repl & eval (#4562)
- fix(deno doc): handle 'declare' (#4573)
- fix(deno doc): parse super-class names (#4595)
- fix(deno doc): parse the "implements" clause of a class def (#4604)
- fix(file_server): serve appropriate content-type header (#4555)
- fix(inspector): proper error message on port collision (#4514)
- fix: Add check to fail the benchmark test on server error (#4519)
- fix: Properly handle invalid utf8 in paths (#4609)
- fix: async ops sanitizer false positives in timers (#4602)
- fix: invalid blob type (#4536)
- fix: make Worker.poll private (#4603)
- fix: remove `Send` trait requirement from the `Resource` trait (#4585)
- refactor(testing): Reduce testing interfaces (#4451)
- upgrade: dprint to 0.9.10 (#4601)
- upgrade: rusty_v8 v0.3.10 (#4576)

### v0.38.0 / 2020.03.28

- feat: Add "deno doc" subcommand (#4500)
- feat: Support --inspect, Chrome Devtools support (#4484)
- feat: Support Unix Domain Sockets (#4176)
- feat: add queueMicrotask to d.ts (#4477)
- feat: window.close() (#4474)
- fix(console): replace object abbreviation with line breaking (#4425)
- fix: add fsEvent notify::Error casts (#4488)
- fix: hide source line if error message longer than 150 chars (#4487)
- fix: parsing bug (#4483)
- fix: remove extra dot in Permission request output (#4471)
- refactor: rename ConsoleOptions to InspectOptions (#4493)
- upgrade: dprint 0.9.6 (#4509, #4491)
- upgrade: prettier 2 for internal code formatting (#4498)
- upgrade: rusty_v8 to v0.3.9 (#4505)

### v0.37.1 / 2020.03.23

- fix: Statically link the C runtime library on Windows (#4469)

### v0.37.0 / 2020.03.23

- BREAKING CHANGE: FileInfo.len renamed to FileName.size (#4338)
- BREAKING CHANGE: Rename Deno.run's args to cmd (#4444)
- feat(ci): Releases should all use zip and LLVM target triples (#4460)
- feat(console): Symbol.toStringTag and display Object symbol entries (#4388)
- feat(std/node): Add chmod Node polyfill (#4358)
- feat(std/node): Add node querystring polyfill (#4370)
- feat(std/node): Node polyfill for fs.chown and fs.close (#4377)
- feat(std/permissions): Add helper functions for permissions to std (#4258)
- feat(std/types): Provide types for React and ReactDOM (#4376)
- feat(test): Add option to skip tests (#4351)
- feat(test): Add support for jsx/tsx for deno test (#4369)
- feat: Add mode option to open/create (#4289)
- feat: Deno.test() sanitizes ops and resources (#4399)
- feat: Fetch should accept a FormData body (#4363)
- feat: First pass at "deno upgrade" (#4328)
- feat: Provide way to build Deno without building V8 from source (#4412)
- feat: Remove `Object.prototype.__proto__` (#4341)
- fix(std/http): Close open connections on server close (#3679)
- fix(std/http): Properly await ops in a server test (#4436)
- fix(std/http): Remove bad error handling (#4435)
- fix(std/node): Node polyfill fsAppend rework (#4322)
- fix(std/node): Stack traces for modules imported via require (#4035)
- fix: Importing JSON doesn't work in bundles (#4404)
- fix: Simplify timer with macrotask callback (#4385)
- fix: Test runner ConnectionReset bug (#4424)
- fix: chmod should throw on Windows (#4446)
- fix: fetch closes unused body (#4393)
- perf: Optimize TextEncoder and TextDecoder (#4430, #4349)
- refactor: Improve test runner (#4336, #4352, #4356, #4371)
- refactor: Remove std/testing/runner.ts, use deno test (#4397, #4392)
- upgrade: Rust 1.42.0 (#4331)
- upgrade: Rust crates (#4412)
- upgrade: to rusty_v8 0.3.5 / v8 8.2.308 (#4364)

### v0.36.0 / 2020.03.11

- BREAKING CHANGE: Remove Deno.errors.Other (#4249)
- BREAKING CHANGE: Rename readDir -> readdir (#4225)
- feat(std/encoding): add binary module (#4274)
- feat(std/node): add appendFile and appendFileSync (#4294)
- feat(std/node): add directory classes (#4087)
- feat(std/node): add os.tmpdir() implementation (#4213)
- feat: Add Deno.umask (#4290)
- feat: Add global --quiet flag (#4135)
- feat: Improvements to std/flags. (#4279)
- feat: Make internal error frames dimmer (#4201)
- feat: Support async function and EventListenerObject as listeners (#4240)
- feat: add actual error class to fail message (#4305)
- feat: seek should return cursor position (#4211)
- feat: support permission mode in mkdir (#4286)
- feat: update metrics to track different op types (#4221)
- fix: Add content type for wasm, fix encoding in wasm test fixture (#4269)
- fix: Add waker to StreamResource to fix hang on close bugs (#4293)
- fix: Flattens dispatch error handling to produce one less useless stack frame
  on op errors. (#4189)
- fix: JavaScript dependencies in bundles. (#4215)
- fix: Stricter permissions for Deno.makeTemp (#4318)
- fix: `deno install` file name including extra dot on Windows (#4243)
- fix: inlining of lib.dom.iterable.d.ts. (#4242)
- fix: properly close FsEventsResource (#4266)
- fix: remove unwanted ANSI Reset Sequence (#4268)
- perf: use Object instead of Map for promise table (#4309)
- perf: use subarray instead of slice in dispatch minimal (#4180)
- refactor(cli/js): add assertOps and assertResources sanitizer in cli/js/ unit
  tests (#4209, #4161)
- refactor(cli/js/net): Cleanup iterable APIs (#4236)
- refactor(core): improve exception handling(#4214, #4214, #4198)
- refactor(core): rename structures related to Modules (#4217)
- refactor: Cleanup options object parameters (#4296)
- refactor: Migrate internal bundles to System (#4233)
- refactor: Rename Option -> Options (#4226)
- refactor: cleanup compiler runtimes (#4230)
- refactor: preliminary cleanup of Deno.runTests() (#4237)
- refactor: reduce unnecessary output in cli/js tests (#4182)
- refactor: reorganize cli/js (#4317, #4316, #4310, #4250, #4302, #4283, #4264)
- refactor: rewrite testPerm into unitTest (#4231)
- refactor: uncomment tests broken tests, use skip (#4311)
- upgrade: dprint 0.8.0 (#4308, #4314)
- upgrade: rust dependencies (#4270)
- upgrade: typescript 3.8.3 (#4301)

### v0.35.0 / 2020.02.28

- feat: Deno.fsEvents() (#3452)
- feat: Support UDP sockets (#3946)
- feat: Deno.setRaw(rid, mode) to turn on/off raw mode (#3958)
- feat: Add Deno.formatDiagnostics (#4032)
- feat: Support TypeScript eval through `deno eval -T` flag (#4141)
- feat: Support types compiler option in compiler APIs (#4155)
- feat: add std/examples/chat (#4022, #4109, #4091)
- feat: support brotli compression for fetch API (#4082)
- feat: reverse URL lookup for cache (#4175)
- feat(std/node): add improve os module (#4064, #4075, #4065)
- feat(std/node): add os Symbol.toPrimitive methods (#4073)
- fix(fetch): proper error for unsupported protocol (#4085)
- fix(std/examples): add tests for examples (#4094)
- fix(std/http): Consume unread body before reading next request (#3990)
- fix(std/ws): createSecKey logic (#4063)
- fix(std/ws): provide default close code for ws.close() (#4172)
- fix(std/ws): sock shouldn't throw eof error when failed to read frame (#4083)
- fix: Bundles can be sync or async based on top level await (#4124)
- fix: Move WebAssembly namespace to shared_globals (#4084)
- fix: Resolve makeTemp paths from CWD (#4104)
- fix: Return non-zero exit code on malformed stdin fmt (#4163)
- fix: add window.self read-only property (#4131)
- fix: fetch in workers (#4054)
- fix: fetch_cached_remote_source support redirect URL without base (#4099)
- fix: issues with JavaScript importing JavaScript. (#4120)
- fix: rewrite normalize_path (#4143)
- refactor(std/http): move io functions to http/io.ts (#4126)
- refactor: Deno.errors (#3936, #4058, #4113, #4093)
- upgrade: TypeScript 3.8 (#4100)
- upgrade: dprint 0.7.0 (#4130)
- upgrade: rusty_v8 0.3.4 (#4179)

### v0.34.0 / 2020.02.20

- feat: Asynchronous event iteration node polyfill (#4016)
- feat: Deno.makeTempFile (#4024)
- feat: Support loading additional TS lib files (#3863)
- feat: add --cert flag for http client (#3972)
- feat(std/io): Export readDelim(), readStringDelim() and readLines() from
  bufio.ts (#4019)
- fix(deno test): support directories as arguments (#4011)
- fix: Enable TS strict mode by default (#3899)
- fix: detecting AMD like imports (#4009)
- fix: emit when bundle contains single module (#4042)
- fix: mis-detecting imports on JavaScript when there is no checkJs (#4040)
- fix: skip non-UTF-8 dir entries in Deno.readDir() (#4004)
- refactor: remove run_worker_loop (#4028)
- refactor: rewrite file_fetcher (#4037, #4030)
- upgrade: dprint 0.6.0 (#4026)

### v0.33.0 / 2020.02.13

- feat(std/http): support trailer headers (#3938, #3989)
- feat(std/node): Add readlink, readlinkSync (#3926)
- feat(std/node): Event emitter node polyfill (#3944, #3959, #3960)
- feat(deno install): add --force flag and remove yes/no prompt (#3917)
- feat: Improve support for diagnostics from runtime compiler APIs (#3911)
- feat: `deno fmt -` formats stdin and print to stdout (#3920)
- feat: add std/signal (#3913)
- feat: make testing API built-in Deno.test() (#3865, #3930, #3973)
- fix(std/http): align serve and serveTLS APIs (#3881)
- fix(std/http/file_server): don't crash on "%" pathname (#3953)
- fix(std/path): Use non-capturing groups in globrex() (#3898)
- fix(deno types): don't panic when piped to head (#3910)
- fix(deno fmt): support top-level await (#3952)
- fix: Correctly determine a --cached-only error (#3979)
- fix: No longer require aligned buffer for shared queue (#3935)
- fix: Prevent providing --allow-env flag twice (#3906)
- fix: Remove unnecessary EOF check in Deno.toAsyncIterable (#3914)
- fix: WASM imports loaded HTTP (#3856)
- fix: better WebWorker API compatibility (#3828 )
- fix: deno fmt improvements (#3988)
- fix: make WebSocket.send() exclusive (#3885)
- refactor: Improve `deno bundle` by using System instead of AMD (#3965)
- refactor: Remove conditionals from installer (#3909)
- refactor: peg workers to a single thread (#3844, #3968, #3931, #3903, #3912,
  #3907, #3904)

### v0.32.0 / 2020.02.03

- BREAKING CHANGE: Replace formatter for "deno fmt", use dprint (#3820, #3824,
  #3842)
- BREAKING CHANGE: Remove std/prettier (#3820)
- BREAKING CHANGE: Remove std/installer (#3843)
- BREAKING CHANGE: Remove --current-thread flag (#3830)
- BREAKING CHANGE: Deno.makeTempDir() checks permissions (#3810)
- feat: deno install in Rust (#3806)
- feat: Improve support of type definitions (#3755)
- feat: deno fetch supports --lock-write (#3787)
- feat: deno eval supports --v8-flags=... (#3797)
- feat: descriptive permission errors (#3808)
- feat: Make fetch API more standards compliant (#3667)
- feat: deno fetch supports multiple files (#3845)
- feat(std/node): Endianness (#3833)
- feat(std/node): Partial os polyfill (#3821)
- feat(std/examples): Bring back xeval (#3822)
- feat(std/encoding): Add base32 support (#3855)
- feat(deno_typescript): Support crate imports (#3814)
- fix: Panic on cache miss (#3784)
- fix: Deno.remove() to properly remove dangling symlinks (#3860)
- refactor: Use tokio::main attribute in lib.rs (#3831)
- refactor: Provide TS libraries for window and worker scope (#3771, #3812,
  #3728)
- refactor(deno_core): Error tracking and scope passing (#3783)
- refactor(deno_core): Rename PinnedBuf to ZeroCopyBuf (#3782)
- refactor(deno_core): Change Loader trait (#3791)
- upgrade: Rust 1.41.0 (#3838)
- upgrade: Rust crates (#3829)

### v0.31.0 / 2020.01.24

- BREAKING CHANGE: remove support for blob: URL in Worker (#3722)
- BREAKING CHANGE: remove Deno namespace support and noDenoNamespace option in
  Worker constructor (#3722)
- BREAKING CHANGE: rename dial to connect and dialTLS to connectTLS (#3710)
- feat: Add signal handlers (#3757)
- feat: Implemented alternative open mode in files (#3119)
- feat: Use globalThis to reference global scope (#3719)
- feat: add AsyncUnref ops (#3721)
- feat: stabilize net Addr (#3709)
- fix: correct yaml's sortKeys type (#3708)
- refactor: Improve path handling in permission checks (#3714)
- refactor: Improve web workers (#3722, #3732, #3730, #3735)
- refactor: Reduce number of ErrorKind variants (#3662)
- refactor: Remove Isolate.shared_response_buf optimization (#3759)
- upgrade: rusty_v8 (#3764, #3769, #3741)

### v0.30.0 / 2020.01.17

- BREAKING CHANGE Revert "feat(flags): script arguments come after '--'" (#3681)
- feat(fs): add more unix-only fields to FileInfo (#3680)
- feat(http): allow response body to be string (#3705)
- feat(std/node): Added node timers builtin (#3634)
- feat: Add Deno.symbols and move internal fields for test (#3693)
- feat: Add gzip, brotli and ETag support for file fetcher (#3597)
- feat: support individual async handler for each op (#3690)
- fix(workers): minimal error handling and async module loading (#3665)
- fix: Remove std/multipart (#3647)
- fix: Resolve read/write whitelists from CWD (#3684)
- fix: process hangs when fetch called (#3657)
- perf: Create an old program to be used in snapshot (#3644, #3661)
- perf: share http client in file fetcher (#3683)
- refactor: remove Isolate.current_send_cb_info and DenoBuf, port
  Isolate.shared_response_buf (#3643)

### v0.29.0 / 2020.01.09

- BREAKING CHANGE Remove xeval subcommand (#3630)
- BREAKING CHANGE script arguments should come after '--' (#3621)
- BREAKING CHANGE Deno.mkdir should conform to style guide BREAKING CHANGE
  (#3617)
- BREAKING CHANGE Deno.args only includes script args (#3628)
- BREAKING CHANGE Rename crates: 'deno' to 'deno_core' and 'deno_cli' to 'deno'
  (#3600)
- feat: Add Deno.create (#3629)
- feat: Add compiler API (#3442)
- fix(ws): Handshake with correctly empty search string (#3587)
- fix(yaml): Export parseAll (#3592)
- perf: TextEncoder.encode improvement (#3596, #3589)
- refactor: Replace libdeno with rusty_v8 (#3556, #3601, #3602, #3605, #3611,
  #3613, #3615)
- upgrade: V8 8.1.108 (#3623)

### v0.28.1 / 2020.01.03

- feat(http): make req.body a Reader (#3575)
- fix: dynamically linking to OpenSSL (#3586)

### v0.28.0 / 2020.01.02

- feat: Add Deno.dir("executable") (#3526)
- feat: Add missing mod.ts files in std (#3509)
- fix(repl): Do not crash on async op reject (#3527)
- fix(std/encoding/yaml): support document separator in parseAll (#3535)
- fix: Allow reading into a 0-length array (#3329)
- fix: Drop unnecessary Object.assign from createResolvable() (#3548)
- fix: Expose shutdown() and ShutdownMode TS def (#3558, #3560)
- fix: Remove wildcard export in uuid module (#3540)
- fix: Return null on error in Deno.dir() (#3531)
- fix: Use shared HTTP client (#3563)
- fix: Use sync ops when clearing the console (#3533)
- refactor: Move HttpBody to cli/http_util.rs (#3569)
- upgrade: Reqwest to 0.10.0 (#3567)
- upgrade: Rust to 1.40.0 (#3542)
- upgrade: Tokio 0.2 (#3418, #3571)

### v0.27.0 / 2019.12.18

- feat: Support utf8 in file_server (#3495)
- feat: add help & switch to flags to file_server (#3489)
- feat: fetch should support URL instance as input (#3496)
- feat: replace Deno.homeDir with Deno.dir (#3491, #3518)
- feat: show detailed version with --version (#3507)
- fix(installer): installs to the wrong directory on Windows (#3462)
- fix(std/http): close connection on .respond() error (#3475)
- fix(std/node): better error message for read perm in require() (#3502)
- fix(timer): due/now Math.max instead of min (#3477)
- fix: Improve empty test case error messages (#3514)
- fix: Only swallow NotFound errors in std/fs/expandGlob() (#3479)
- fix: decoding uri in file_server (#3187)
- fix: file_server should get file and fileInfo concurrently (#3486)
- fix: file_server swallowing permission errors (#3467)
- fix: isolate tests silently failing (#3459)
- fix: permission errors are swallowed in fs.exists, fs.emptyDir, fs.copy
  (#3493, #3501, #3504)
- fix: plugin ops should change op count metrics (#3455)
- fix: release assets not being executable (#3480)
- upgrade: tokio 0.2 in deno_core_http_bench, take2 (#3435)
- upgrade: upgrade subcommand links to v0.26.0 (#3492)

### v0.26.0 / 2019.12.05

- feat: Add --no-remote, rename --no-fetch to --cached-only (#3417)
- feat: Native plugins AKA dlopen (#3372)
- fix: Improve html for file_server (#3423)
- fix: MacOS Catalina build failures (#3441)
- fix: Realpath behavior in windows (#3425)
- fix: Timer/microtask ordering (#3439)
- fix: Tweaks to arg_hacks and add v8-flags to repl (#3409)
- refactor: Disable eager polling for ops (#3434)

### v0.25.0 / 2019.11.26

- feat: Support named exports on bundles (#3352)
- feat: Add --check for deno fmt (#3369)
- feat: Add Deno.realpath (#3404)
- feat: Add ignore parser for std/prettier (#3399)
- feat: Add std/encoding/yaml module (#3361)
- feat: Add std/node polyfill for require() (#3382, #3380)
- feat: Add std/node/process (#3368)
- feat: Allow op registration during calls in core (#3375)
- feat: Better error message for missing module (#3402)
- feat: Support load yaml/yml prettier config (#3370)
- fix: Make private namespaces in lib.deno_runtime.d.ts more private (#3400)
- fix: Remote .wasm import content type issue (#3351)
- fix: Run std tests with cargo test (#3344)
- fix: deno fmt should respect prettierrc and prettierignore (#3346)
- fix: std/datetime toIMF bug (#3357)
- fix: better error for 'relative import path not prefixed with...' (#3405)
- refactor: Elevate DenoPermissions lock to top level (#3398)
- refactor: Reorganize flags, removes ability to specify run arguments like
  `--allow-net` after the script (#3389)
- refactor: Use futures 0.3 API (#3358, #3359, #3363, #3388, #3381)
- chore: Remove unneeded tokio deps (#3376)

### v0.24.0 / 2019.11.14

- feat: Add Node compat module std/node (#3319)
- feat: Add permissions.request (#3296)
- feat: Add prettier flags to deno fmt (#3314)
- feat: Allow http server to take { hostname, port } argument (#3233)
- feat: Make bundles fully standalone (#3325)
- feat: Support .wasm via imports (#3328)
- fix: Check for closing status when iterating Listener (#3309)
- fix: Error handling in std/fs/walk() (#3318)
- fix: Exclude prebuilt from deno_src release (#3272)
- fix: Turn on TS strict mode for deno_typescript (#3330)
- fix: URL parse bug (#3316)
- refactor: resources and workers (#3285, #3271, #3274, #3342, #3290)
- upgrade: Prettier 1.19 (#3275, #3305)
- upgrade: Rust deps (#3292)
- upgrade: TypeScript 3.7 (#3275)
- upgrade: V8 8.0.192

### v0.23.0 / 2019.11.04

- feat: Add serveTLS and listenAndServeTLS (#3257)
- feat: Lockfile support (#3231)
- feat: Adds custom inspect method for URL (#3241)
- fix: Support for deep `Map` equality with `asserts#equal` (#3236, #3258)
- fix: Make EOF unique symbol (#3244)
- fix: Prevent customInspect error from crashing console (#3226)

### v0.22.0 / 2019.10.28

- feat: Deno.listenTLS (#3152)
- feat: Publish source tarballs for releases (#3203)
- feat: Support named imports/exports for subset of properties in JSON modules
  (#3210)
- feat: Use web standard Permissions API (#3200)
- feat: Remove --no-prompt flag, fail on missing permissions (#3183)
- feat: top-level-for-await (#3212)
- feat: Add ResourceTable in core (#3150)
- feat: Re-enable standard stream support for fetch bodies (#3192)
- feat: Add CustomInspect for Headers (#3130)
- fix: Cherry-pick depot_tools 6a1d778 to fix macOS Catalina issues (#3175)
- fix: Remove runtime panics in op dispatch (#3176, #3202, #3131)
- fix: BufReader.readString to actually return Deno.EOF at end (#3191)
- perf: faster TextDecoder (#3180, #3204)
- chore: Reenable std tests that were disabled during merge (#3159)
- chore: Remove old website (#3194, #3181)
- chore: Use windows-2019 image in Github Actions (#3198)
- chore: use v0.21.0 for subcommands (#3168)
- upgrade: V8 to 7.9.317.12 (#3208)

### v0.21.0 / 2019.10.19

- feat: --reload flag to take arg for partial reload (#3109)
- feat: Allow "deno eval" to run code as module (#3148)
- feat: support --allow-net=:4500 (#3115)
- fix: Ensure DENO_DIR when saving the REPL history (#3106)
- fix: Update echo_server to new listen API (denoland/deno_std#625)
- fix: [prettier] deno fmt should format jsx/tsx files (#3118)
- fix: [tls] op_dial_tls is not registered and broken (#3121)
- fix: clearTimer bug (#3143)
- fix: remote jsx/tsx files were compiled as js/ts (#3125)
- perf: eager poll async ops in Isolate (#3046, #3128)
- chore: Move std/fs/path to std/path (#3100)
- upgrade: V8 to 7.9.304 (#3127)
- upgrade: prettier type definition (#3101)
- chore: Add debug build to github actions (#3127)
- chore: merge deno_std into deno repo (#3091, #3096)

### v0.20.0 / 2019.10.06

In deno:

- feat: Add Deno.hostname() (#3032)
- feat: Add support for passing a key to Deno.env() (#2952)
- feat: JSX Support (#3038)
- feat: Replace Isolate::set_dispatch with Isolate::register_op (#3002, #3039,
  #3041)
- feat: window.onunload (#3023)
- fix: Async compiler processing (#3043)
- fix: Implement ignoreBOM option of UTF8Decoder in text_encoding (#3040)
- fix: Support top-level-await in TypeScript (#3024)
- fix: iterators on UrlSearchParams (#3044)
- fix: listenDefaults/dialDefaults may be overridden in some cases (#3027)
- upgrade: V8 to 7.9.218 (#3067)
- upgrade: rust to 1.38.0 (#3030)
- chore: Migrate CI to github actions (#3052, #3056, #3049, #3071, #3076, #3070,
  #3066, #3061, #3010)
- chore: Remove deno_cli_snapshots crate. Move //js to //cli/js (#3064)
- chore: use xeval from deno_std (#3058)

In deno_std:

- feat: test runner v2 (denoland/deno_std#604)
- feat: wss support with dialTLS (denoland/deno_std#615)
- fix(ws): mask must not be set by default for server (denoland/deno_std#616)
- fix: Implement expandGlob() and expandGlobSync() (denoland/deno_std#617)
- upgrade: eslint and @typescript-eslint (denoland/deno_std#621)

### v0.19.0 / 2019.09.24

In deno:

- feat: Add Deno.dialTLS()
- feat: Make deno_cli installable via crates.io (#2946)
- feat: Remove test.py, use cargo test as test frontend (#2967)
- feat: dial/listen API change (#3000)
- feat: parallelize downloads from TS compiler (#2949)
- fix: Make `window` compatible with ts 3.6 (#2984)
- fix: Remove some non-standard web API constructors (#2970)
- fix: debug logging in runtime/compiler (#2953)
- fix: flag parsing of config file (#2996)
- fix: reschedule global timer if it fires earlier than expected (#2989)
- fix: type directive parsing (#2954)
- upgrade: V8 to 7.9.110 for top-level-await (#3015)
- upgrade: to TypeScript 3.6.3 (#2969)

In deno_std:

- feat: Implement BufReader.readString (denoland/deno_std#607)
- fix: TOML's key encoding (denoland/deno_std#612)
- fix: remove //testing/main.ts (denoland/deno_std#605)
- fix: types in example_client for ws module (denoland/deno_std#609)
- upgrade: mime-db to commit c50e0d1 (denoland/deno_std#608)

### v0.18.0 / 2019.09.13

In deno:

- build: remove tools/build.py; cargo build is the build frontend now (#2865,
  #2874, #2876)
- feat: Make integration tests rust unit tests (#2884)
- feat: Set user agent for http client (#2916)
- feat: add bindings to run microtasks from Isolate (#2793)
- fix(fetch): implement bodyUsed (#2877)
- fix(url): basing in constructor (#2867, #2921)
- fix(xeval): incorrect chunk matching behavior (#2857)
- fix: Default 'this' to window in EventTarget (#2918)
- fix: Expose the DOM Body interface globally (#2903)
- fix: Keep all deno_std URLs in sync (#2930)
- fix: make 'deno fmt' faster (#2928)
- fix: panic during block_on (#2905)
- fix: panic during fetch (#2925)
- fix: path normalization in resolve_from_cwd() (#2875)
- fix: remove deprecated Deno.platform (#2895)
- fix: replace bad rid panics with errors (#2870)
- fix: type directives import (#2910)
- upgrade: V8 7.9.8 (#2907)
- upgrade: rust crates (#2937)

In deno_std:

- feat: Add xeval (denoland/deno_std#581)
- fix(flags): Parse builtin properties (denoland/deno_std#579)
- fix(uuid): Make it v4 rfc4122 compliant (denoland/deno_std#580)
- perf: Improve prettier speed by adding d.ts files (denoland/deno_std#591)
- upgrade: prettier to 1.18.2 (denoland/deno_std#592)

### v0.17.0 / 2019.09.04

In deno:

- feat: Add window.queueMicrotask (#2844)
- feat: Support HTTP proxies in fetch (#2822)
- feat: Support `_` and `_error` in REPL (#2845, #2843)
- feat: add statusText for fetch (#2851)
- feat: implement Addr interface (#2821)
- fix: Improve error stacks for async ops (#2820)
- fix: add console.dirxml (#2835)
- fix: do not export `isConsoleInstance` (#2850)
- fix: set/clearTimeout's params should not be bigint (#2834, #2838)
- fix: shared queue requires aligned buffer (#2816)
- refactor: Remove Node build dependency and change how internal V8 snapshots
  are built (#2825, #2827, #2826, #2826)
- refactor: Remove flatbuffers (#2818, #2819, #2817, #2812, #2815, #2799)
- regression: Introduce regression in fetch's Request/Response stream API to
  support larger refactor (#2826)

In deno_std:

- fix: better paths handling in test runner (denoland/deno_std#574)
- fix: avoid prototype builtin `hasOwnProperty` (denoland/deno_std#577)
- fix: boolean regexp (denoland/deno_std#582)
- fix: printf should use padEnd and padStart (denoland/deno_std#583)
- fix: ws should use crypto getRandomValues (denoland/deno_std#584)

### v0.16.0 / 2019.08.22

In deno:

- feat: "deno test" subcommand (#2783, #2784, #2800)
- feat: implement console.trace() (#2780)
- feat: support .d.ts files (#2746)
- feat: support custom inspection of objects (#2791)
- fix: dynamic import panic (#2792)
- fix: handle tsconfig.json with comments (#2773)
- fix: import map panics, use import map's location as its base URL (#2770)
- fix: set response.url (#2782)

In deno_std:

- feat: add overloaded form of unit test declaration (denoland/deno_std#563)
- feat: add printf implementation (fmt/sprintf.ts) (denoland/deno_std#566)
- feat: print out the failed tests after the summary (denoland/deno_std#554)
- feat: test runner (denoland/deno_std#516, denoland/deno_std#564,
  denoland/deno_std#568)
- fix: accept absolute root directories in the file server
  (denoland/deno_std#558)
- fix: refactor 'assertEquals' (denoland/deno_std#560)
- fix: test all text functions in colors module (denoland/deno_std#553)
- fix: move colors module into fmt module (denoland/deno_std#571)

### v0.15.0 / 2019.08.13

In deno:

- feat: print cache location when no arg in deno info (#2752)
- fix: Dynamic import should respect permissions (#2764)
- fix: Propagate Url::to_file_path() errors instead of panicking (#2771)
- fix: cache paths on Windows are broken (#2760)
- fix: dynamic import base path problem for REPL and eval (#2757)
- fix: permission requirements for Deno.rename() and Deno.link() (#2737)

In deno_std: None

### v0.14.0 / 2019.08.09

In deno:

- feat: remove `Deno.build.args` (#2728)
- feat: support native line ending conversion in the `Blob` constructor (#2695)
- feat: add option to delete the `Deno` namespace in a worker (#2717)
- feat: support starting workers using a blob: URL (#2729)
- feat: make `Deno.execPath()` a function (#2743, #2744)
- feat: support `await import(...)` syntax for dynamic module imports (#2516)
- fix: enforce permissions on `Deno.kill()`, `Deno.homeDir()` and
  `Deno.execPath()` (#2714, #2723)
- fix: `cargo build` now builds incrementally (#2740)
- fix: avoid REPL crash when DENO_DIR doesn't exist (#2727)
- fix: resolve worker module URLs relative to the host main module URL (#2751)
- doc: improve documentation on using the V8 profiler (#2742)

In deno_std:

- fix: make the 'ws' module (websockets) work again (denoland/deno_std#550)

### v0.13.0 / 2019.07.31

In deno:

- feat: add debug info to ModuleResolutionError (#2697)
- feat: expose writeAll() and writeAllSync() (#2298)
- feat: Add --current-thread flag (#2702)
- fix: REPL shouldn't panic when it gets SIGINT (#2662)
- fix: Remap stack traces of unthrown errors (#2693)
- fix: bring back --no-fetch flag (#2671)
- fix: handle deno -v and deno --version (#2684)
- fix: make importmap flag global (#2687)
- fix: timer's params length (#2655)
- perf: Remove v8::Locker calls (#2665, #2664)

In deno_std:

- fix: Make shebangs Linux compatible (denoland/deno_std#545)
- fix: Ignore error of writing responses to aborted requests
  (denoland/deno_std#546)
- fix: use Deno.execPath where possible (denoland/deno_std#548)

### v0.12.0 / 2019.07.16

In deno:

- feat: Support window.onload (#2643)
- feat: generate default file name for bundle when URL ends in a slash (#2625)
- fix: for '-' arg after script name (#2631)
- fix: upgrade v8 to 7.7.200 (#2624)

In deno_std:

- Rename catjson.ts to catj.ts (denoland/deno_std#533)
- Remove os.userHomeDir in favor of Deno.homeDir (denoland/deno_std#523)
- fix: emptydir on windows (denoland/deno_std#531)

### v0.11.0 / 2019.07.06

In deno:

- feat: Add Deno.homeDir() (#2578)
- feat: Change Reader interface (#2591)
- feat: add bash completions (#2577)
- feat: parse CLI flags after script name (#2596)
- fix: multiple error messages for a missing file (#2587)
- fix: normalize Deno.execPath (#2598)
- fix: return useful error when import path has no ./ (#2605)
- fix: run blocking function on a different task (#2570)

In deno_std:

- feat: add UUID module (denoland/deno_std#479)
- feat: prettier support reading code from stdin (denoland/deno_std#498)

### v0.10.0 / 2019.06.25

In deno:

- feat: improve module download progress (#2576)
- feat: improve 'deno install' (#2551)
- feat: log permission access with -L=info (#2518)
- feat: redirect process stdio to file (#2554)
- fix: add encodeInto to TextEncoder (#2558)
- fix: clearTimeout should convert to number (#2539)
- fix: clearTimeout.name / clearInterval.name (#2540)
- fix: event `isTrusted` is enumerable (#2543)
- fix: fetch() body now async iterable (#2563)
- fix: fetch() now handles redirects (#2561)
- fix: prevent multiple downloads of modules (#2477)
- fix: silent failure of WebAssembly.instantiate() (#2548)
- fix: urlSearchParams custom symbol iterator (#2537)

In deno_std

- feat(testing): Pretty output + Silent mode (denoland/deno_std#314)
- feat: Add os/userHomeDir (denoland/deno_std#521)
- feat: add catjson example (denoland/deno_std#517)
- feat: add encoding/hex module (denoland/deno_std#434)
- feat: improve installer (denoland/deno_std#512, denoland/deno_std#510,
  denoland/deno_std#499)
- fix: bundle/run handles Deno.args better. (denoland/deno_std#514)
- fix: file server should order filenames (denoland/deno_std#511)

### v0.9.0 / 2019.06.15

In deno:

- feat: add deno install command (#2522)
- feat: URLSearchParams should work with custom iterator (#2512)
- feat: default output filename for deno bundle (#2484)
- feat: expose window.Response (#2515)
- feat: Add --seed for setting RNG seed (#2483)
- feat: Import maps (#2360)
- fix: setTimeout API adjustments (#2511, #2497)
- fix: URL and URLSearchParams bugs (#2495, #2488)
- fix: make global request type an interface (#2503)
- upgrade: V8 to 7.7.37 (#2492)

In deno_std:

- feat: installer (denoland/deno_std#489)
- feat: bundle loader (denoland/deno_std#480)

### v0.8.0 / 2019.06.08

In deno:

- feat: Add 'bundle' subcommand. (#2467)
- feat: Handle compiler diagnostics in Rust (#2445)
- feat: add deno fmt --stdout option (#2439)
- feat: CLI defaults to run subcommand (#2451)
- fix: Compiler exit before emit if preEmitDiagnostics found (#2441)
- fix: Deno.core.evalContext & Deno.core.print (#2465)
- fix: Improve setup.py for package managers (#2423)
- fix: Use body when Request instance is passed to fetch (#2435)
- perf: Create fewer threads (#2476)
- upgrade: TypeScript to 3.5.1 (#2437)
- upgrade: std/prettier@0.5.0 to std/prettier@0.7.0 (#2425)

In deno_std:

- ci: Check file changes during test (denoland/deno_std#476)
- ci: Implement strict mode (denoland/deno_std#453)
- ci: Make CI config DRY (denoland/deno_std#470)
- encoding/csv: add easy api (denoland/deno_std#458)
- io: make port BufReader.readByte() return
  `number | EOF`(denoland/deno_std#472)
- ws: Add sec-websocket-version to handshake header (denoland/deno_std#468)

### v0.7.0 / 2019.05.29

In deno:

- TS compiler refactor (#2380)
- add EventTarget implementation (#2377)
- add module and line no for Rust logger (#2409)
- re-fix permissions for dial and listen (#2400)
- Fix concurrent accepts (#2403)
- Rename --allow-high-precision to --allow-hrtime (#2398)
- Use tagged version of prettier in CLI (#2387)

In deno_std:

- io: refactor BufReader/Writer interfaces to be more idiomatic
  (denoland/deno_std#444)
- http: add rfc7230 handling (denoland/deno_std#451)
- http: add ParseHTTPVersion (denoland/deno_std#452)
- rename strings/strings.ts to strings/mod.ts (denoland/deno_std#449)
- Prettier: support for specified files and glob mode (denoland/deno_std#438)
- Add encoding/csv (denoland/deno_std#432)
- rename bytes/bytes.ts to bytes/mod.ts
- remove function prefix of bytes module
- add bytes.repeat() (denoland/deno_std#446)
- http: fix content-length checking (denoland/deno_std#437)
- Added isGlob function (denoland/deno_std#433)
- http: send an empty response body if none is provided (denoland/deno_std#429)
- http: make server handle bad client requests properly (denoland/deno_std#419)
- fix(fileserver): wrong url href of displayed files (denoland/deno_std#426)
- http: delete conn parameter in readRequest (denoland/deno_std#430)
- Rename //multipart/multipart.ts to //mime/multipart.ts (denoland/deno_std#420)
- feat(prettier): output to stdout instead of write file by default unless
  specified --write flag (denoland/deno_std#332)

### v0.6.0 / 2019.05.20

In deno:

- Fix permissions for dial and listen (#2373)
- Add crypto.getRandomValues() (#2327)
- Don't print new line if progress bar was not used (#2374)
- Remove FileInfo.path (#2313)

In deno_std

- Clean up HTTP async iterator code (denoland/deno_std#411)
- fix: add esnext lib to tsconfig.json (denoland/deno_std#416)
- feat(fs): add copy/copySync (denoland/deno_std#278)
- feat: add Tar and Untar classes (denoland/deno_std#388)
- ws: make acceptable() more robust (denoland/deno_std#404)

### v0.5.0 / 2019.05.11

In deno:

- Add progress bar (#2309)
- fix: edge case in toAsyncIterator (#2335)
- Upgrade rust crates (#2334)
- white listed permissions (#2129 #2317)
- Add Deno.chown (#2292)

In deno_std:

- benching: use performance.now (denoland/deno_std#385)
- bytes fix bytesFindIndex and bytesFindLastIndex (denoland/deno_std#381)

### v0.4.0 / 2019.05.03

In deno:

- add "deno run" subcommand (#2215)
- add "deno xeval" subcommand (#2260)
- add --no-fetch CLI flag to prevent remote downloads (#2213)
- Fix: deno --v8-options does not print v8 options (#2277)
- Performance improvements and fix memory leaks (#2259, #2238)
- Add Request global constructor (#2253)
- fs: add Deno.utime/Deno.utimeSync (#2241)
- Make `atob` follow the spec (#2242)
- Upgrade V8 to 7.6.53 (#2236)
- Remove ? from URL when deleting all params (#2217)
- Add support for custom tsconfig.json (#2089)
- URLSearchParams init with itself (#2218)

In deno_std:

- textproto: fix invalid header error and move tests (#369)
- Add http/cookie improvements (#368, #359)
- fix ensureLink (#360)

### v0.3.10 / 2019.04.25

In deno:

- Fix "deno types" (#2209)
- CLI flags/subcommand rearrangement (#2210, #2212)

### v0.3.9 / 2019.04.25

In deno:

- Fix #2033, shared queue push bug (#2158)
- Fix panic handler (#2188)
- cli: Change "deno --types" to "deno types" and "deno --prefetch" to "deno
  prefetch" (#2157)
- Make Deno/Deno.core not deletable/writable (#2153)
- Add Deno.kill(pid, signo) and process.kill(signo) (Unix only) (#2177)
- symlink: Ignore type parameter on non-Windows platforms (#2185)
- upgrade rust crates (#2186)
- core: make Isolate concrete, remove Dispatch trait (#2183)

In deno_std:

- http: add cookie module (#338)
- fs: add getFileInfoType() (#341)
- fs: add ensureLink/ensureLinkSync (#353)
- fs: add ensureSymlink/ensureSymlinkSync (#268)
- fs: add readFileStr, writeFileStr (#276, #340)
- testing: support Sets in asserts.equals (#350)

### v0.3.8 / 2019.04.19

In deno:

- Async module loading (#2084 #2133)
- core: improve tail latency (#2131)
- third_party: upgrade rust crates
- add custom panic handler to avoid silent failures (#2098)
- fix absolute path resolution from remote (#2109)
- Add deno eval subcommand (#2102)
- fix: re-expose DomFile (#2100)
- avoid prototype builtin hasOwnProperty (#2144)

In deno_std:

- Enforce HTTP/1.1 pipeline response order (deno_std#331)
- EOL add mixed detection (deno_std#325)
- Added read file str (deno_std#276)
- add writeFileStr and update documentation (deno_std#340)

### v0.3.7 / 2019.04.11

In deno:

- Use clap for command line flag parsing (#2093, #2068, #2065, #2025)
- Allow high precision performance.now() (#1977)
- Fix `console instanceof Console` (#2073)
- Add link/linkSync fs call for hardlinks (#2074)
- build: Use -O3 instead of -O (#2070)

In deno_std:

- fs: add fs/mod.ts entry point (deno_std#272)
- prettier: change flag parsing (deno_std#327)
- fs: add EOL detect / format (deno_std#289)
- fs: ensure exists file/dir must be the same type or it will throw error
  (deno_std#294)

### v0.3.6 / 2019.04.04

In deno:

- upgrade rust crates (#2016)
- EventTarget improvements (#2019, #2018)
- Upgrade to TypeScript 3.4.1 (#2027)
- console/toString improvements (#2032, #2042, #2041, #2040)
- Add web worker JS API (#1993, #2039)
- Fix redirect module resolution bug (#2031)
- core: publish to crates.io (#2015,#2022, #2023, #2024)
- core: add RecursiveLoad for async module loading (#2034)

In deno_std:

- toml: Full support of inline table (deno_std#320)
- fix benchmarks not returning on deno 0.3.4+ (deno_std#317)

### v0.3.5 / 2019.03.28

In deno:

- Add Process.stderrOutput() (#1828)
- Check params in Event and CustomEvent (#2011, #1997)
- Merge --reload and --recompile flags (#2003)
- Add Deno.openSync, .readSync, .writeSync, .seekSync (#2000)
- Do not close file on invalid seek mode (#2004)
- Fix bug when shared queue is overflowed (#1992)
- core: Resolve callback moved from Behavior to mod_instantiate() (#1999)
- core: libdeno and DenoCore renamed to Deno.core (#1998)
- core: Allow terminating an Isolate from another thread (#1982)

In deno_std:

- Add TOML parsing module (#300)
- testing: turn off exitOnFail by default (#307, #309)
- Fix assertEquals for RegExp & Date (#305)
- Fix prettier check in empty files (#302)
- remove unnecessary path.resolve in move/readJson/writeJson (#292)
- fix: fs.exists not work for symlink (#291)
- Add prettier styling options (#281)

### v0.3.4 / 2019.03.20

In deno itself:

- Performance improvements (#1959, #1938)
- Improve pretty printing of objects (#1969)
- More permissions prompt options (#1926)

In deno_std:

- Add prettier styling options (#281)
- Extract internal method isSubdir to fs/utils.ts (#285)
- Add strings/pad (#282)

### v0.3.3 / 2019.03.13

In deno itself:

- Rename Deno.build.gnArgs to Deno.build.args (#1912, #1909)
- Upgrade to TypeScript 3.3 (#1908)
- Basic Arm64 support (#1887)
- Remove builtin "deno" module, use Deno global var (#1895)
- Improvements to internal deno_core crate (#1904, #1914)
- Add --no-prompt flag for non-interactive environments (#1913)

In deno_std

- Add fs extras: ensureDir, ensureFile, readJson, emptyDir, move, exists (#269,
  #266, #264, #263, #260)
- Datetime module improvement (#259)
- asserts: Add unimplemented, unreachable, assertNotEquals, assertArrayContains
  (#246, #248)

### v0.3.2 / 2019.03.06

In deno itself:

- Reorganize version and platform into Deno.build and Deno.version (#1879)
- Allow inspection and revocation of permissions (#1875)
- Fix unicode output on Windows (#1876)
- Add Deno.build.gnArgs (#1845)
- Fix security bug #1858 (#1864, #1874)
- Replace deno.land/x/std links with deno.land/std/ (#1890)

In deno_std:

- Move asserts out of testing/mod.ts into testing/assert.ts Rename assertEqual
  to assertEquals (#240, #242)
- Update mime-db to 1.38.0 (#238)
- Use pretty assertEqual in testing (#234)
- Add eslint to CI (#235)
- Refactor WebSockets (#173)
- Allow for parallel testing (#224)
- testing: use color module for displaying colors (#223)
- Glob integration for the FS walker (#219)

### v0.3.1 / 2019.02.27

- Add import.meta.main (#1835)
- Fix console.table display of Map (#1839)
- New low-level Rust API (#1827)
- Upgrade V8 to 7.4.238 (#1849)
- Upgrade crates (#1848)

### v0.3.0 / 2019.02.18

The major API change in this release is that instead of importing a `"deno"`
module, there is now a global variable called `Deno`. This allows code that does
deno-specific stuff to still operate in browsers. We will remain backward
compatible with the old way of importing core functionality, but it will be
removed in the near future, so please update your code. See #1748 for more
details.

- Add Deno global namespace object (#1748)
- Add window.location (#1761)
- Add back typescript version number and add Deno.version object (#1788)
- Add `seek` and implement `Seeker` on `File` (#1797)
- Add Deno.execPath (#1743)
- Fix behavior for extensionless files with .mime file (#1779)
- Add env option in Deno.run (#1773)
- Turn on `v8_postmortem_support` (#1758)
- Upgrade V8 to 7.4.158 (#1767)
- Use proper directory for cache files (#1763)
- REPL multiline support with recoverable errors (#1731)
- Respect `NO_COLOR` in TypeScript output (#1736)
- Support scoped variables, unblock REPL async op, and REPL error colors (#1721)

### v0.2.11 / 2019.02.08

- Add deps to --info output (#1720)
- Add --allow-read (#1689)
- Add deno.isTTY() (#1622)
- Add emojis to permission prompts (#1684)
- Add basic WebAssembly support (#1677)
- Add `NO_COLOR` support https://no-color.org/ (#1716)
- Add color exceptions (#1698)
- Fix: do not load cache files when recompile flag is set (#1695)
- Upgrade V8 to 7.4.98 (#1640)

### v0.2.10 / 2019.02.02

- Add --fmt (#1646)
- Add --info (#1647, #1660)
- Better error message for bad filename CLI argument. (#1650)
- Clarify writeFile options and avoid unexpected perm modification (#1643)
- Add performance.now (#1633)
- Add import.meta.url (#1624)

### v0.2.9 / 2019.01.29

- Add REPL functions "help" and "exit" (#1563)
- Split out compiler snapshot (#1566)
- Combine deno.removeAll into deno.remove (#1596)
- Add console.table (#1608)
- Add console.clear() (#1562)
- console output with format (#1565)
- env key/value should both be strings (#1567)
- Add CustomEvent API (#1505)

### v0.2.8 / 2019.01.19

- Add --prefetch flag for deps prefetch without running (#1475)
- Kill all pending accepts when TCP listener is closed (#1517)
- Add globalThis definition to runtime (#1534)
- mkdir should not be recursive by default (#1530)
- Avoid crashes on ES module resolution when module not found (#1546)

### v0.2.7 / 2019.01.14

- Use rust 2018 edition
- Native ES modules (#1460 #1492 #1512 #1514)
- Properly parse network addresses (#1515)
- Added rid to Conn interface (#1513)
- Prevent segfault when eval throws an error (#1411)
- Add --allow-all flag (#1482)

### v0.2.6 / 2019.01.06

- Implement console.groupCollapsed (#1452)
- Add deno.pid (#1464)
- Add Event web API (#1059)
- Support more fetch init body types (#1449)

### v0.2.5 / 2018.12.31

- Runtime argument checks (#1427 #1415)
- Lazily create .mime files only with mismatch/no extension (#1417)
- Fix FormData.name (#1412)
- Print string with NULL '\0' (#1428)

### v0.2.4 / 2018.12.23

- "cargo build" support (#1369 #1296 #1377 #1379)
- Remove support for extensionless import (#1396)
- Upgrade V8 to 7.2.502.16 (#1403)
- make stdout unbuffered (#1355)
- Implement `Body.formData` for fetch (#1393)
- Improve handling of non-coercible objects in assertEqual (#1385)
- Avoid fetch segfault on empty Uri (#1394)
- Expose deno.inspect (#1378)
- Add illegal header name and value guards (#1375)
- Fix URLSearchParams set() and constructor() (#1368)
- Remove prebuilt v8 support (#1369)
- Enable jumbo build in release. (#1362)
- Add URL implementation (#1359)
- Add console.count and console.time (#1358)
- runtime arg check `URLSearchParams` (#1390)

### v0.2.3 / 2018.12.14

- console.assert should not throw error (#1335)
- Support more modes in deno.open (#1282, #1336)
- Simplify code fetch logic (#1322)
- readDir entry mode (#1326)
- Use stderr for exceptions (#1303)
- console.log formatting improvements (#1327, #1299)
- Expose TooLarge error code for buffers (#1298)

### v0.2.2 / 2018.12.07

- Don't crash when .mime file not exist in cache (#1291)
- Process source maps in Rust instead of JS (#1280)
- Use alternate TextEncoder/TextDecoder implementation (#1281)
- Upgrade flatbuffers to 80d148
- Fix memory leaks (#1265, #1275)

### v0.2.1 / 2018.11.30

- Allow async functions in REPL (#1233)
- Handle Location header relative URI (#1240)
- Add deno.readAll() (#1234)
- Add Process.output (#1235)
- Upgrade to TypeScript 3.2.1
- Upgrade crates: tokio 0.1.13, hyper 0.12.16, ring 0.13.5

### v0.2.0 / 2018.11.27 / Mildly usable

[An intro talk was recorded.](https://www.youtube.com/watch?v=FlTG0UXRAkE)

Stability and usability improvements. `fetch()` is 90% functional now. Basic
REPL support was added. Shebang support was added. Command-line argument parsing
was improved. A forwarding service `https://deno.land/x` was set up for Deno
code. Example code has been posted to
[deno.land/x/examples](https://github.com/denoland/deno_examples) and
[deno.land/x/net](https://github.com/denoland/net).

The resources table was added to abstract various types of I/O streams and other
allocated state. A resource is an integer identifier which maps to some Rust
object. It can be used with various ops, particularly read and write.

Changes since v0.1.12:

- First pass at running subprocesses (#1156)
- Improve flag parsing (#1200)
- Improve fetch() (#1194 #1188 #1102)
- Support shebang (#1197)

### v0.1.12 / 2018.11.12

- Update to TypeScript 3.1.6 (#1177)
- Fixes Headers type not available. (#1175)
- Reader/Writer to use Uint8Array not ArrayBufferView (#1171)
- Fixes importing modules starting with 'http'. (#1167)
- build: Use target/ instead of out/ (#1153)
- Support repl multiline input (#1165)

### v0.1.11 / 2018.11.05

- Performance and stability improvements on all platforms.
- Add repl (#998)
- Add deno.Buffer (#1121)
- Support cargo check (#1128)
- Upgrade Rust crates and Flatbuffers. (#1145, #1127)
- Add helper to turn deno.Reader into async iterator (#1130)
- Add ability to load JSON as modules (#1065)
- Add deno.resources() (#1119)
- Add application/x-typescript mime type support (#1111)

### v0.1.10 / 2018.10.27

- Add URLSearchParams (#1049)
- Implement clone for FetchResponse (#1054)
- Use content-type headers when importing from URLs. (#1020)
- Use checkJs option, JavaScript will be type checked and users can supply JSDoc
  type annotations that will be enforced by Deno (#1068)
- Add separate http/https cache dirs to DENO_DIR (#971)
- Support https in fetch. (#1100)
- Add chmod/chmodSync on unix (#1088)
- Remove broken features: --deps and trace() (#1103)
- Ergonomics: Prompt TTY for permission escalation (#1081)

### v0.1.9 / 2018.10.20

- Performance and stability improvements on all platforms.
- Add cwd() and chdir() #907
- Specify deno_dir location with env var DENO_DIR #970
- Make fetch() header compliant with the current spec #1019
- Upgrade TypeScript to 3.1.3
- Upgrade V8 to 7.1.302.4

### v0.1.8 / 2018.10.12 / Connecting to Tokio / Fleshing out APIs

Most file system ops were implemented. Basic TCP networking is implemented.
Basic stdio streams exposed. And many random OS facilities were exposed (e.g.
environmental variables)

Tokio was chosen as the backing event loop library. A careful mapping of JS
Promises onto Rust Futures was made, preserving error handling and the ability
to execute synchronously in the main thread.

Continuous benchmarks were added: https://denoland.github.io/deno/ Performance
issues are beginning to be addressed.

"deno --types" was added to reference runtime APIs.

Working towards https://github.com/denoland/deno/milestone/2 We expect v0.2 to
be released in last October or early November.

Changes since v0.1.7:

- Fix promise reject issue (#936)
- Add --types command line flag.
- Add metrics()
- Add redirect follow feature #934
- Fix clearTimer bug #942
- Improve error printing #935
- Expose I/O interfaces Closer, Seeker, ReaderCloser, WriteCloser, ReadSeeker,
  WriteSeeker, ReadWriteCloser, ReadWriteSeeker
- Fix silent death on double await #919
- Add Conn.closeRead() and Conn.closeWrite() #903

### v0.1.7 / 2018.10.04

- Improve fetch headers (#853)
- Add deno.truncate (#805)
- Add copyFile/copyFileSync (#863)
- Limit depth of output in console.log for nested objects, and add console.dir
  (#826)
- Guess extensions on extension not provided (#859)
- Renames: deno.platform -> deno.platform.os deno.arch -> deno.platform.arch
- Upgrade TS to 3.0.3
- Add readDirSync(), readDir()
- Add support for TCP servers and clients. (#884) Adds deno.listen(),
  deno.dial(), deno.Listener and deno.Conn.

### v0.1.6 / 2018.09.28

- Adds deno.stdin, deno.stdout, deno.stderr, deno.open(), deno.write(),
  deno.read(), deno.Reader, deno.Writer, deno.copy() #846
- Print 'Compiling' when compiling TS.
- Support zero-copy for writeFile() writeFileSync() #838
- Fixes eval error bug #837
- Make Deno multithreaded #782
- console.warn() goes to stderr #810
- Add deno.readlink()/readlinkSync() #797
- Add --recompile flag #801
- Use constructor.name to print out function type #664
- Rename deno.argv to deno.args
- Add deno.trace() #795
- Continuous benchmarks

### v0.1.5 / 2018.09.21

- Add atob() btoa() #776
- Add deno.arch deno.platform #773
- Add deno.symlink() and deno.symlinkSync() #742
- Add deno.mkdir() and deno.mkdirSync() #746
- Add deno.makeTempDir() #740
- Improvements to FileInfo interface #765, #761
- Add fetch.blob()
- Upgrade V8 to 7.0.276.15
- Upgrade Rust crates

### v0.1.4 / 2018.09.12

- Support headers in fetch()
- Adds many async fs functions: deno.rename() deno.remove(), deno.removeAll(),
  deno.removeSync(), deno.removeAllSync(), deno.mkdir(), deno.stat(),
  deno.lstat() deno.readFile() and deno.writeFile().
- Add mode in FileInfo
- Access error codes via error.kind
- Check --allow-net permissions when using fetch()
- Add deno --deps for listing deps of a script.

### v0.1.3 / 2018.09.05 / Scale binding infrastructure

ETA v.0.2 October 2018 https://github.com/denoland/deno/milestone/2

We decided to use Tokio https://tokio.rs/ to provide asynchronous I/O, thread
pool execution, and as a base for high level support for various internet
protocols like HTTP. Tokio is strongly designed around the idea of Futures -
which map quite well onto JavaScript promises. We want to make it as easy as
possible to start a Tokio future from JavaScript and get a Promise for handling
it. We expect this to result in preliminary file system operations, fetch() for
http. Additionally we are working on CI, release, and benchmarking
infrastructure to scale development.

Changes since v0.1.2:

- Fixes module resolution error #645
- Better flag parsing
- lStatSync -> lstatSync
- Added deno.renameSync()
- Added deno.mkdirSync()
- Fix circular dependencies #653
- Added deno.env() and --allow-env

### v0.1.2 / 2018.08.30

- Added https import support.
- Added deno.makeTempDirSync().
- Added deno.lstatSync() and deno.statSync().

### v0.1.1 / 2018.08.27

### v0.1.0 / 2018.08.23 / Rust rewrite and V8 snapshot

Complete! https://github.com/denoland/deno/milestone/1

Go is a garbage collected language and we are worried that combining it with
V8's GC will lead to difficult contention problems down the road.

The V8Worker2 binding/concept is being ported to a new C++ library called
libdeno. libdeno will include the entire JS runtime as a V8 snapshot. It still
follows the message passing paradigm. Rust will be bound to this library to
implement the privileged part of deno. See deno2/README.md for more details.

V8 Snapshots allow deno to avoid recompiling the TypeScript compiler at startup.
This is already working.

When the rewrite is at feature parity with the Go prototype, we will release
binaries for people to try.

### v0.0.0 / 2018.05.14 - 2018.06.22 / Golang Prototype

https://github.com/denoland/deno/tree/golang

https://www.youtube.com/watch?v=M3BM9TB-8yA

https://tinyclouds.org/jsconf2018.pdf

### 2007-2017 / Prehistory

https://github.com/ry/v8worker

https://libuv.org/

https://tinyclouds.org/iocp-links.html

https://nodejs.org/

https://github.com/nodejs/http-parser

https://tinyclouds.org/libebb/
