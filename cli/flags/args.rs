// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::util;
use crate::flags::DenoFlags;
use clap::Arg;
use clap::ArgMatches;
use log::Level;

// Creates vector of strings, Vec<String>
macro_rules! svec {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

pub fn lock<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("lock")
    .long("lock")
    .value_name("FILE")
    .help("Check the specified lock file")
    .takes_value(true)
}

pub fn lock_write<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("lock-write")
    .long("lock-write")
    .help("Write lock file. Use with --lock.")
}

pub fn parse_lock_args(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("lock") {
    let lockfile = matches.value_of("lock").unwrap();
    flags.lock = Some(lockfile.to_string());
  }
  if matches.is_present("lock-write") {
    flags.lock_write = true;
  }
}

pub fn no_fetch<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("no-fetch")
    .long("no-fetch")
    .help("Do not download remote modules")
}

pub fn parse_no_fetch(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("no-fetch") {
    flags.no_fetch = true;
  }
}

pub fn log_level<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("log-level")
    .short("L")
    .long("log-level")
    .help("Set log level")
    .takes_value(true)
    .possible_values(&["debug", "info"])
}

pub fn parse_log_level(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("log-level") {
    flags.log_level = match matches.value_of("log-level").unwrap() {
      "debug" => Some(Level::Debug),
      "info" => Some(Level::Info),
      _ => unreachable!(),
    };
  }
}

