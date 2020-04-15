System.register(
  "$deno$/process.ts",
  [
    "$deno$/files.ts",
    "$deno$/ops/resources.ts",
    "$deno$/buffer.ts",
    "$deno$/ops/process.ts",
  ],
  function (exports_52, context_52) {
    "use strict";
    let files_ts_2, resources_ts_4, buffer_ts_1, process_ts_1, Process;
    const __moduleName = context_52 && context_52.id;
    async function runStatus(rid) {
      const res = await process_ts_1.runStatus(rid);
      if (res.gotSignal) {
        const signal = res.exitSignal;
        return { signal, success: false };
      } else {
        const code = res.exitCode;
        return { code, success: code === 0 };
      }
    }
    function isRid(arg) {
      return !isNaN(arg);
    }
    function run({
      cmd,
      cwd = undefined,
      env = {},
      stdout = "inherit",
      stderr = "inherit",
      stdin = "inherit",
    }) {
      const res = process_ts_1.run({
        cmd: cmd.map(String),
        cwd,
        env: Object.entries(env),
        stdin: isRid(stdin) ? "" : stdin,
        stdout: isRid(stdout) ? "" : stdout,
        stderr: isRid(stderr) ? "" : stderr,
        stdinRid: isRid(stdin) ? stdin : 0,
        stdoutRid: isRid(stdout) ? stdout : 0,
        stderrRid: isRid(stderr) ? stderr : 0,
      });
      return new Process(res);
    }
    exports_52("run", run);
    return {
      setters: [
        function (files_ts_2_1) {
          files_ts_2 = files_ts_2_1;
        },
        function (resources_ts_4_1) {
          resources_ts_4 = resources_ts_4_1;
        },
        function (buffer_ts_1_1) {
          buffer_ts_1 = buffer_ts_1_1;
        },
        function (process_ts_1_1) {
          process_ts_1 = process_ts_1_1;
        },
      ],
      execute: function () {
        Process = class Process {
          // @internal
          constructor(res) {
            this.rid = res.rid;
            this.pid = res.pid;
            if (res.stdinRid && res.stdinRid > 0) {
              this.stdin = new files_ts_2.File(res.stdinRid);
            }
            if (res.stdoutRid && res.stdoutRid > 0) {
              this.stdout = new files_ts_2.File(res.stdoutRid);
            }
            if (res.stderrRid && res.stderrRid > 0) {
              this.stderr = new files_ts_2.File(res.stderrRid);
            }
          }
          status() {
            return runStatus(this.rid);
          }
          async output() {
            if (!this.stdout) {
              throw new Error("Process.output: stdout is undefined");
            }
            try {
              return await buffer_ts_1.readAll(this.stdout);
            } finally {
              this.stdout.close();
            }
          }
          async stderrOutput() {
            if (!this.stderr) {
              throw new Error("Process.stderrOutput: stderr is undefined");
            }
            try {
              return await buffer_ts_1.readAll(this.stderr);
            } finally {
              this.stderr.close();
            }
          }
          close() {
            resources_ts_4.close(this.rid);
          }
          kill(signo) {
            process_ts_1.kill(this.pid, signo);
          }
        };
        exports_52("Process", Process);
      },
    };
  }
);
