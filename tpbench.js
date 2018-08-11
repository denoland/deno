const assert = require("assert");
const { fork } = require("child_process");
const {
  openSync,
  closeSync,
  read,
  readSync,
  unlinkSync,
  writeFileSync
} = require("fs");
const os = require("os"),
  { cpus, tmpdir } = os;
//const { cpus, release, tmpdir, type } = require("os");
const { resolve } = require("path");
const { promisify } = require("util");
const readAsync = promisify(read);

const ROUNDS = 3; // Number of benchmark rounds.
const THREADPOOL_SIZES = [1, 2, 4, 8];
const CONCURRENCIES = [0, 1, 2, 4, 8, 16, 32, 64]; // # Concurrency values to benchmark.
const WARMUP_DURATION = 0.5; // Warm-up time before a single benchmark, in seconds.
const BENCH_DURATION = 2; // Duration of a single benchmark, in seconds.
const TIME_CHECKS = 5; // Check the time N times during benchmarking.
const INITIAL_GUESS_RATE = 10000; // Before we know better, assume N ops/sec.
const READ_LENGTH = 1; // The number of bytes to read from the file.

const WARMUP_PHASE = "W";
const BENCH_PHASE = "B";

async function run_child() {
  const round = +process.env.ROUND;
  const threadpool_size = +process.env.UV_THREADPOOL_SIZE;

  for (let concurrency of CONCURRENCIES) {
    await bench(round, threadpool_size, concurrency);
  }
}

async function run_master() {
  printSysInfo();
  print("\n");

  for (let round = 0; round < ROUNDS; round++) {
    print(`round #${round}\n`);
    for (let threadpool_size of THREADPOOL_SIZES) {
      print(`  threadpool_size: ${threadpool_size}\n`);
      await forkAsync({
        FORK: true,
        ROUND: round,
        UV_THREADPOOL_SIZE: threadpool_size
      });
    }
    print("\n");
  }
}

function run() {
  if (process.env.FORK) run_child();
  else run_master();
}

let baseline_rate;

async function bench(round, threadpool_size, concurrency) {
  let read_fn;
  let thread_count;

  if (concurrency) {
    // Async benchmark. `concurrency` is the number of parallel 'threads'.
    read_fn = readAsync;
    thread_count = concurrency;
  } else {
    // Benchmark `readSync` in a single thread.
    read_fn = readSync;
    thread_count = 1;
  }

  // Log benchmark info.
  // print(`round #` + align(round, ROUNDS));
  // print(`    threadpool_size: ` + align(threadpool_size, THREADPOOL_SIZES));
  print(
    `    ` +
      align(concurrency, CONCURRENCIES, c => (c ? `async(${c})` : `sync`))
  );

  let phase; // Phase: WARMUP_PHASE or BENCH_PHASE.
  let rate = INITIAL_GUESS_RATE; // In op/second. A new estimate is computed after each interval.
  let interval; // Time is checked after each interval. Increments up to TIME_CHECKS.
  let interval_duration; // Target average duration of a single interval.
  let interval_target_ops; // Interval ends when `ops === interval_target_ops`.
  let start_time; // Start (hr)time of current phase, in [sec, nsec].
  let elapsed; // Actual time duration of current phase, in seconds.
  let ops; // Ops completed so far in the current phase.

  function beginPhase(phase_) {
    phase = phase_;
    const phase_duration =
      phase === WARMUP_PHASE ? WARMUP_DURATION : BENCH_DURATION;
    interval = 0;
    interval_duration = phase_duration / TIME_CHECKS;
    interval_target_ops = rate * interval_duration;
    start_time = process.hrtime();
    ops = 0;
  }

  beginPhase(WARMUP_PHASE);

  const threads = new Array(thread_count).fill(null).map(thread);
  await Promise.all(threads); // Wait for all threads to finish.

  printStats(ops, elapsed, rate);

  async function thread() {
    const fd = acquireTestFd();
    let buf = new Uint8Array(READ_LENGTH);

    for (;;) {
      // Execute ops in a loop until the intermediate target is reached.
      while (ops < interval_target_ops) {
        await read_fn(fd, buf, 0, buf.byteLength, 0);
        ++ops;
      }

      // Exit if another thread already crossed the finish line.
      if (interval == TIME_CHECKS) {
        break;
      }

      // Measure time and compute new rate estimate.
      const now = process.hrtime();
      elapsed = now[0] - start_time[0] + (now[1] - start_time[1]) / 1e9;
      rate = ops / elapsed;
      if (!concurrency) baseline_rate = rate;
      // Debug.
      // print(`    phase: ${phase}    interval: #${interval}`
      // print_stats(ops, elapsed, rate);

      if (++interval < TIME_CHECKS) {
        // Moved to the next time check interval. Set the ops target for it.
        interval_target_ops = rate * interval_duration * (interval + 1);
      } else if (phase === WARMUP_PHASE) {
        // Warm-up phase complete, transition to benchmark phase.
        beginPhase(BENCH_PHASE);
      } else {
        // Benchmark phase complete. Stop thread.
        break;
      }
    }
    releaseTestFd(fd);
  }

  function printStats() {
    print(`    ops: ${ops.toFixed(0).padStart(7)}`);
    print(`    time: ${elapsed.toFixed(4).padStart(6)}s`);
    print(`    rate: ${rate.toFixed(0).padStart(7)}/s`);
    if (rate > baseline_rate) {
      print(`    ${(rate / baseline_rate).toFixed(2).padStart(5)}x FASTER`);
    } else if (rate < baseline_rate) {
      print(`    ${(baseline_rate / rate).toFixed(2).padStart(5)}x slower`);
    }
    print("\n");
  }
}

