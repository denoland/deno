// Copyright 2018-2026 the Deno authors. MIT license.
//! The "Permission options" section shown at the end of `--help` for every
//! command that accepts permission flags.
//!
//! The permission args themselves are hidden from the generated options table
//! (as they were under clap) because they are documented here instead, with
//! usage examples and the permission-related environment variables.

pub static PERMISSION_HELP: &str = "Permission options:
Docs: https://docs.deno.com/go/permissions

  -A, --allow-all                           Allow all permissions.
  -P, --permission-set[=<NAME>]             Loads the permission set from the config file.
  --no-prompt                               Always throw if required permission wasn't passed.
                                            Can also be set via the DENO_NO_PROMPT environment variable.
  -R, --allow-read[=<PATH>...]              Allow file system read access. Optionally specify allowed paths.
                                            --allow-read  |  --allow-read=\"/etc,/var/log.txt\"
  -W, --allow-write[=<PATH>...]             Allow file system write access. Optionally specify allowed paths.
                                            --allow-write  |  --allow-write=\"/etc,/var/log.txt\"
  -I, --allow-import[=<IP_OR_HOSTNAME>...]  Allow importing from remote hosts. Optionally specify allowed IP addresses and host names, with ports as necessary.
                                            Default value: deno.land:443,jsr.io:443,esm.sh:443,raw.esm.sh:443,cdn.jsdelivr.net:443,raw.githubusercontent.com:443,gist.githubusercontent.com:443
                                            --allow-import  |  --allow-import=\"example.com,github.com\"
  -N, --allow-net[=<IP_OR_HOSTNAME>...]     Allow network access. Optionally specify allowed IP addresses and host names, with ports as necessary. A Unix domain socket can be scoped with unix:<absolute-path>.
                                            --allow-net  |  --allow-net=\"localhost:8080,deno.land\"  |  --allow-net=\"unix:/var/run/docker.sock\"
  -E, --allow-env[=<VARIABLE_NAME>...]      Allow access to environment variables. Optionally specify accessible environment variables.
                                            --allow-env  |  --allow-env=\"PORT,HOME,PATH\"
  -S, --allow-sys[=<API_NAME>...]           Allow access to OS information. Optionally allow specific APIs by function name.
                                            --allow-sys  |  --allow-sys=\"systemMemoryInfo,osRelease\"
  --allow-run[=<PROGRAM_NAME>...]           Allow running subprocesses. Optionally specify allowed runnable program names.
                                            --allow-run  |  --allow-run=\"whoami,ps\"
  --allow-ffi[=<PATH>...]                   (Unstable) Allow loading dynamic libraries. Optionally specify allowed directories or files.
                                            --allow-ffi  |  --allow-ffi=\"./libfoo.so\"
  --deny-read[=<PATH>...]                   Deny file system read access. Optionally specify denied paths.
                                            --deny-read  |  --deny-read=\"/etc,/var/log.txt\"
  --deny-write[=<PATH>...]                  Deny file system write access. Optionally specify denied paths.
                                            --deny-write  |  --deny-write=\"/etc,/var/log.txt\"
  --deny-net[=<IP_OR_HOSTNAME>...]          Deny network access. Optionally specify defined IP addresses and host names, with ports as necessary.
                                            --deny-net  |  --deny-net=\"localhost:8080,deno.land\"
  --deny-env[=<VARIABLE_NAME>...]           Deny access to environment variables. Optionally specify inacessible environment variables.
                                            --deny-env  |  --deny-env=\"PORT,HOME,PATH\"
  --deny-sys[=<API_NAME>...]                Deny access to OS information. Optionally deny specific APIs by function name.
                                            --deny-sys  |  --deny-sys=\"systemMemoryInfo,osRelease\"
  --deny-run[=<PROGRAM_NAME>...]            Deny running subprocesses. Optionally specify denied runnable program names.
                                            --deny-run  |  --deny-run=\"whoami,ps\"
  --deny-ffi[=<PATH>...]                    (Unstable) Deny loading dynamic libraries. Optionally specify denied directories or files.
                                            --deny-ffi  |  --deny-ffi=\"./libfoo.so\"
  --deny-import[=<IP_OR_HOSTNAME>...]       Deny importing from remote hosts. Optionally specify denied IP addresses and host names, with ports as necessary.
                                            --deny-import  |  --deny-import=\"example.com:443,github.com:443\"
  --ignore-env[=<VARIABLE_NAME>...]         Ignore access to environment variables returning `undefined`. Optionally specify ignored environment variables.
                                            --ignore-env  |  --ignore-env=\"PORT,HOME,PATH\"
  --ignore-read[=<PATH>...]                 Ignore file system read access with a `NotFound` error. Optionally specify ignored paths.
                                            --ignore-read  |  --ignore-read=\"/etc,/var/log.txt\"
  DENO_TRACE_PERMISSIONS                    Environmental variable to enable stack traces in permission prompts.
                                            DENO_TRACE_PERMISSIONS=1 deno run main.ts
  DENO_AUDIT_PERMISSIONS                    Environmental variable to audit all permissions accesses. Set to a file path for JSONL output, or \"otel\" to emit as OpenTelemetry log events via the configured OTel exporter.
                                            DENO_AUDIT_PERMISSIONS=./audit.jsonl deno run main.ts
                                            DENO_AUDIT_PERMISSIONS=otel deno run main.ts
";
