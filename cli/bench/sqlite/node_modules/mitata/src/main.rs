mod lib;

fn main() {
  let mitata = clap::Command::new("mitata");

  let matches = mitata
    .arg_required_else_help(true)
    .arg(clap::Arg::new("benchmark").required(true).forbid_empty_values(true).multiple_occurrences(true)).get_matches();

  let benchmarks: Vec<String> = matches.values_of("benchmark").unwrap().map(|x| x.to_string()).collect();
  let mut options = lib::reporter::Options::new(&benchmarks.iter().map(|x| x.as_str()).collect::<Vec<&str>>());

  options.percentiles = false;
  let mut results = Vec::with_capacity(benchmarks.len());

  println!("{}", lib::fmt::color(&format!("cpu: {}", lib::cpu::name()), lib::fmt::Color::Gray));
  println!("{}\n", lib::fmt::color(&format!("runtime: shell ({})", env!("TARGET")), lib::fmt::Color::Gray));

  println!("{}", lib::reporter::header(&options));

  println!("{}", lib::reporter::br(&options));

  for cmd in benchmarks {
    let mut args = cmd.split(' ');
    let name = args.next().unwrap();

    let stats = lib::bench::sync(std::time::Duration::from_millis(500), || {
      let mut cmd = std::process::Command::new(name);

      cmd
        .args(args.clone())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())

        .spawn().unwrap().wait().unwrap();
    }, false);

    let stats = lib::reporter::BenchmarkStats {
      avg: stats.avg,
      min: stats.min,
      max: stats.max,
      p75: stats.p75,
      p99: stats.p99,
      p995: stats.p995,
    };

    println!("{}", lib::reporter::benchmark(&cmd, &stats, &options));

    results.push(lib::reporter::GroupBenchmark {
      name: cmd,
      stats: stats,
      baseline: false,
      group: "group".to_string(),
    });
  }

  println!("\n{}", lib::reporter::summary(&results, &options));
}