function print(s) {
  process.stdout.write(s);
}

function align(input, other, map = v => v) {
  const width = Math.max(
    ...[]
      .concat(other)
      .map(map)
      .map(v => String(v).length)
  );
  return String(map(input)).padEnd(width);
}

function printSysInfo() {
  print(`Node ${process.version} ${process.arch}\n`);
  print(`${os.type()} ${os.release}\n`);
  print(
    Object.entries(
      os
        .cpus()
        .map(cpu => cpu.model)
        .reduce((acc, m) => ({ ...acc, [m]: 1 + (acc[m] || 0) }), {})
    )
      .map(([model, count]) => `cpu: ${count} x ${model}\n`)
      .join("")
  );
  const gb = b => (b / (1 << 30)).toFixed(1);
  print(`mem: ${gb(os.totalmem())}GB (free: ${gb(os.freemem())}GB)\n`);
}

function forkAsync(merge_env) {
  return new Promise((res, rej) => {
    const child = fork(__filename, { env: { ...process.env, ...merge_env } });
    child.on("error", rej);
    child.on("close", res);
  });
}

let test_files = [];
let test_files_avail_fd = [];

function createTestFile() {
  let unique = Math.random()
    .toString(36)
    .slice(2);
  const filename = resolve(tmpdir(), `tpbench-${unique}.tmp`);
  writeFileSync(filename, "x".repeat(READ_LENGTH));
  const fd = openSync(filename, "r");
  test_files.push({ fd, filename });
  return fd;
}

function removeTestFile({ fd, filename }) {
  closeSync(fd);
  unlinkSync(filename);
}

function removeTestFiles() {
  // console.error("\ncleaning up...");
  test_files.forEach(removeTestFile);
  test_files = [];
  test_files_avail_fd = [];
}

function acquireTestFd() {
  for (let fd = test_files_avail_fd.pop(); fd !== undefined; ) {
    return fd;
  }
  return createTestFile();
}

function releaseTestFd(fd) {
  test_files_avail_fd.push(fd);
}

process.on("exit", removeTestFiles);
process.on("SIGINT", () => process.exit());

run();
