# Releases

Binary releases can be downloaded manually at
https://github.com/denoland/deno/releases

We also have one-line install commands at
https://github.com/denoland/deno_install

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
- feat: Prvode way to build Deno without building V8 from source (#4412)
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
- feat: Make internel error frames dimmer (#4201)
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
- refactor: reduce unnecesarry output in cli/js tests (#4182)
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
- fix: Move WebAsssembly namespace to shared_globals (#4084)
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
- fix: Cherry-pick depot_tools 6a1d778 to fix macOS Cataliona issues (#3175)
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
- fix: [tls] op_dial_tls is not registerd and broken (#3121)
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
- fix: listenDefaults/dialDefaults may be overriden in some cases (#3027)
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
