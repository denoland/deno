# Releases

Binary releases can be downloaded manually at
https://github.com/denoland/deno/releases

We also have one-line install commands at
https://github.com/denoland/deno_install

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
- fix: add exnext lib to tsconfig.json (denoland/deno_std#416)
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
- Improve handling of non-coercable objects in assertEqual (#1385)
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
- Continuous benchmarks https://denoland.github.io/deno/

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
