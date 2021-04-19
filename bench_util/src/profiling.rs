use bencher::{DynBenchFn, StaticBenchFn, TestDescAndFn, TestOpts};

pub fn is_profiling() -> bool {
  std::env::var("PROFILING").is_ok()
}

#[macro_export]
// Tweaked and copied from https://github.com/bluss/bencher/blob/master/macros.rs
macro_rules! bench_or_profile {
  ($($group_name:path),+) => {
    fn main() {
      use $crate::bencher::TestOpts;
      use $crate::bencher::run_tests_console;
      let mut test_opts = TestOpts::default();
      // check to see if we should filter:
      if let Some(arg) = ::std::env::args().skip(1).find(|arg| *arg != "--bench") {
          test_opts.filter = Some(arg);
      }
      let mut benches = Vec::new();
      $(
          benches.extend($group_name());
      )+

      if $crate::is_profiling() {
        // Run profling
        $crate::run_profiles(&test_opts, benches);
      } else {
        // Run benches
        run_tests_console(&test_opts, benches).unwrap();
      }
    }
  };
  ($($group_name:path,)+) => {
      bench_or_profile!($($group_name),+);
  };
}

pub fn run_profiles(opts: &TestOpts, tests: Vec<TestDescAndFn>) {
  let tests = filter_tests(opts, tests);
  // let decs = tests.iter().map(|t| t.desc.clone()).collect();

  println!();
  for b in tests {
    println!("Profiling {}", b.desc.name);
    run_profile(b);
  }
  println!();
}

fn run_profile(test: TestDescAndFn) {
  match test.testfn {
    DynBenchFn(bencher) => {
      bencher::bench::run_once(|harness| bencher.run(harness));
    }
    StaticBenchFn(benchfn) => {
      bencher::bench::run_once(|harness| benchfn(harness));
    }
  };
}

// Copied from https://github.com/bluss/bencher/blob/master/lib.rs
fn filter_tests(
  opts: &TestOpts,
  tests: Vec<TestDescAndFn>,
) -> Vec<TestDescAndFn> {
  let mut filtered = tests;

  // Remove tests that don't match the test filter
  filtered = match opts.filter {
    None => filtered,
    Some(ref filter) => filtered
      .into_iter()
      .filter(|test| test.desc.name.contains(&filter[..]))
      .collect(),
  };

  // Sort the tests alphabetically
  filtered.sort_by(|t1, t2| t1.desc.name.cmp(&t2.desc.name));

  filtered
}