pub fn reload<'a, 'b>() -> Arg<'a, 'b> {
  Arg::with_name("reload")
    .short("r")
    .min_values(0)
    .takes_value(true)
    .use_delimiter(true)
    .require_equals(true)
    .long("reload")
    .help("Reload source code cache (recompile TypeScript)")
    .value_name("CACHE_BLACKLIST")
    .long_help("Reload source code cache (recompile TypeScript)
          --reload
            Reload everything
          --reload=https://deno.land/std
            Reload all standard modules
          --reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts
            Reloads specific modules")
}

pub fn parse_reload(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("reload") {
    if matches.value_of("reload").is_some() {
      let cache_bl = matches.values_of("reload").unwrap();
      let raw_cache_blacklist: Vec<String> =
        cache_bl.map(std::string::ToString::to_string).collect();
      flags.cache_blacklist = util::resolve_urls(raw_cache_blacklist);
      debug!("cache blacklist: {:#?}", &flags.cache_blacklist);
      flags.reload = false;
    } else {
      flags.reload = true;
    }
  }
}

pub fn permissions<'a, 'b>() -> Vec<Arg<'a, 'b>> {
  vec![
    Arg::with_name("allow-read")
      .long("allow-read")
      .min_values(0)
      .takes_value(true)
      .use_delimiter(true)
      .require_equals(true)
      .help("Allow file system read access"),
    Arg::with_name("allow-write")
      .long("allow-write")
      .min_values(0)
      .takes_value(true)
      .use_delimiter(true)
      .require_equals(true)
      .help("Allow file system write access"),
    Arg::with_name("allow-net")
      .long("allow-net")
      .min_values(0)
      .takes_value(true)
      .use_delimiter(true)
      .require_equals(true)
      .help("Allow network access"),
    Arg::with_name("allow-env")
      .long("allow-env")
      .help("Allow environment access"),
    Arg::with_name("allow-run")
      .long("allow-run")
      .help("Allow running subprocesses"),
    Arg::with_name("allow-hrtime")
      .long("allow-hrtime")
      .help("Allow high resolution time measurement"),
    Arg::with_name("allow-all")
      .short("A")
      .long("allow-all")
      .help("Allow all permissions"),
  ]
}

pub fn parse_permissions(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("allow-read") {
    if matches.value_of("allow-read").is_some() {
      let read_wl = matches.values_of("allow-read").unwrap();
      let raw_read_whitelist: Vec<String> =
        read_wl.map(std::string::ToString::to_string).collect();
      flags.read_whitelist = util::resolve_paths(raw_read_whitelist);
      debug!("read whitelist: {:#?}", &flags.read_whitelist);
    } else {
      flags.allow_read = true;
    }
  }
  if matches.is_present("allow-write") {
    if matches.value_of("allow-write").is_some() {
      let write_wl = matches.values_of("allow-write").unwrap();
      let raw_write_whitelist =
        write_wl.map(std::string::ToString::to_string).collect();
      flags.write_whitelist = util::resolve_paths(raw_write_whitelist);
      debug!("write whitelist: {:#?}", &flags.write_whitelist);
    } else {
      flags.allow_write = true;
    }
  }
  if matches.is_present("allow-net") {
    if matches.value_of("allow-net").is_some() {
      let net_wl = matches.values_of("allow-net").unwrap();
      let raw_net_whitelist =
        net_wl.map(std::string::ToString::to_string).collect();
      flags.net_whitelist = util::resolve_hosts(raw_net_whitelist);
      debug!("net whitelist: {:#?}", &flags.net_whitelist);
    } else {
      flags.allow_net = true;
    }
  }
  if matches.is_present("allow-env") {
    flags.allow_env = true;
  }
  if matches.is_present("allow-run") {
    flags.allow_run = true;
  }
  if matches.is_present("allow-hrtime") {
    flags.allow_hrtime = true;
  }
  if matches.is_present("allow-all") {
    flags.allow_read = true;
    flags.allow_env = true;
    flags.allow_net = true;
    flags.allow_run = true;
    flags.allow_read = true;
    flags.allow_write = true;
    flags.allow_hrtime = true;
  }
}

pub fn configuration<'a, 'b>() -> Vec<Arg<'a, 'b>> {
  vec![
    Arg::with_name("config")
      .short("c")
      .long("config")
      .value_name("FILE")
      .help("Load tsconfig.json configuration file")
      .takes_value(true),
    Arg::with_name("importmap")
      .long("importmap")
      .value_name("FILE")
      .help("Load import map file")
      .long_help(
        "Load import map file
Specification: https://wicg.github.io/import-maps/
Examples: https://github.com/WICG/import-maps#the-import-map",
      )
      .takes_value(true),
  ]
}

pub fn parse_configuration(flags: &mut DenoFlags, matches: &ArgMatches) {
  flags.config_path = matches.value_of("config").map(ToOwned::to_owned);
  flags.import_map_path = matches.value_of("importmap").map(ToOwned::to_owned);
}

pub fn runtime<'a, 'b>() -> Vec<Arg<'a, 'b>> {
  vec![
    Arg::with_name("current-thread")
      .long("current-thread")
      .help("Use tokio::runtime::current_thread"),
    Arg::with_name("seed")
      .long("seed")
      .value_name("NUMBER")
      .help("Seed Math.random()")
      .takes_value(true)
      .validator(|val: String| match val.parse::<u64>() {
        Ok(_) => Ok(()),
        Err(_) => Err("Seed should be a number".to_string()),
      }),
    Arg::with_name("v8-options")
      .long("v8-options")
      .help("Print V8 command line options"),
    Arg::with_name("v8-flags")
      .long("v8-flags")
      .takes_value(true)
      .use_delimiter(true)
      .require_equals(true)
      .help("Set V8 command line options"),
  ]
}

pub fn parse_runtime(flags: &mut DenoFlags, matches: &ArgMatches) {
  if matches.is_present("current-thread") {
    flags.current_thread = true;
  }
  if matches.is_present("v8-options") {
    let v8_flags = svec!["deno", "--help"];
    flags.v8_flags = Some(v8_flags);
  }
  if matches.is_present("v8-flags") {
    let mut v8_flags: Vec<String> = matches
      .values_of("v8-flags")
      .unwrap()
      .map(String::from)
      .collect();

    v8_flags.insert(0, "deno".to_string());
    flags.v8_flags = Some(v8_flags);
  }
  if matches.is_present("seed") {
    let seed_string = matches.value_of("seed").unwrap();
    let seed = seed_string.parse::<u64>().unwrap();
    flags.seed = Some(seed);

    let v8_seed_flag = format!("--random-seed={}", seed);

    match flags.v8_flags {
      Some(ref mut v8_flags) => {
        v8_flags.push(v8_seed_flag);
      }
      None => {
        flags.v8_flags = Some(svec!["deno", v8_seed_flag]);
      }
    }
  }
}
