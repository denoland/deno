// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { File } = window.__bootstrap.files;
  const { close } = window.__bootstrap.resources;
  const { readAll } = window.__bootstrap.buffer;
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const { assert, pathFromURL } = window.__bootstrap.util;

  function opKill(pid, signo) {
    sendSync("op_kill", { pid, signo });
  }

  function opRunStatus(rid) {
    return sendAsync("op_run_status", { rid });
  }

  function opRun(request) {
    assert(request.cmd.length > 0);
    return sendSync("op_run", request);
  }

  async function runStatus(rid) {
    const res = await opRunStatus(rid);

    if (res.gotSignal) {
      const signal = res.exitSignal;
      return { success: false, code: 128 + signal, signal };
    } else if (res.exitCode != 0) {
      return { success: false, code: res.exitCode };
    } else {
      return { success: true, code: 0 };
    }
  }

  class Process {
    constructor(res) {
      this.rid = res.rid;
      this.pid = res.pid;

      if (res.stdinRid && res.stdinRid > 0) {
        this.stdin = new File(res.stdinRid);
      }

      if (res.stdoutRid && res.stdoutRid > 0) {
        this.stdout = new File(res.stdoutRid);
      }

      if (res.stderrRid && res.stderrRid > 0) {
        this.stderr = new File(res.stderrRid);
      }
    }

    status() {
      return runStatus(this.rid);
    }

    async output() {
      if (!this.stdout) {
        throw new TypeError("stdout was not piped");
      }
      try {
        return await readAll(this.stdout);
      } finally {
        this.stdout.close();
      }
    }

    async stderrOutput() {
      if (!this.stderr) {
        throw new TypeError("stderr was not piped");
      }
      try {
        return await readAll(this.stderr);
      } finally {
        this.stderr.close();
      }
    }

    close() {
      close(this.rid);
    }

    kill(signo) {
      opKill(this.pid, signo);
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
    if (cmd[0] != null) {
      cmd[0] = pathFromURL(cmd[0]);
    }
    const res = opRun({
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

  window.__bootstrap.process = {
    run,
    Process,
    kill: opKill,
  };
})(this);
