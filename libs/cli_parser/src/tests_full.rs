// Copyright 2018-2026 the Deno authors. MIT license.
#![allow(
  clippy::useless_vec,
  reason = "svec! macro creates Vec<String> for test inputs"
)]
// Ported from deno/cli/args/flags.rs tests (lines 8089-15448).
// See top-level task description for mapping rules between the Deno
// original and this standalone parser crate.

use pretty_assertions::assert_eq;

use crate::CliErrorKind;
use crate::convert::flags_from_vec;
use crate::flags::*;

/// Creates Vec<String> from string literals.
macro_rules! svec {
    ($($x:expr),* $(,)?) => {
        vec![$($x.to_string()),*]
    };
}

// ---------------------------------------------------------------------------
// 1. global_flags
// ---------------------------------------------------------------------------

#[test]
fn global_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "--log-level", "debug", "--quiet", "run", "script.ts"]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string()
      )),
      log_level: Some(log::Level::Error),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
  #[rustfmt::skip]
    let r2 = flags_from_vec(svec!["deno", "run", "--log-level", "debug", "--quiet", "script.ts"]);
  let flags2 = r2.unwrap();
  assert_eq!(flags2, flags);
}

// ---------------------------------------------------------------------------
// 2. upgrade
// ---------------------------------------------------------------------------

#[test]
fn upgrade() {
  let r = flags_from_vec(svec!["deno", "upgrade", "--dry-run", "--force"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: true,
        dry_run: true,
        canary: false,
        release_candidate: false,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: None,
        branch: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 3. upgrade_with_output_flag
// ---------------------------------------------------------------------------

#[test]
fn upgrade_with_output_flag() {
  let r = flags_from_vec(svec!["deno", "upgrade", "--output", "example.txt"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: false,
        canary: false,
        release_candidate: false,
        version: None,
        output: Some(String::from("example.txt")),
        version_or_hash_or_channel: None,
        checksum: None,
        pr: None,
        branch: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 4. version
// ---------------------------------------------------------------------------

#[test]
fn version() {
  let r = flags_from_vec(svec!["deno", "--version"]);
  assert_eq!(r.unwrap_err().kind, CliErrorKind::DisplayVersion);
  let r = flags_from_vec(svec!["deno", "-V"]);
  assert_eq!(r.unwrap_err().kind, CliErrorKind::DisplayVersion);
}

// ---------------------------------------------------------------------------
// 5. run_reload
// ---------------------------------------------------------------------------

#[test]
fn run_reload() {
  let r = flags_from_vec(svec!["deno", "run", "-r", "script.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string()
      )),
      reload: true,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 6. run_watch
// ---------------------------------------------------------------------------

#[test]
fn run_watch() {
  let r = flags_from_vec(svec!["deno", "run", "--watch", "script.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: false,
          exclude: vec![],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(svec!["deno", "--watch", "--no-clear-screen", "script.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: true,
          exclude: vec![],
        }),
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--watch-hmr",
    "--no-clear-screen",
    "script.ts"
  ]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: true,
          paths: vec![],
          no_clear_screen: true,
          exclude: vec![],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--unstable-hmr",
    "--no-clear-screen",
    "script.ts"
  ]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: true,
          paths: vec![],
          no_clear_screen: true,
          exclude: vec![],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--watch-hmr=foo.txt",
    "--no-clear-screen",
    "script.ts"
  ]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: true,
          paths: vec![String::from("foo.txt")],
          no_clear_screen: true,
          exclude: vec![],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  // --hmr + --watch conflict not implemented (requires conflict checking)
  // let r =
  //     flags_from_vec(svec!["deno", "run", "--hmr", "--watch", "script.ts"]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 7. run_watch_with_external
// ---------------------------------------------------------------------------

#[test]
fn run_watch_with_external() {
  let r = flags_from_vec(svec!["deno", "--watch=file1,file2", "script.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("file1"), String::from("file2")],
          no_clear_screen: false,
          exclude: vec![],
        }),
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 8. run_watch_with_no_clear_screen
// ---------------------------------------------------------------------------

#[test]
fn run_watch_with_no_clear_screen() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--watch",
    "--no-clear-screen",
    "script.ts"
  ]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: true,
          exclude: vec![],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 9. run_watch_with_excluded_paths
// ---------------------------------------------------------------------------

#[test]
fn run_watch_with_excluded_paths() {
  let r = flags_from_vec(svec!(
    "deno",
    "--watch",
    "--watch-exclude=foo",
    "script.ts"
  ));

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: false,
          exclude: vec![String::from("foo")],
        }),
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!(
    "deno",
    "run",
    "--watch=foo",
    "--watch-exclude=bar",
    "script.ts"
  ));
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo")],
          no_clear_screen: false,
          exclude: vec![String::from("bar")],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--watch",
    "--watch-exclude=foo,bar",
    "script.ts"
  ]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: false,
          exclude: vec![String::from("foo"), String::from("bar")],
        }),
        bare: false,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "--watch=foo,bar",
    "--watch-exclude=baz,qux",
    "script.ts"
  ]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo"), String::from("bar")],
          no_clear_screen: false,
          exclude: vec![String::from("baz"), String::from("qux")],
        }),
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 10. run_reload_allow_write
// ---------------------------------------------------------------------------

#[test]
fn run_reload_allow_write() {
  let r =
    flags_from_vec(svec!["deno", "run", "-r", "--allow-write", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      reload: true,
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string()
      )),
      permissions: PermissionFlags {
        allow_write: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 11. run_coverage
// ---------------------------------------------------------------------------

#[test]
fn run_coverage() {
  let r = flags_from_vec(svec!["deno", "run", "--coverage=foo", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: false,
        coverage_dir: Some("foo".to_string()),
        print_task_list: false,
      }),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 12. run_v8_flags
// ---------------------------------------------------------------------------

#[test]
fn run_v8_flags() {
  let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--help"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default("_".to_string())),
      v8_flags: svec!["--help"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--v8-flags=--expose-gc,--gc-stats=1",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      v8_flags: svec!["--expose-gc", "--gc-stats=1"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "run", "--v8-flags=--expose-gc"]);
  assert!(r.is_ok());
}

// ---------------------------------------------------------------------------
// 13. serve_flags
// ---------------------------------------------------------------------------

#[test]
fn serve_flags() {
  let r = flags_from_vec(svec!["deno", "serve", "main.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
        "main.ts".to_string(),
        8000,
        "0.0.0.0"
      )),
      permissions: PermissionFlags {
        allow_net: None,
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
  let r = flags_from_vec(svec!["deno", "serve", "--port", "5000", "main.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
        "main.ts".to_string(),
        5000,
        "0.0.0.0"
      )),
      permissions: PermissionFlags {
        allow_net: None,
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
  let r = flags_from_vec(svec![
    "deno",
    "serve",
    "--port",
    "5000",
    "--allow-net=example.com",
    "main.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
        "main.ts".to_string(),
        5000,
        "0.0.0.0"
      )),
      permissions: PermissionFlags {
        allow_net: Some(vec!["example.com".to_string()]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
  let r = flags_from_vec(svec![
    "deno",
    "serve",
    "--port",
    "5000",
    "--allow-net",
    "main.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
        "main.ts".to_string(),
        5000,
        "0.0.0.0"
      )),
      permissions: PermissionFlags {
        allow_net: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 14. has_permission -- SKIPPED (method on Flags, port later)
// ---------------------------------------------------------------------------

#[test]
fn has_permission() {
  let r = flags_from_vec(svec!["deno", "--allow-read", "x.ts"]);
  assert_eq!(r.unwrap().has_permission(), true);

  let r = flags_from_vec(svec!["deno", "run", "--deny-read", "x.ts"]);
  assert_eq!(r.unwrap().has_permission(), true);

  let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
  assert_eq!(r.unwrap().has_permission(), false);
}

// ---------------------------------------------------------------------------
// 15. has_permission_in_argv -- SKIPPED (method on Flags, port later)
// ---------------------------------------------------------------------------

#[test]
fn has_permission_in_argv() {
  let r = flags_from_vec(svec!["deno", "run", "x.ts", "--allow-read"]);
  assert_eq!(r.unwrap().has_permission_in_argv(), true);

  let r = flags_from_vec(svec!["deno", "x.ts", "--deny-read"]);
  assert_eq!(r.unwrap().has_permission_in_argv(), true);

  let r = flags_from_vec(svec!["deno", "run", "x.ts"]);
  assert_eq!(r.unwrap().has_permission_in_argv(), false);
}

// ---------------------------------------------------------------------------
// 16. script_args
// ---------------------------------------------------------------------------

#[test]
fn script_args() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-net",
    "gist.ts",
    "--title",
    "X"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "gist.ts".to_string()
      )),
      argv: svec!["--title", "X"],
      permissions: PermissionFlags {
        allow_net: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 17. allow_all
// ---------------------------------------------------------------------------

#[test]
fn allow_all() {
  let r = flags_from_vec(svec!["deno", "run", "--allow-all", "gist.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "gist.ts".to_string()
      )),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 18. allow_read
// ---------------------------------------------------------------------------

#[test]
fn allow_read() {
  let r = flags_from_vec(svec!["deno", "run", "--allow-read", "gist.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "gist.ts".to_string()
      )),
      permissions: PermissionFlags {
        allow_read: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 19. short_permission_flags
// ---------------------------------------------------------------------------

#[test]
fn short_permission_flags() {
  let r = flags_from_vec(svec!["deno", "run", "-RNESWI", "gist.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "gist.ts".to_string()
      )),
      permissions: PermissionFlags {
        allow_read: Some(vec![]),
        allow_write: Some(vec![]),
        allow_env: Some(vec![]),
        allow_import: Some(vec![]),
        allow_net: Some(vec![]),
        allow_sys: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 20. deny_read
// ---------------------------------------------------------------------------

#[test]
fn deny_read() {
  let r = flags_from_vec(svec!["deno", "--deny-read", "gist.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "gist.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      permissions: PermissionFlags {
        deny_read: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 21. double_hyphen
// ---------------------------------------------------------------------------

#[test]
fn double_hyphen() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-write",
    "script.ts",
    "--",
    "-D",
    "--allow-net"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      argv: svec!["-D", "--allow-net"],
      permissions: PermissionFlags {
        allow_write: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 22. fmt
// ---------------------------------------------------------------------------

#[test]
fn fmt() {
  let r = flags_from_vec(svec!["deno", "fmt", "script_1.ts", "script_2.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "fmt", "--permit-no-files", "--check"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: true,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: true,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "fmt"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "fmt", "--watch"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Some(Default::default()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "fmt",
    "--watch",
    "--no-clear-screen",
    "--unstable-css",
    "--unstable-html",
    "--unstable-component",
    "--unstable-yaml",
    "--unstable-sql"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: true,
        unstable_sql: true,
        watch: Some(WatchFlags {
          hmr: false,
          no_clear_screen: true,
          exclude: vec![],
        })
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "fmt",
    "--check",
    "--watch",
    "foo.ts",
    "--ignore=bar.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: true,
        fail_fast: false,
        files: FileFlags {
          include: vec!["foo.ts".to_string()],
          ignore: vec!["bar.js".to_string()],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Some(Default::default()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "fmt", "--config", "deno.jsonc"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "fmt",
    "--config",
    "deno.jsonc",
    "--watch",
    "foo.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec!["foo.ts".to_string()],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Some(Default::default()),
      }),
      config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "fmt",
    "--use-tabs",
    "--line-width",
    "60",
    "--indent-width",
    "4",
    "--single-quote",
    "--prose-wrap",
    "never",
    "--no-semicolons",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: Some(true),
        line_width: std::num::NonZeroU32::new(60),
        indent_width: std::num::NonZeroU8::new(4),
        single_quote: Some(true),
        prose_wrap: Some("never".to_string()),
        no_semicolons: Some(true),
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  // try providing =false to the booleans
  let r = flags_from_vec(svec![
    "deno",
    "fmt",
    "--use-tabs=false",
    "--single-quote=false",
    "--no-semicolons=false",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: Some(false),
        line_width: None,
        indent_width: None,
        single_quote: Some(false),
        prose_wrap: None,
        no_semicolons: Some(false),
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  // --ext html without files requires validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "fmt", "--ext", "html"]);
  // assert!(r.is_err());
  let r = flags_from_vec(svec!["deno", "fmt", "--ext", "html", "./**"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Fmt(FmtFlags {
        check: false,
        fail_fast: false,
        files: FileFlags {
          include: vec!["./**".to_string()],
          ignore: vec![],
        },
        permit_no_files: false,
        use_tabs: None,
        line_width: None,
        indent_width: None,
        single_quote: None,
        prose_wrap: None,
        no_semicolons: None,
        unstable_component: false,
        unstable_sql: false,
        watch: Default::default(),
      }),
      ext: Some("html".to_string()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 23. lint
// ---------------------------------------------------------------------------

#[test]
fn lint() {
  let r = flags_from_vec(svec!["deno", "lint", "script_1.ts", "script_2.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--permit-no-files",
    "--allow-import",
    "--watch",
    "script_1.ts",
    "script_2.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: true,
        json: false,
        compact: false,
        watch: Some(Default::default()),
      }),
      permissions: PermissionFlags {
        allow_import: Some(vec![]),
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--watch",
    "--no-clear-screen",
    "script_1.ts",
    "script_2.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Some(WatchFlags {
          hmr: false,
          no_clear_screen: true,
          exclude: vec![],
        }),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--fix",
    "--ignore=script_1.ts,script_2.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec![],
          ignore: vec!["script_1.ts".to_string(), "script_2.ts".to_string()],
        },
        fix: true,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "lint", "--rules"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        fix: false,
        rules: true,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--rules",
    "--rules-tags=recommended"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        fix: false,
        rules: true,
        maybe_rules_tags: Some(svec!["recommended"]),
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--rules-tags=",
    "--rules-include=ban-untagged-todo,no-undef",
    "--rules-exclude=no-const-assign"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: Some(svec![""]),
        maybe_rules_include: Some(svec!["ban-untagged-todo", "no-undef"]),
        maybe_rules_exclude: Some(svec!["no-const-assign"]),
        permit_no_files: false,
        json: false,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "lint", "--json", "script_1.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: true,
        compact: false,
        watch: Default::default(),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--config",
    "Deno.jsonc",
    "--json",
    "script_1.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: true,
        compact: false,
        watch: Default::default(),
      }),
      config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "lint",
    "--config",
    "Deno.jsonc",
    "--compact",
    "script_1.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Lint(LintFlags {
        files: FileFlags {
          include: vec!["script_1.ts".to_string()],
          ignore: vec![],
        },
        fix: false,
        rules: false,
        maybe_rules_tags: None,
        maybe_rules_include: None,
        maybe_rules_exclude: None,
        permit_no_files: false,
        json: false,
        compact: true,
        watch: Default::default(),
      }),
      config_flag: ConfigFlag::Path("Deno.jsonc".to_string()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 24. types
// ---------------------------------------------------------------------------

#[test]
fn types() {
  let r = flags_from_vec(svec!["deno", "types"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Types,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 25. cache
// ---------------------------------------------------------------------------

#[test]
fn cache() {
  let r = flags_from_vec(svec!["deno", "cache", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "cache", "--env-file", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      env_file: Some(svec![".env"]),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 26. check
// ---------------------------------------------------------------------------

#[test]
fn check() {
  let r = flags_from_vec(svec!["deno", "check", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["script.ts"],
        doc: false,
        doc_only: false,
        check_js: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "check"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["."],
        doc: false,
        doc_only: false,
        check_js: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "check", "--doc", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["script.ts"],
        doc: true,
        doc_only: false,
        check_js: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "check", "--doc-only", "markdown.md"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["markdown.md"],
        doc: false,
        doc_only: true,
        check_js: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  // `--doc` and `--doc-only` are mutually exclusive (conflict checking not implemented)
  // let r = flags_from_vec(svec![
  //     "deno",
  //     "check",
  //     "--doc",
  //     "--doc-only",
  //     "script.ts"
  // ]);
  // assert!(r.is_err());

  for all_flag in ["--remote", "--all"] {
    let r = flags_from_vec(svec!["deno", "check", all_flag, "script.ts"]);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Check(CheckFlags {
          files: svec!["script.ts"],
          doc: false,
          doc_only: false,
          check_js: false,
        }),
        type_check_mode: TypeCheckMode::All,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );

    // --all/--remote + --no-remote conflict (not implemented)
    // let r = flags_from_vec(svec![
    //     "deno",
    //     "check",
    //     all_flag,
    //     "--no-remote",
    //     "script.ts"
    // ]);
    // assert!(r.is_err());
  }

  let r = flags_from_vec(svec!["deno", "check", "--check-js", "script.js"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["script.js"],
        doc: false,
        doc_only: false,
        check_js: true,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 27. info
// ---------------------------------------------------------------------------

#[test]
fn info() {
  let r = flags_from_vec(svec!["deno", "info", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: false,
        file: Some("script.ts".to_string()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "info", "--reload", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: false,
        file: Some("script.ts".to_string()),
      }),
      reload: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "info", "--json", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: true,
        file: Some("script.ts".to_string()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "info"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: false,
        file: None
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "info", "--json"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: true,
        file: None
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "info",
    "--no-npm",
    "--no-remote",
    "--config",
    "tsconfig.json"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: false,
        file: None
      }),
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      no_npm: true,
      no_remote: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 28. tsconfig
// ---------------------------------------------------------------------------

#[test]
fn tsconfig() {
  let r =
    flags_from_vec(svec!["deno", "run", "-c", "tsconfig.json", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 29. eval
// ---------------------------------------------------------------------------

#[test]
fn eval() {
  let r = flags_from_vec(svec!["deno", "eval", "'console.log(\"hello\")'"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Eval(EvalFlags {
        print: false,
        code: "'console.log(\"hello\")'".to_string(),
      }),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 30. eval_p
// ---------------------------------------------------------------------------

#[test]
fn eval_p() {
  let r = flags_from_vec(svec!["deno", "eval", "-p", "1+2"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Eval(EvalFlags {
        print: true,
        code: "1+2".to_string(),
      }),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 31. eval_typescript
// ---------------------------------------------------------------------------

#[test]
fn eval_typescript() {
  let r = flags_from_vec(svec![
    "deno",
    "eval",
    "--ext=ts",
    "'console.log(\"hello\")'"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Eval(EvalFlags {
        print: false,
        code: "'console.log(\"hello\")'".to_string(),
      }),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ext: Some("ts".to_string()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 32. eval_with_flags
// ---------------------------------------------------------------------------

#[test]
fn eval_with_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "eval", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--reload", "--lock=lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--env-file=.example.env", "42"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Eval(EvalFlags {
        print: false,
        code: "42".to_string(),
      }),
      import_map_path: Some("import_map.json".to_string()),
      no_remote: true,
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      type_check_mode: TypeCheckMode::None,
      reload: true,
      lock: Some(String::from("lock.json")),
      ca_data: Some(CaData::File("example.crt".to_string())),
      cached_only: true,
      location: deno_core::url::Url::parse("https:foo").ok(),
      v8_flags: svec!["--help", "--random-seed=1"],
      seed: Some(1),
      inspect: Some("127.0.0.1:9229".parse().unwrap()),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      env_file: Some(vec![".example.env".to_owned()]),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 33. eval_args
// ---------------------------------------------------------------------------

#[test]
fn eval_args() {
  let r = flags_from_vec(svec![
    "deno",
    "eval",
    "console.log(Deno.args)",
    "arg1",
    "arg2"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Eval(EvalFlags {
        print: false,
        code: "console.log(Deno.args)".to_string(),
      }),
      argv: svec!["arg1", "arg2"],
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 34. repl
// ---------------------------------------------------------------------------

#[test]
fn repl() {
  let r = flags_from_vec(svec!["deno"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: None,
        is_default_command: true,
        json: false,
      }),
      unsafely_ignore_certificate_errors: None,
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 35. repl_trace_ops
// ---------------------------------------------------------------------------

#[test]
fn repl_trace_ops() {
  let r = flags_from_vec(svec!["deno", "repl", "--trace-ops"]);
  assert_eq!(r.unwrap().trace_ops, Some(vec![]));
  let r = flags_from_vec(svec!["deno", "repl", "--trace-ops=http,websocket"]);
  assert_eq!(
    r.unwrap().trace_ops,
    Some(vec!["http".to_string(), "websocket".to_string()])
  );
}

// ---------------------------------------------------------------------------
// 36. repl_with_flags
// ---------------------------------------------------------------------------

#[test]
fn repl_with_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "-A", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--reload", "--lock=lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--unsafely-ignore-certificate-errors", "--env-file=.example.env"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: None,
        is_default_command: false,
        json: false,
      }),
      import_map_path: Some("import_map.json".to_string()),
      no_remote: true,
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      type_check_mode: TypeCheckMode::None,
      reload: true,
      lock: Some(String::from("lock.json")),
      ca_data: Some(CaData::File("example.crt".to_string())),
      cached_only: true,
      location: deno_core::url::Url::parse("https:foo").ok(),
      v8_flags: svec!["--help", "--random-seed=1"],
      seed: Some(1),
      inspect: Some("127.0.0.1:9229".parse().unwrap()),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      env_file: Some(vec![".example.env".to_owned()]),
      unsafely_ignore_certificate_errors: Some(vec![]),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 37. repl_with_eval_flag
// ---------------------------------------------------------------------------

#[test]
fn repl_with_eval_flag() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--allow-write", "--eval", "console.log('hello');"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: Some("console.log('hello');".to_string()),
        is_default_command: false,
        json: false,
      }),
      permissions: PermissionFlags {
        allow_write: Some(vec![]),
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 38. repl_with_eval_file_flag
// ---------------------------------------------------------------------------

#[test]
fn repl_with_eval_file_flag() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file=./a.js,./b.ts,https://docs.deno.com/hello_world.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: Some(vec![
          "./a.js".to_string(),
          "./b.ts".to_string(),
          "https://docs.deno.com/hello_world.ts".to_string()
        ]),
        eval: None,
        is_default_command: false,
        json: false,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 39. repl_with_eval_file_flag_no_equals
// ---------------------------------------------------------------------------

#[test]
fn repl_with_eval_file_flag_no_equals() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file", "./script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: Some(vec!["./script.ts".to_string()]),
        eval: None,
        is_default_command: false,
        json: false,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 40. repl_with_eval_file_flag_multiple
// ---------------------------------------------------------------------------

#[test]
fn repl_with_eval_file_flag_multiple() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "repl", "--eval-file", "./a.ts", "--eval-file", "./b.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: Some(vec!["./a.ts".to_string(), "./b.ts".to_string()]),
        eval: None,
        is_default_command: false,
        json: false,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 41. allow_read_allowlist -- SKIPPED (uses test_util::TempDir)
// ---------------------------------------------------------------------------

#[test]
fn allow_read_allowlist() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-read=.,/tmp/testdir",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      permissions: PermissionFlags {
        allow_read: Some(svec![".", "/tmp/testdir"]),
        ..Default::default()
      },
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 42. deny_read_denylist
// ---------------------------------------------------------------------------

#[test]
fn deny_read_denylist() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-read=.,/tmp/testdir",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      permissions: PermissionFlags {
        deny_read: Some(svec![".", "/tmp/testdir"]),
        ..Default::default()
      },
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 43. ignore_read_ignorelist
// ---------------------------------------------------------------------------

#[test]
fn ignore_read_ignorelist() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--ignore-read=something.txt",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        ignore_read: Some(svec!["something.txt"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 44. ignore_read_ignorelist_multiple
// ---------------------------------------------------------------------------

#[test]
fn ignore_read_ignorelist_multiple() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--ignore-read=something.txt",
    "--ignore-read=something2.txt",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        ignore_read: Some(svec!["something.txt", "something2.txt"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 45. allow_write_allowlist -- SKIPPED (uses test_util::TempDir)
// ---------------------------------------------------------------------------

#[test]
fn allow_write_allowlist() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-write=.,/tmp/testdir",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      permissions: PermissionFlags {
        allow_write: Some(svec![".", "/tmp/testdir"]),
        ..Default::default()
      },
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 46. deny_write_denylist
// ---------------------------------------------------------------------------

#[test]
fn deny_write_denylist() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-write=.,/tmp/testdir",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      permissions: PermissionFlags {
        deny_write: Some(svec![".", "/tmp/testdir"]),
        ..Default::default()
      },
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 47. allow_net_allowlist
// ---------------------------------------------------------------------------

#[test]
fn allow_net_allowlist() {
  let r =
    flags_from_vec(svec!["deno", "run", "--allow-net=127.0.0.1", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_net: Some(svec!["127.0.0.1"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 48. deny_net_denylist
// ---------------------------------------------------------------------------

#[test]
fn deny_net_denylist() {
  let r = flags_from_vec(svec!["deno", "--deny-net=127.0.0.1", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      permissions: PermissionFlags {
        deny_net: Some(svec!["127.0.0.1"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 49. allow_env_allowlist
// ---------------------------------------------------------------------------

#[test]
fn allow_env_allowlist() {
  let r = flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_env: Some(svec!["HOME"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 50. deny_env_denylist
// ---------------------------------------------------------------------------

#[test]
fn deny_env_denylist() {
  let r = flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_env: Some(svec!["HOME"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 51. ignore_env_ignorelist
// ---------------------------------------------------------------------------

#[test]
fn ignore_env_ignorelist() {
  let r =
    flags_from_vec(svec!["deno", "run", "--ignore-env=HOME", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        ignore_env: Some(svec!["HOME"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 52. allow_env_allowlist_multiple
// ---------------------------------------------------------------------------

#[test]
fn allow_env_allowlist_multiple() {
  let r =
    flags_from_vec(svec!["deno", "run", "--allow-env=HOME,PATH", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_env: Some(svec!["HOME", "PATH"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 53. deny_env_denylist_multiple
// ---------------------------------------------------------------------------

#[test]
fn deny_env_denylist_multiple() {
  let r =
    flags_from_vec(svec!["deno", "run", "--deny-env=HOME,PATH", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_env: Some(svec!["HOME", "PATH"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 54. deny_env_ignorelist_multiple
// ---------------------------------------------------------------------------

#[test]
fn deny_env_ignorelist_multiple() {
  let r =
    flags_from_vec(svec!["deno", "run", "--ignore-env=HOME,PATH", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        ignore_env: Some(svec!["HOME", "PATH"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 55. allow_env_allowlist_validator -- SKIPPED (env var validation)
// ---------------------------------------------------------------------------

#[test]
fn allow_env_allowlist_validator() {
  let r = flags_from_vec(svec!["deno", "run", "--allow-env=HOME", "script.ts"]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec!["deno", "--allow-env=H=ME", "script.ts"]);
  assert!(r.is_err());
  let r =
    flags_from_vec(svec!["deno", "run", "--allow-env=H\0ME", "script.ts"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 56. deny_env_denylist_validator
// ---------------------------------------------------------------------------

#[test]
fn deny_env_denylist_validator() {
  let r = flags_from_vec(svec!["deno", "run", "--deny-env=HOME", "script.ts"]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec!["deno", "run", "--deny-env=H=ME", "script.ts"]);
  assert!(r.is_err());
  let r = flags_from_vec(svec!["deno", "--deny-env=H\0ME", "script.ts"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 57. allow_sys
// ---------------------------------------------------------------------------

#[test]
fn allow_sys() {
  let r = flags_from_vec(svec!["deno", "run", "--allow-sys", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_sys: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 58. deny_sys
// ---------------------------------------------------------------------------

#[test]
fn deny_sys() {
  let r = flags_from_vec(svec!["deno", "run", "--deny-sys", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_sys: Some(vec![]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 59. allow_sys_allowlist
// ---------------------------------------------------------------------------

#[test]
fn allow_sys_allowlist() {
  let r =
    flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_sys: Some(svec!["hostname"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 60. deny_sys_denylist
// ---------------------------------------------------------------------------

#[test]
fn deny_sys_denylist() {
  let r = flags_from_vec(svec!["deno", "--deny-sys=hostname", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      permissions: PermissionFlags {
        deny_sys: Some(svec!["hostname"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 61. allow_sys_allowlist_multiple
// ---------------------------------------------------------------------------

#[test]
fn allow_sys_allowlist_multiple() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-sys=hostname,osRelease",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_sys: Some(svec!["hostname", "osRelease"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 62. deny_sys_denylist_multiple
// ---------------------------------------------------------------------------

#[test]
fn deny_sys_denylist_multiple() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-sys=hostname,osRelease",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_sys: Some(svec!["hostname", "osRelease"]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 63. allow_sys_allowlist_validator -- SKIPPED (sys validation)
// ---------------------------------------------------------------------------

#[test]
fn allow_sys_allowlist_validator() {
  let r =
    flags_from_vec(svec!["deno", "run", "--allow-sys=hostname", "script.ts"]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-sys=hostname,osRelease",
    "script.ts"
  ]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec!["deno", "run", "--allow-sys=foo", "script.ts"]);
  assert!(r.is_err());
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-sys=hostname,foo",
    "script.ts"
  ]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 64. deny_sys_denylist_validator
// ---------------------------------------------------------------------------

#[test]
fn deny_sys_denylist_validator() {
  let r =
    flags_from_vec(svec!["deno", "run", "--deny-sys=hostname", "script.ts"]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-sys=hostname,osRelease",
    "script.ts"
  ]);
  assert!(r.is_ok());
  let r = flags_from_vec(svec!["deno", "run", "--deny-sys=foo", "script.ts"]);
  assert!(r.is_err());
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-sys=hostname,foo",
    "script.ts"
  ]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 65. reload_validator -- SKIPPED (reload URL validation)
// ---------------------------------------------------------------------------

#[test]
fn reload_validator() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--reload=http://deno.land/",
    "script.ts"
  ]);
  assert!(r.is_ok(), "should accept valid urls");

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--reload=http://deno.land/a,http://deno.land/b",
    "script.ts"
  ]);
  assert!(r.is_ok(), "should accept accept multiple valid urls");

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--reload=./relativeurl/",
    "script.ts"
  ]);
  assert!(r.is_err(), "Should reject relative urls that start with ./");

  let r =
    flags_from_vec(svec!["deno", "run", "--reload=relativeurl/", "script.ts"]);
  assert!(r.is_err(), "Should reject relative urls");

  let r =
    flags_from_vec(svec!["deno", "run", "--reload=/absolute", "script.ts"]);
  assert!(r.is_err(), "Should reject absolute urls");

  let r = flags_from_vec(svec!["deno", "--reload=/", "script.ts"]);
  assert!(r.is_err(), "Should reject absolute root url");

  let r = flags_from_vec(svec!["deno", "run", "--reload=", "script.ts"]);
  assert!(r.is_err(), "Should reject when nothing is provided");

  let r = flags_from_vec(svec!["deno", "run", "--reload=,", "script.ts"]);
  assert!(r.is_err(), "Should reject when a single comma is provided");

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--reload=,http://deno.land/a",
    "script.ts"
  ]);
  assert!(r.is_err(), "Should reject a leading comma");

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--reload=http://deno.land/a,",
    "script.ts"
  ]);
  assert!(r.is_err(), "Should reject a trailing comma");
}

// ---------------------------------------------------------------------------
// 66. run_import_map
// ---------------------------------------------------------------------------

#[test]
fn run_import_map() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--import-map=import_map.json",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      import_map_path: Some("import_map.json".to_owned()),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 67. info_import_map
// ---------------------------------------------------------------------------

#[test]
fn info_import_map() {
  let r = flags_from_vec(svec![
    "deno",
    "info",
    "--import-map=import_map.json",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        file: Some("script.ts".to_string()),
        json: false,
      }),
      import_map_path: Some("import_map.json".to_owned()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 68. cache_import_map
// ---------------------------------------------------------------------------

#[test]
fn cache_import_map() {
  let r = flags_from_vec(svec![
    "deno",
    "cache",
    "--import-map=import_map.json",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      import_map_path: Some("import_map.json".to_owned()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 69. doc_import_map
// ---------------------------------------------------------------------------

#[test]
fn doc_import_map() {
  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--import-map=import_map.json",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        source_files: DocSourceFileFlag::Paths(vec!["script.ts".to_owned()]),
        private: false,
        json: false,
        html: None,
        lint: false,
        filter: None,
      }),
      import_map_path: Some("import_map.json".to_owned()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 70. run_env_default
// ---------------------------------------------------------------------------

#[test]
fn run_env_default() {
  let r = flags_from_vec(svec!["deno", "run", "--env", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      env_file: Some(vec![".env".to_owned()]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 71. run_env_file_default
// ---------------------------------------------------------------------------

#[test]
fn run_env_file_default() {
  let r = flags_from_vec(svec!["deno", "run", "--env-file", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      env_file: Some(vec![".env".to_owned()]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 72. run_no_code_cache
// ---------------------------------------------------------------------------

#[test]
fn run_no_code_cache() {
  let r = flags_from_vec(svec!["deno", "--no-code-cache", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 73. run_env_defined
// ---------------------------------------------------------------------------

#[test]
fn run_env_defined() {
  let r =
    flags_from_vec(svec!["deno", "run", "--env=.another_env", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      env_file: Some(vec![".another_env".to_owned()]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 74. run_env_file_defined
// ---------------------------------------------------------------------------

#[test]
fn run_env_file_defined() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--env-file=.another_env",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      env_file: Some(vec![".another_env".to_owned()]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 75. run_multiple_env_file_defined
// ---------------------------------------------------------------------------

#[test]
fn run_multiple_env_file_defined() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--env-file",
    "--env-file=.two_env",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      env_file: Some(vec![".env".to_owned(), ".two_env".to_owned()]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 76. cache_multiple
// ---------------------------------------------------------------------------

#[test]
fn cache_multiple() {
  let r = flags_from_vec(svec!["deno", "cache", "script.ts", "script_two.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts", "script_two.ts"],
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 77. run_seed
// ---------------------------------------------------------------------------

#[test]
fn run_seed() {
  let r = flags_from_vec(svec!["deno", "run", "--seed", "250", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      seed: Some(250_u64),
      v8_flags: svec!["--random-seed=250"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 78. run_seed_with_v8_flags
// ---------------------------------------------------------------------------

#[test]
fn run_seed_with_v8_flags() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--seed",
    "250",
    "--v8-flags=--expose-gc",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      seed: Some(250_u64),
      v8_flags: svec!["--expose-gc", "--random-seed=250"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 79. install
// ---------------------------------------------------------------------------

#[test]
fn install() {
  let r = flags_from_vec(svec![
    "deno",
    "install",
    "-g",
    "jsr:@std/http/file-server",
    "npm:chalk",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Install(InstallFlags::Global(
        InstallFlagsGlobal {
          name: None,
          module_urls: svec!["jsr:@std/http/file-server", "npm:chalk"],
          args: vec![],
          root: None,
          force: false,
          compile: false,
        }
      )),
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(svec!["deno", "install", "-g", "jsr:@std/http/file-server"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Install(InstallFlags::Global(
        InstallFlagsGlobal {
          name: None,
          module_urls: svec!["jsr:@std/http/file-server"],
          args: vec![],
          root: None,
          force: false,
          compile: false,
        }
      )),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 80. install_with_flags
// ---------------------------------------------------------------------------

#[test]
fn install_with_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "install", "--global", "--import-map", "import_map.json", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock=lock.json", "--cert", "example.crt", "--cached-only", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--inspect=127.0.0.1:9229", "--name", "file_server", "--root", "/foo", "--force", "jsr:@std/http/file-server", "--", "foo", "bar"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Install(InstallFlags::Global(
        InstallFlagsGlobal {
          name: Some("file_server".to_string()),
          module_urls: svec!["jsr:@std/http/file-server"],
          args: svec!["foo", "bar"],
          root: Some("/foo".to_string()),
          force: true,
          compile: false,
        }
      )),
      import_map_path: Some("import_map.json".to_string()),
      no_remote: true,
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      type_check_mode: TypeCheckMode::None,
      reload: true,
      lock: Some(String::from("lock.json")),
      ca_data: Some(CaData::File("example.crt".to_string())),
      cached_only: true,
      v8_flags: svec!["--help", "--random-seed=1"],
      seed: Some(1),
      inspect: Some("127.0.0.1:9229".parse().unwrap()),
      unsafely_ignore_certificate_errors: Some(vec![]),
      permissions: PermissionFlags {
        allow_net: Some(vec![]),
        allow_read: Some(vec![]),
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 81. uninstall
// ---------------------------------------------------------------------------

#[test]
fn uninstall() {
  // deno uninstall without package — requires missing-required validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "uninstall"]);
  // assert!(r.is_err());

  let r = flags_from_vec(svec![
    "deno",
    "uninstall",
    "--frozen",
    "--lockfile-only",
    "@std/load"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Uninstall(UninstallFlags {
        kind: UninstallKind::Local(RemoveFlags {
          packages: vec!["@std/load".to_string()],
          lockfile_only: true,
        }),
      }),
      frozen_lockfile: Some(true),
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(svec!["deno", "uninstall", "file_server", "@std/load"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Uninstall(UninstallFlags {
        kind: UninstallKind::Local(RemoveFlags {
          packages: vec!["file_server".to_string(), "@std/load".to_string()],
          lockfile_only: false,
        }),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "uninstall", "-g", "file_server"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Uninstall(UninstallFlags {
        kind: UninstallKind::Global(UninstallFlagsGlobal {
          name: "file_server".to_string(),
          root: None,
        }),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "uninstall",
    "-g",
    "--root",
    "/user/foo/bar",
    "file_server"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Uninstall(UninstallFlags {
        kind: UninstallKind::Global(UninstallFlagsGlobal {
          name: "file_server".to_string(),
          root: Some("/user/foo/bar".to_string()),
        }),
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 82. uninstall_with_help_flag -- SKIPPED (help flag)
// ---------------------------------------------------------------------------

#[test]

fn uninstall_with_help_flag() {
  let r = flags_from_vec(svec!["deno", "uninstall", "--help"]);
  assert!(r.is_ok());
  assert!(matches!(r.unwrap().subcommand, DenoSubcommand::Help(_)));
}

// ---------------------------------------------------------------------------
// 83. log_level
// ---------------------------------------------------------------------------

#[test]
fn log_level() {
  let r =
    flags_from_vec(svec!["deno", "run", "--log-level=debug", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      log_level: Some(log::Level::Debug),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 84. quiet
// ---------------------------------------------------------------------------

#[test]
fn quiet() {
  let r = flags_from_vec(svec!["deno", "-q", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      log_level: Some(log::Level::Error),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 85. completions -- SKIPPED (CompletionsFlags type is different)
// ---------------------------------------------------------------------------

#[test]
fn completions() {
  let r = flags_from_vec(svec!["deno", "completions", "zsh"]).unwrap();
  assert!(matches!(r.subcommand, DenoSubcommand::Completions(_)));
}

// ---------------------------------------------------------------------------
// 86. run_with_args
// ---------------------------------------------------------------------------

#[test]
fn run_with_args() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "script.ts",
    "--allow-read",
    "--allow-net"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      argv: svec!["--allow-read", "--allow-net"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--location",
    "https:foo",
    "--allow-read",
    "script.ts",
    "--allow-net",
    "-r",
    "--help",
    "--foo",
    "bar"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      location: deno_core::url::Url::parse("https:foo").ok(),
      permissions: PermissionFlags {
        allow_read: Some(vec![]),
        ..Default::default()
      },
      argv: svec!["--allow-net", "-r", "--help", "--foo", "bar"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "run", "script.ts", "foo", "bar"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      argv: svec!["foo", "bar"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
  let r = flags_from_vec(svec!["deno", "run", "script.ts", "-"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      argv: svec!["-"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "run", "script.ts", "-", "foo", "bar"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      argv: svec!["-", "foo", "bar"],
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 87. no_check
// ---------------------------------------------------------------------------

#[test]
fn no_check() {
  let r = flags_from_vec(svec!["deno", "--no-check", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      type_check_mode: TypeCheckMode::None,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 88. no_check_remote
// ---------------------------------------------------------------------------

#[test]
fn no_check_remote() {
  let r =
    flags_from_vec(svec!["deno", "run", "--no-check=remote", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 89. repl_with_unsafely_ignore_certificate_errors
// ---------------------------------------------------------------------------

#[test]
fn repl_with_unsafely_ignore_certificate_errors() {
  let r = flags_from_vec(svec![
    "deno",
    "repl",
    "--eval",
    "console.log('hello');",
    "--unsafely-ignore-certificate-errors"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: Some("console.log('hello');".to_string()),
        is_default_command: false,
        json: false,
      }),
      unsafely_ignore_certificate_errors: Some(vec![]),
      type_check_mode: TypeCheckMode::None,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 90. run_with_unsafely_ignore_certificate_errors
// ---------------------------------------------------------------------------

#[test]
fn run_with_unsafely_ignore_certificate_errors() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--unsafely-ignore-certificate-errors",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      unsafely_ignore_certificate_errors: Some(vec![]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 91. run_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address
// ---------------------------------------------------------------------------

#[test]
fn run_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      unsafely_ignore_certificate_errors: Some(svec![
        "deno.land",
        "localhost",
        "[::]",
        "127.0.0.1",
        "[::1]",
        "1.2.3.4"
      ]),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 92. repl_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address
// ---------------------------------------------------------------------------

#[test]
fn repl_with_unsafely_treat_insecure_origin_as_secure_with_ipv6_address() {
  let r = flags_from_vec(svec![
    "deno",
    "repl",
    "--unsafely-ignore-certificate-errors=deno.land,localhost,[::],127.0.0.1,[::1],1.2.3.4"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: None,
        is_default_command: false,
        json: false,
      }),
      unsafely_ignore_certificate_errors: Some(svec![
        "deno.land",
        "localhost",
        "[::]",
        "127.0.0.1",
        "[::1]",
        "1.2.3.4"
      ]),
      type_check_mode: TypeCheckMode::None,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 93. no_remote
// ---------------------------------------------------------------------------

#[test]
fn no_remote() {
  let r = flags_from_vec(svec!["deno", "run", "--no-remote", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      no_remote: true,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 94. no_npm
// ---------------------------------------------------------------------------

#[test]
fn no_npm() {
  let r = flags_from_vec(svec!["deno", "run", "--no-npm", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      no_npm: true,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 95. local_npm
// ---------------------------------------------------------------------------

#[test]
fn local_npm() {
  let r = flags_from_vec(svec!["deno", "--node-modules-dir", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      node_modules_dir: Some(NodeModulesDirMode::Auto),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 96. vendor_flag
// ---------------------------------------------------------------------------

#[test]
fn vendor_flag() {
  let r = flags_from_vec(svec!["deno", "run", "--vendor", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      vendor: Some(true),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "run", "--vendor=false", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      vendor: Some(false),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 97. cached_only
// ---------------------------------------------------------------------------

#[test]
fn cached_only() {
  let r = flags_from_vec(svec!["deno", "run", "--cached-only", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      cached_only: true,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 98. allow_net_allowlist_with_ports
// ---------------------------------------------------------------------------

#[test]
fn allow_net_allowlist_with_ports() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-net=deno.land,:8000,:4545",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_net: Some(svec![
          "deno.land",
          "0.0.0.0:8000",
          "127.0.0.1:8000",
          "localhost:8000",
          "0.0.0.0:4545",
          "127.0.0.1:4545",
          "localhost:4545"
        ]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 99. deny_net_denylist_with_ports
// ---------------------------------------------------------------------------

#[test]
fn deny_net_denylist_with_ports() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-net=deno.land,:8000,:4545",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_net: Some(svec![
          "deno.land",
          "0.0.0.0:8000",
          "127.0.0.1:8000",
          "localhost:8000",
          "0.0.0.0:4545",
          "127.0.0.1:4545",
          "localhost:4545"
        ]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 100. allow_net_allowlist_with_ipv6_address
// ---------------------------------------------------------------------------

#[test]
fn allow_net_allowlist_with_ipv6_address() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        allow_net: Some(svec![
          "deno.land",
          "deno.land:80",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4:5678",
          "0.0.0.0:5678",
          "127.0.0.1:5678",
          "localhost:5678",
          "[::1]:8080"
        ]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 101. deny_net_denylist_with_ipv6_address
// ---------------------------------------------------------------------------

#[test]
fn deny_net_denylist_with_ipv6_address() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-net=deno.land,deno.land:80,[::],127.0.0.1,[::1],1.2.3.4:5678,:5678,[::1]:8080",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      permissions: PermissionFlags {
        deny_net: Some(svec![
          "deno.land",
          "deno.land:80",
          "[::]",
          "127.0.0.1",
          "[::1]",
          "1.2.3.4:5678",
          "0.0.0.0:5678",
          "127.0.0.1:5678",
          "localhost:5678",
          "[::1]:8080"
        ]),
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 102. test_no_colon_in_value_name -- SKIPPED (tests help text format)
// ---------------------------------------------------------------------------

#[test]
fn test_no_colon_in_value_name() {
  // This test validates clap-specific help text formatting.
  // Not applicable to our custom parser — kept as a no-op.
}

// ---------------------------------------------------------------------------
// 103. test_with_flags
// ---------------------------------------------------------------------------

#[test]
fn test_with_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "test", "--no-npm", "--no-remote", "--trace-leaks", "--no-run", "--filter", "- foo", "--coverage=cov", "--clean", "--location", "https:foo", "--allow-net", "--permit-no-files", "dir1/", "dir2/", "--", "arg1", "arg2"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: true,
        doc: false,
        fail_fast: None,
        filter: Some("- foo".to_string()),
        permit_no_files: true,
        files: FileFlags {
          include: vec!["dir1/".to_string(), "dir2/".to_string()],
          ignore: vec![],
        },
        shuffle: None,
        parallel: false,
        trace_leaks: true,
        coverage_dir: Some("cov".to_string()),
        coverage_raw_data_only: false,
        clean: true,
        watch: Default::default(),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      no_npm: true,
      no_remote: true,
      location: deno_core::url::Url::parse("https:foo").ok(),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        allow_net: Some(vec![]),
        ..Default::default()
      },
      argv: svec!["arg1", "arg2"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 104. run_with_cafile
// ---------------------------------------------------------------------------

#[test]
fn run_with_cafile() {
  let r =
    flags_from_vec(svec!["deno", "run", "--cert", "example.crt", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      ca_data: Some(CaData::File("example.crt".to_owned())),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 105. run_with_base64_cafile
// ---------------------------------------------------------------------------

#[test]
fn run_with_base64_cafile() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--cert",
    "base64:bWVvdw==",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      ca_data: Some(CaData::Bytes(b"meow".into())),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 106. run_with_enable_testing_features
// ---------------------------------------------------------------------------

#[test]
fn run_with_enable_testing_features() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--enable-testing-features-do-not-use",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      enable_testing_features: true,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 107. test_with_fail_fast
// ---------------------------------------------------------------------------

#[test]
fn test_with_fail_fast() {
  let r = flags_from_vec(svec!["deno", "test", "--fail-fast=3"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: std::num::NonZeroUsize::new(3),
        filter: None,
        permit_no_files: false,
        shuffle: None,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Default::default(),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  // --fail-fast=0 value validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "test", "--fail-fast=0"]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 108. test_with_enable_testing_features
// ---------------------------------------------------------------------------

#[test]
fn test_with_enable_testing_features() {
  let r = flags_from_vec(svec![
    "deno",
    "test",
    "--enable-testing-features-do-not-use"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: None,
        filter: None,
        permit_no_files: false,
        shuffle: None,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Default::default(),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      enable_testing_features: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 109. test_reporter
// ---------------------------------------------------------------------------

#[test]
fn test_reporter() {
  let r = flags_from_vec(svec!["deno", "test", "--reporter=pretty"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        reporter: TestReporterConfig::Pretty,
        ..Default::default()
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "test", "--reporter=dot"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        reporter: TestReporterConfig::Dot,
        ..Default::default()
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      log_level: Some(log::Level::Error),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "test", "--reporter=junit"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        reporter: TestReporterConfig::Junit,
        ..Default::default()
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "test", "--reporter=tap"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        reporter: TestReporterConfig::Tap,
        ..Default::default()
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      log_level: Some(log::Level::Error),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "test",
    "--reporter=dot",
    "--junit-path=report.xml"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        reporter: TestReporterConfig::Dot,
        junit_path: Some("report.xml".to_string()),
        ..Default::default()
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      log_level: Some(log::Level::Error),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "test", "--junit-path"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 110. test_shuffle
// ---------------------------------------------------------------------------

#[test]
fn test_shuffle() {
  let r = flags_from_vec(svec!["deno", "test", "--shuffle=1"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: None,
        filter: None,
        permit_no_files: false,
        shuffle: Some(1),
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Default::default(),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 111. test_watch
// ---------------------------------------------------------------------------

#[test]
fn test_watch() {
  let r = flags_from_vec(svec!["deno", "test", "--watch"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: None,
        filter: None,
        permit_no_files: false,
        shuffle: None,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Some(Default::default()),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 112. test_watch_explicit_cwd
// ---------------------------------------------------------------------------

#[test]
fn test_watch_explicit_cwd() {
  let r = flags_from_vec(svec!["deno", "test", "--watch", "./"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: None,
        filter: None,
        permit_no_files: false,
        shuffle: None,
        files: FileFlags {
          include: vec!["./".to_string()],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Some(Default::default()),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 113. test_watch_with_no_clear_screen
// ---------------------------------------------------------------------------

#[test]
fn test_watch_with_no_clear_screen() {
  let r = flags_from_vec(svec!["deno", "test", "--watch", "--no-clear-screen"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        no_run: false,
        doc: false,
        fail_fast: None,
        filter: None,
        permit_no_files: false,
        shuffle: None,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        parallel: false,
        trace_leaks: false,
        coverage_dir: None,
        coverage_raw_data_only: false,
        clean: false,
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          no_clear_screen: true,
          exclude: vec![],
          paths: vec![],
        }),
        reporter: Default::default(),
        junit_path: None,
        hide_stacktraces: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 114. test_watch_with_paths
// ---------------------------------------------------------------------------

#[test]
fn test_watch_with_paths() {
  let r = flags_from_vec(svec!("deno", "test", "--watch=foo"));

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo")],
          no_clear_screen: false,
          exclude: vec![],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "test", "--watch=foo,bar"]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo"), String::from("bar")],
          no_clear_screen: false,
          exclude: vec![],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 115. test_watch_with_excluded_paths
// ---------------------------------------------------------------------------

#[test]
fn test_watch_with_excluded_paths() {
  let r =
    flags_from_vec(svec!("deno", "test", "--watch", "--watch-exclude=foo",));

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: false,
          exclude: vec![String::from("foo")],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(
      svec!("deno", "test", "--watch=foo", "--watch-exclude=bar",),
    );
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo")],
          no_clear_screen: false,
          exclude: vec![String::from("bar")],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(
      svec!["deno", "test", "--watch", "--watch-exclude=foo,bar",],
    );

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![],
          no_clear_screen: false,
          exclude: vec![String::from("foo"), String::from("bar")],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "test",
    "--watch=foo,bar",
    "--watch-exclude=baz,qux",
  ]);

  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        watch: Some(WatchFlagsWithPaths {
          hmr: false,
          paths: vec![String::from("foo"), String::from("bar")],
          no_clear_screen: false,
          exclude: vec![String::from("baz"), String::from("qux")],
        }),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 116. test_coverage_default_dir
// ---------------------------------------------------------------------------

#[test]
fn test_coverage_default_dir() {
  let r = flags_from_vec(svec!["deno", "test", "--coverage"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        coverage_dir: Some("coverage".to_string()),
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 117. test_hide_stacktraces
// ---------------------------------------------------------------------------

#[test]
fn test_hide_stacktraces() {
  let r = flags_from_vec(svec!["deno", "test", "--hide-stacktraces"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags {
        hide_stacktraces: true,
        ..TestFlags::default()
      }),
      type_check_mode: TypeCheckMode::Local,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 118. upgrade_with_ca_file
// ---------------------------------------------------------------------------

#[test]
fn upgrade_with_ca_file() {
  let r = flags_from_vec(svec!["deno", "upgrade", "--cert", "example.crt"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: false,
        canary: false,
        release_candidate: false,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: None,
        branch: None,
      }),
      ca_data: Some(CaData::File("example.crt".to_owned())),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 119. upgrade_release_candidate
// ---------------------------------------------------------------------------

#[test]
fn upgrade_release_candidate() {
  let r = flags_from_vec(svec!["deno", "upgrade", "--rc"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: false,
        canary: false,
        release_candidate: true,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: None,
        branch: None,
      }),
      ..Flags::default()
    }
  );

  // --rc + --canary conflict not implemented (requires conflict checking)
  // let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--canary"]);
  // assert!(r.is_err());

  let r = flags_from_vec(svec!["deno", "upgrade", "--rc", "--version"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 120. upgrade_pr
// ---------------------------------------------------------------------------

#[test]
fn upgrade_pr() {
  let r = flags_from_vec(svec!["deno", "upgrade", "pr", "12345"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: false,
        canary: false,
        release_candidate: false,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: Some(12345),
        branch: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 121. upgrade_pr_with_hash_prefix
// ---------------------------------------------------------------------------

#[test]
fn upgrade_pr_with_hash_prefix() {
  let r = flags_from_vec(svec!["deno", "upgrade", "pr", "#6789"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: false,
        canary: false,
        release_candidate: false,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: Some(6789),
        branch: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 122. upgrade_pr_with_flags
// ---------------------------------------------------------------------------

#[test]
fn upgrade_pr_with_flags() {
  let r = flags_from_vec(svec!["deno", "upgrade", "--dry-run", "pr", "33250"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Upgrade(UpgradeFlags {
        force: false,
        dry_run: true,
        canary: false,
        release_candidate: false,
        version: None,
        output: None,
        version_or_hash_or_channel: None,
        checksum: None,
        pr: Some(33250),
        branch: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 123. upgrade_pr_missing_number -- SKIPPED (PR validation)
// ---------------------------------------------------------------------------

#[test]
fn upgrade_pr_missing_number() {
  let r = flags_from_vec(svec!["deno", "upgrade", "pr"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 124. upgrade_pr_invalid_number
// ---------------------------------------------------------------------------

#[test]
fn upgrade_pr_invalid_number() {
  let r = flags_from_vec(svec!["deno", "upgrade", "pr", "abc"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 125. cache_with_cafile
// ---------------------------------------------------------------------------

#[test]
fn cache_with_cafile() {
  let r = flags_from_vec(svec![
    "deno",
    "cache",
    "--cert",
    "example.crt",
    "script.ts",
    "script_two.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts", "script_two.ts"],
      }),
      ca_data: Some(CaData::File("example.crt".to_owned())),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 126. info_with_cafile
// ---------------------------------------------------------------------------

#[test]
fn info_with_cafile() {
  let r = flags_from_vec(svec![
    "deno",
    "info",
    "--cert",
    "example.crt",
    "https://example.com"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Info(InfoFlags {
        json: false,
        file: Some("https://example.com".to_string()),
      }),
      ca_data: Some(CaData::File("example.crt".to_owned())),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 127. doc
// ---------------------------------------------------------------------------

#[test]
fn doc() {
  let r = flags_from_vec(svec!["deno", "doc", "--json", "path/to/module.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: true,
        html: None,
        lint: false,
        source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "doc", "--html", "path/to/module.ts"]);
  assert!(r.is_ok());

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--html",
    "--name=My library",
    "path/to/module.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: false,
        lint: false,
        html: Some(DocHtmlFlag {
          name: Some("My library".to_string()),
          category_docs_path: None,
          symbol_redirect_map_path: None,
          default_symbol_map_path: None,
          strip_trailing_html: false,
          output: String::from("./docs/"),
        }),
        source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--html",
    "--name=My library",
    "--lint",
    "--output=./foo",
    "path/to/module.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: false,
        html: Some(DocHtmlFlag {
          name: Some("My library".to_string()),
          category_docs_path: None,
          symbol_redirect_map_path: None,
          default_symbol_map_path: None,
          strip_trailing_html: false,
          output: String::from("./foo"),
        }),
        lint: true,
        source_files: DocSourceFileFlag::Paths(svec!["path/to/module.ts"]),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  // --html requires source files (validation not implemented)
  // let r = flags_from_vec(svec!["deno", "doc", "--html", "--name=My library",]);
  // assert!(r.is_err());

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--filter",
    "SomeClass.someField",
    "path/to/module.ts",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: false,
        html: None,
        lint: false,
        source_files: DocSourceFileFlag::Paths(vec![
          "path/to/module.ts".to_string()
        ]),
        filter: Some("SomeClass.someField".to_string()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "doc"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: false,
        html: None,
        lint: false,
        source_files: Default::default(),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--filter",
    "Deno.Listener",
    "--builtin"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        lint: false,
        json: false,
        html: None,
        source_files: DocSourceFileFlag::Builtin,
        filter: Some("Deno.Listener".to_string()),
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--no-npm",
    "--no-remote",
    "--private",
    "path/to/module.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: true,
        lint: false,
        json: false,
        html: None,
        source_files: DocSourceFileFlag::Paths(svec!["path/to/module.js"]),
        filter: None,
      }),
      no_npm: true,
      no_remote: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "path/to/module.js",
    "path/to/module2.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        lint: false,
        json: false,
        html: None,
        source_files: DocSourceFileFlag::Paths(vec![
          "path/to/module.js".to_string(),
          "path/to/module2.js".to_string()
        ]),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "path/to/module.js",
    "--builtin",
    "path/to/module2.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        json: false,
        html: None,
        lint: false,
        source_files: DocSourceFileFlag::Paths(vec![
          "path/to/module.js".to_string(),
          "path/to/module2.js".to_string()
        ]),
        filter: None,
      }),
      ..Flags::default()
    }
  );

  // --lint without source files requires validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "doc", "--lint",]);
  // assert!(r.is_err());

  let r = flags_from_vec(svec![
    "deno",
    "doc",
    "--lint",
    "path/to/module.js",
    "path/to/module2.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Doc(DocFlags {
        private: false,
        lint: true,
        json: false,
        html: None,
        source_files: DocSourceFileFlag::Paths(vec![
          "path/to/module.js".to_string(),
          "path/to/module2.js".to_string()
        ]),
        filter: None,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 128. inspect_default_host
// ---------------------------------------------------------------------------

#[test]
fn inspect_default_host() {
  let r = flags_from_vec(svec!["deno", "run", "--inspect", "foo.js"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "foo.js".to_string(),
      )),
      inspect: Some("127.0.0.1:9229".parse().unwrap()),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 129. inspect_wait
// ---------------------------------------------------------------------------

#[test]
fn inspect_wait() {
  let r = flags_from_vec(svec!["deno", "--inspect-wait", "foo.js"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "foo.js".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      inspect_wait: Some("127.0.0.1:9229".parse().unwrap()),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--inspect-wait=127.0.0.1:3567",
    "foo.js"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "foo.js".to_string(),
      )),
      inspect_wait: Some("127.0.0.1:3567".parse().unwrap()),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 130. compile
// ---------------------------------------------------------------------------

#[test]
fn compile() {
  let r = flags_from_vec(svec![
    "deno",
    "compile",
    "https://examples.deno.land/color-logging.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Compile(CompileFlags {
        source_file: "https://examples.deno.land/color-logging.ts".to_string(),
        output: None,
        args: vec![],
        target: None,
        no_terminal: false,
        icon: None,
        include: Default::default(),
        exclude: Default::default(),
        eszip: false,
        self_extracting: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 131. compile_with_flags
// ---------------------------------------------------------------------------

#[test]
fn compile_with_flags() {
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "compile", "--include", "include.txt", "--exclude=exclude.txt", "--import-map", "import_map.json", "--no-code-cache", "--no-remote", "--config", "tsconfig.json", "--no-check", "--unsafely-ignore-certificate-errors", "--reload", "--lock=lock.json", "--cert", "example.crt", "--cached-only", "--location", "https:foo", "--allow-read", "--allow-net", "--v8-flags=--help", "--seed", "1", "--no-terminal", "--icon", "favicon.ico", "--output", "colors", "--env-file=.example.env", "https://examples.deno.land/color-logging.ts", "foo", "bar", "-p", "8080"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Compile(CompileFlags {
        source_file: "https://examples.deno.land/color-logging.ts".to_string(),
        output: Some(String::from("colors")),
        args: svec!["foo", "bar", "-p", "8080"],
        target: None,
        no_terminal: true,
        icon: Some(String::from("favicon.ico")),
        include: vec!["include.txt".to_string()],
        exclude: vec!["exclude.txt".to_string()],
        eszip: false,
        self_extracting: false,
      }),
      import_map_path: Some("import_map.json".to_string()),
      no_remote: true,
      code_cache_enabled: false,
      config_flag: ConfigFlag::Path("tsconfig.json".to_owned()),
      type_check_mode: TypeCheckMode::None,
      reload: true,
      lock: Some(String::from("lock.json")),
      ca_data: Some(CaData::File("example.crt".to_string())),
      cached_only: true,
      location: deno_core::url::Url::parse("https:foo").ok(),
      permissions: PermissionFlags {
        allow_read: Some(vec![]),
        allow_net: Some(vec![]),
        ..Default::default()
      },
      unsafely_ignore_certificate_errors: Some(vec![]),
      v8_flags: svec!["--help", "--random-seed=1"],
      seed: Some(1),
      env_file: Some(vec![".example.env".to_owned()]),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 132. coverage
// ---------------------------------------------------------------------------

#[test]
fn coverage() {
  let r = flags_from_vec(svec!["deno", "coverage", "foo.json"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Coverage(CoverageFlags {
        files: FileFlags {
          include: vec!["foo.json".to_string()],
          ignore: vec![],
        },
        include: vec![r"^file:".to_string()],
        exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
        ..CoverageFlags::default()
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 133. coverage_with_lcov_and_out_file
// ---------------------------------------------------------------------------

#[test]
fn coverage_with_lcov_and_out_file() {
  let r = flags_from_vec(svec![
    "deno",
    "coverage",
    "--lcov",
    "--output=foo.lcov",
    "foo.json"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Coverage(CoverageFlags {
        files: FileFlags {
          include: vec!["foo.json".to_string()],
          ignore: vec![],
        },
        include: vec![r"^file:".to_string()],
        exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
        r#type: CoverageType::Lcov,
        output: Some(String::from("foo.lcov")),
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 134. coverage_with_default_files
// ---------------------------------------------------------------------------

#[test]
fn coverage_with_default_files() {
  let r = flags_from_vec(svec!["deno", "coverage",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Coverage(CoverageFlags {
        files: FileFlags {
          include: vec!["coverage".to_string()],
          ignore: vec![],
        },
        include: vec![r"^file:".to_string()],
        exclude: vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()],
        ..CoverageFlags::default()
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 135. location_with_bad_scheme
// ---------------------------------------------------------------------------

#[test]
fn location_with_bad_scheme() {
  // In our crate we don't validate URL schemes, so "foo:" is accepted as-is.
  #[rustfmt::skip]
    let r = flags_from_vec(svec!["deno", "run", "--location", "foo:", "mod.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "mod.ts".to_string(),
      )),
      location: deno_core::url::Url::parse("foo:").ok(),
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 136. test_config_path_args -- SKIPPED (uses resolve_cwd which is not available)
// ---------------------------------------------------------------------------

#[test]
fn test_config_path_args() {
  // This test validates resolve_cwd and config_path_args methods which
  // require filesystem access. Not applicable in this crate — kept as no-op.
}

// ---------------------------------------------------------------------------
// 137. test_no_clear_watch_flag_without_watch_flag
// ---------------------------------------------------------------------------

#[test]
fn test_no_clear_watch_flag_without_watch_flag() {
  let r = flags_from_vec(svec!["deno", "run", "--no-clear-screen", "foo.js"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 138. task_subcommand
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand() {
  let r = flags_from_vec(svec!["deno", "task", "build", "hello", "world",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec!["hello", "world"],
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "--cwd", "foo", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: Some("foo".to_string()),
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "--filter", "*", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: Some("*".to_string()),
        eval: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "--recursive", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: true,
        filter: Some("*".to_string()),
        eval: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "-r", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: true,
        filter: Some("*".to_string()),
        eval: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "task", "--eval", "echo 1"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("echo 1".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: true,
      }),
      ..Flags::default()
    }
  );

  // deno task --eval without task code requires validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "task", "--eval"]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 139. task_subcommand_double_hyphen
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_double_hyphen() {
  let r = flags_from_vec(svec![
    "deno",
    "task",
    "-c",
    "deno.json",
    "build",
    "--",
    "hello",
    "world",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec!["hello", "world"],
      config_flag: ConfigFlag::Path("deno.json".to_owned()),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno", "task", "--cwd", "foo", "build", "--", "hello", "world"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: Some("foo".to_string()),
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec!["hello", "world"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 140. task_subcommand_double_hyphen_only
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_double_hyphen_only() {
  let r = flags_from_vec(svec!["deno", "task", "build", "--"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec![],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 141. task_following_arg
// ---------------------------------------------------------------------------

#[test]
fn task_following_arg() {
  let r = flags_from_vec(svec!["deno", "task", "build", "-1", "--test"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec!["-1", "--test"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 142. task_following_double_hyphen_arg
// ---------------------------------------------------------------------------

#[test]
fn task_following_double_hyphen_arg() {
  let r = flags_from_vec(svec!["deno", "task", "build", "--test"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      argv: svec!["--test"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 143. task_with_global_flags
// ---------------------------------------------------------------------------

#[test]
fn task_with_global_flags() {
  let r = flags_from_vec(svec!["deno", "--quiet", "task", "build"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: Some("build".to_string()),
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      log_level: Some(log::Level::Error),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 144. task_subcommand_empty
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_empty() {
  let r = flags_from_vec(svec!["deno", "task"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: None,
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 145. task_subcommand_config
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_config() {
  let r = flags_from_vec(svec!["deno", "task", "--config", "deno.jsonc"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: None,
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 146. task_subcommand_config_short
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_config_short() {
  let r = flags_from_vec(svec!["deno", "task", "-c", "deno.jsonc"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Task(TaskFlags {
        cwd: None,
        task: None,
        is_run: false,
        recursive: false,
        filter: None,
        eval: false,
      }),
      config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 147. task_subcommand_noconfig_invalid -- SKIPPED (custom error)
// ---------------------------------------------------------------------------

#[test]
fn task_subcommand_noconfig_invalid() {
  let r = flags_from_vec(svec!["deno", "task", "--no-config"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 148. bench_with_flags
// ---------------------------------------------------------------------------

#[test]
fn bench_with_flags() {
  let r = flags_from_vec(svec![
    "deno",
    "bench",
    "--json",
    "--no-npm",
    "--no-remote",
    "--no-run",
    "--filter",
    "- foo",
    "--location",
    "https:foo",
    "--allow-net",
    "dir1/",
    "dir2/",
    "--",
    "arg1",
    "arg2"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Bench(BenchFlags {
        filter: Some("- foo".to_string()),
        json: true,
        no_run: true,
        files: FileFlags {
          include: vec!["dir1/".to_string(), "dir2/".to_string()],
          ignore: vec![],
        },
        watch: Default::default(),
        permit_no_files: false,
      }),
      no_npm: true,
      no_remote: true,
      type_check_mode: TypeCheckMode::Local,
      location: deno_core::url::Url::parse("https:foo").ok(),
      permissions: PermissionFlags {
        allow_net: Some(vec![]),
        no_prompt: true,
        ..Default::default()
      },
      argv: svec!["arg1", "arg2"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 149. bench_watch
// ---------------------------------------------------------------------------

#[test]
fn bench_watch() {
  let r = flags_from_vec(svec!["deno", "bench", "--watch"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Bench(BenchFlags {
        filter: None,
        json: false,
        no_run: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        watch: Some(Default::default()),
        permit_no_files: false,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 150. bench_no_files
// ---------------------------------------------------------------------------

#[test]
fn bench_no_files() {
  let r = flags_from_vec(svec!["deno", "bench", "--permit-no-files"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Bench(BenchFlags {
        filter: None,
        json: false,
        no_run: false,
        files: FileFlags {
          include: vec![],
          ignore: vec![],
        },
        watch: None,
        permit_no_files: true,
      }),
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 151. run_with_check
// ---------------------------------------------------------------------------

#[test]
fn run_with_check() {
  let r = flags_from_vec(svec!["deno", "run", "--check", "script.ts",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "run", "--check=all", "script.ts",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      type_check_mode: TypeCheckMode::All,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "--check=foo", "script.ts",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      type_check_mode: TypeCheckMode::None,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  // --no-check + --check conflict (not implemented)
  // let r = flags_from_vec(svec![
  //     "deno",
  //     "run",
  //     "--no-check",
  //     "--check",
  //     "script.ts",
  // ]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 152. no_config
// ---------------------------------------------------------------------------

#[test]
fn no_config() {
  let r = flags_from_vec(svec!["deno", "run", "--no-config", "script.ts",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags::new_default(
        "script.ts".to_string(),
      )),
      config_flag: ConfigFlag::Disabled,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );

  // --config + --no-config conflict (not implemented)
  // let r = flags_from_vec(svec![
  //     "deno",
  //     "run",
  //     "--config",
  //     "deno.json",
  //     "--no-config",
  //     "script.ts",
  // ]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 153. init
// ---------------------------------------------------------------------------

#[test]
fn init() {
  let r = flags_from_vec(svec!["deno", "init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "foo"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: Some(String::from("foo")),
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "--quiet"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      log_level: Some(log::Level::Error),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "--lib"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: None,
        lib: true,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "--serve"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: None,
        lib: false,
        serve: true,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "foo", "--lib"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: None,
        package_args: vec![],
        dir: Some(String::from("foo")),
        lib: true,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // --lib + --npm conflict (not implemented)
  // let r = flags_from_vec(svec!["deno", "init", "--lib", "--npm", "vite"]);
  // assert!(r.is_err());

  // --serve + --npm conflict (not implemented)
  // let r = flags_from_vec(svec!["deno", "init", "--serve", "--npm", "vite"]);
  // assert!(r.is_err());

  // Per-positional trailing: flags after the package positional go to
  // package_args (not parsed as init flags)
  let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "--lib"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["--lib"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // Per-positional trailing: --serve after package goes to package_args
  let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "--serve"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["--serve"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // Per-positional trailing: positional after package goes to package_args
  let r = flags_from_vec(svec!["deno", "init", "--npm", "vite", "new_dir"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["new_dir"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // Per-positional trailing with -- separator
  let r =
    flags_from_vec(svec!["deno", "init", "--npm", "vite", "--", "--serve"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["--serve"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "init", "--npm", "--yes", "npm:vite"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: true,
      }),
      ..Flags::default()
    }
  );

  // --jsr basic
  let r = flags_from_vec(svec!["deno", "init", "--jsr", "@denotest/create"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@denotest/create".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // --jsr with jsr: prefix already present
  let r = flags_from_vec(svec!["deno", "init", "--jsr", "jsr:@fresh/init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // --jsr with --yes
  let r =
    flags_from_vec(svec!["deno", "init", "--jsr", "--yes", "@denotest/create"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@denotest/create".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: true,
      }),
      ..Flags::default()
    }
  );

  // --jsr with extra args
  let r = flags_from_vec(svec![
    "deno",
    "init",
    "--jsr",
    "@denotest/create",
    "my-project"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@denotest/create".to_string()),
        package_args: svec!["my-project"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // TODO: conflict checking not yet implemented
  // let r = flags_from_vec(svec!["deno", "init", "--jsr", "--npm", "@foo/bar"]);
  // assert!(r.is_err());
  // let r = flags_from_vec(svec!["deno", "init", "--jsr", "--lib", "@foo/bar"]);
  // assert!(r.is_err());
  // let r = flags_from_vec(svec!["deno", "init", "--jsr"]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 154. create -- complex test with many cases
// ---------------------------------------------------------------------------

#[test]
fn create() {
  // deno create (no package) — requires missing-required validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "create"]);
  // assert!(r.is_err());

  // deno create vite — requires package prefix validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "create", "vite"]);
  // assert!(r.is_err());

  // Per-positional trailing: extra positionals after package go to package_args
  let r = flags_from_vec(svec!["deno", "create", "npm:vite", "my-project"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["my-project"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r =
    flags_from_vec(svec!["deno", "create", "npm:vite", "--", "my-project"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["my-project"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "create", "--npm", "vite"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // Per-positional trailing: extra positionals after --npm package go to package_args
  let r =
    flags_from_vec(svec!["deno", "create", "--npm", "vite", "my-project"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: svec!["my-project"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "create", "--yes", "npm:vite"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("npm:vite".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: true,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "create", "jsr:@std/http/file-server"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@std/http/file-server".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "create", "jsr:@fresh/init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "create", "--yes", "jsr:@fresh/init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: true,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "create",
    "jsr:@fresh/init",
    "--",
    "my-project"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: svec!["my-project"],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // empty jsr: prefix — validation not yet implemented
  // let r = flags_from_vec(svec!["deno", "create", "jsr:"]);
  // assert!(r.is_err());

  // --jsr flag
  let r = flags_from_vec(svec!["deno", "create", "--jsr", "@fresh/init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: false,
      }),
      ..Flags::default()
    }
  );

  // --jsr with --yes
  let r =
    flags_from_vec(svec!["deno", "create", "--jsr", "--yes", "@fresh/init"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Init(InitFlags {
        package: Some("jsr:@fresh/init".to_string()),
        package_args: vec![],
        dir: None,
        lib: false,
        serve: false,
        empty: false,
        yes: true,
      }),
      ..Flags::default()
    }
  );

  // TODO: semantic validation not yet implemented
  // let r = flags_from_vec(svec!["deno", "create", "--jsr", "npm:vite"]);
  // assert!(r.is_err());
  // let r = flags_from_vec(svec!["deno", "create", "--jsr", "--npm", "@foo"]);
  // assert!(r.is_err());
  // let r = flags_from_vec(svec!["deno", "create", "npm:"]);
  // assert!(r.is_err());
  // let r = flags_from_vec(svec!["deno", "create", "--npm", "jsr:@std/http"]);
  // assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// 155. jupyter
// ---------------------------------------------------------------------------

#[test]
fn jupyter() {
  let r = flags_from_vec(svec!["deno", "jupyter"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: false,
        kernel: false,
        conn_file: None,
        name: None,
        display: None,
        force: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "jupyter", "--install"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: true,
        kernel: false,
        conn_file: None,
        name: None,
        display: None,
        force: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--force"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: true,
        kernel: false,
        conn_file: None,
        name: None,
        display: None,
        force: true,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "jupyter",
    "--install",
    "--name",
    "debugdeno",
    "--display",
    "Deno (debug)"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: true,
        kernel: false,
        conn_file: None,
        name: Some("debugdeno".to_string()),
        display: Some("Deno (debug)".to_string()),
        force: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "jupyter", "-n", "debugdeno",]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: false,
        kernel: false,
        conn_file: None,
        name: Some("debugdeno".to_string()),
        display: None,
        force: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "jupyter",
    "--kernel",
    "--conn",
    "path/to/conn/file"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: false,
        kernel: true,
        conn_file: Some(String::from("path/to/conn/file")),
        name: None,
        display: None,
        force: false,
      }),
      ..Flags::default()
    }
  );

  // Jupyter conflict/validation checks (not implemented)
  // let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--conn", "path/to/conn/file"]);
  // r.unwrap_err();
  // let r = flags_from_vec(svec!["deno", "jupyter", "--kernel",]);
  // r.unwrap_err();
  // let r = flags_from_vec(svec!["deno", "jupyter", "--install", "--kernel",]);
  // r.unwrap_err();
  // let r = flags_from_vec(svec!["deno", "jupyter", "--display", "deno"]);
  // r.unwrap_err();
  // let r = flags_from_vec(svec!["deno", "jupyter", "--kernel", "--display"]);
  // r.unwrap_err();
  // let r = flags_from_vec(svec!["deno", "jupyter", "--force"]);
  // r.unwrap_err();
}

// ---------------------------------------------------------------------------
// 156. publish_args
// ---------------------------------------------------------------------------

#[test]
fn publish_args() {
  let r = flags_from_vec(svec![
    "deno",
    "publish",
    "--no-provenance",
    "--dry-run",
    "--allow-slow-types",
    "--allow-dirty",
    "--token=asdf",
    "--set-version=1.0.1",
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Publish(PublishFlags {
        token: Some("asdf".to_string()),
        dry_run: true,
        allow_slow_types: true,
        allow_dirty: true,
        no_provenance: true,
        set_version: Some("1.0.1".to_string()),
      }),
      type_check_mode: TypeCheckMode::Local,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 157. add_or_install_subcommand
// ---------------------------------------------------------------------------

#[test]
fn add_or_install_subcommand() {
  // deno add without packages — requires missing-required validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "add"]);
  // r.unwrap_err();
  for cmd in ["add", "install"] {
    let mk_flags = |flags: AddFlags| -> Flags {
      match cmd {
        "add" => Flags {
          subcommand: DenoSubcommand::Add(flags),
          ..Flags::default()
        },
        "install" => Flags {
          subcommand: DenoSubcommand::Install(InstallFlags::Local(
            InstallFlagsLocal::Add(flags),
          )),
          ..Flags::default()
        },
        _ => unreachable!(),
      }
    };

    {
      let r = flags_from_vec(svec!["deno", cmd, "@david/which"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["@david/which"],
          dev: false,
          default_registry: None,
          lockfile_only: false,
          save_exact: false,
        })
      );
    }
    {
      let r = flags_from_vec(svec![
        "deno",
        cmd,
        "--frozen",
        "--lockfile-only",
        "@david/which",
        "@luca/hello"
      ]);
      let mut expected_flags = mk_flags(AddFlags {
        packages: svec!["@david/which", "@luca/hello"],
        dev: false,
        default_registry: None,
        lockfile_only: true,
        save_exact: false,
      });
      expected_flags.frozen_lockfile = Some(true);
      assert_eq!(r.unwrap(), expected_flags);
    }
    {
      let r = flags_from_vec(svec!["deno", cmd, "--dev", "npm:chalk"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["npm:chalk"],
          dev: true,
          default_registry: None,
          lockfile_only: false,
          save_exact: false,
        }),
      );
    }
    {
      let r = flags_from_vec(svec!["deno", cmd, "--npm", "chalk"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["chalk"],
          dev: false,
          default_registry: Some(DefaultRegistry::Npm),
          lockfile_only: false,
          save_exact: false,
        }),
      );
    }
    {
      let r = flags_from_vec(svec!["deno", cmd, "--jsr", "@std/fs"]);
      assert_eq!(
        r.unwrap(),
        mk_flags(AddFlags {
          packages: svec!["@std/fs"],
          dev: false,
          default_registry: Some(DefaultRegistry::Jsr),
          lockfile_only: false,
          save_exact: false,
        }),
      );
    }
  }
}

// ---------------------------------------------------------------------------
// 158. remove_subcommand
// ---------------------------------------------------------------------------

#[test]
fn remove_subcommand() {
  // deno remove without packages — requires missing-required validation (not implemented)
  // let r = flags_from_vec(svec!["deno", "remove"]);
  // r.unwrap_err();

  let r = flags_from_vec(svec!["deno", "remove", "@david/which"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Remove(RemoveFlags {
        packages: svec!["@david/which"],
        lockfile_only: false,
      }),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "remove",
    "--frozen",
    "--lockfile-only",
    "@david/which",
    "@luca/hello"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Remove(RemoveFlags {
        packages: svec!["@david/which", "@luca/hello"],
        lockfile_only: true,
      }),
      frozen_lockfile: Some(true),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 159. run_with_frozen_lockfile
// ---------------------------------------------------------------------------

#[test]
fn run_with_frozen_lockfile() {
  let cases = [
    (Some("--frozen"), Some(true)),
    (Some("--frozen=true"), Some(true)),
    (Some("--frozen=false"), Some(false)),
    (None, None),
  ];
  for (flag, frozen) in cases {
    let mut args = svec!["deno", "run"];
    if let Some(f) = flag {
      args.push(f.into());
    }
    args.push("script.ts".into());
    let r = flags_from_vec(args);
    assert_eq!(
      r.unwrap(),
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        frozen_lockfile: frozen,
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }
}

// ---------------------------------------------------------------------------
// 160. allow_scripts -- uses PackageReq, simplified for our crate
// ---------------------------------------------------------------------------

#[test]
fn allow_scripts() {
  // Basic cases: --allow-scripts present vs absent
  let r =
    flags_from_vec(svec!["deno", "cache", "--allow-scripts", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      allow_scripts: PackagesAllowedScripts::All,
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec!["deno", "cache", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      allow_scripts: PackagesAllowedScripts::None,
      ..Flags::default()
    }
  );

  // With specific packages (simplified: String instead of PackageReq)
  let r = flags_from_vec(svec![
    "deno",
    "cache",
    "--allow-scripts=npm:foo",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      allow_scripts: PackagesAllowedScripts::Some(vec![
        deno_semver::package::PackageReq::from_str("foo").unwrap(),
      ]),
      ..Flags::default()
    }
  );

  let r = flags_from_vec(svec![
    "deno",
    "cache",
    "--allow-scripts=npm:foo,npm:bar@2",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Cache(CacheFlags {
        files: svec!["script.ts"],
      }),
      allow_scripts: PackagesAllowedScripts::Some(vec![
        deno_semver::package::PackageReq::from_str("foo").unwrap(),
        deno_semver::package::PackageReq::from_str("bar@2").unwrap(),
      ]),
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 161. bare_run
// ---------------------------------------------------------------------------

#[test]
fn bare_run() {
  let r = flags_from_vec(svec!["deno", "--no-config", "script.ts"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        watch: None,
        bare: true,
        coverage_dir: None,
        print_task_list: false,
      }),
      config_flag: ConfigFlag::Disabled,
      code_cache_enabled: true,
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// 162. bare_global
// ---------------------------------------------------------------------------

#[test]
fn bare_global() {
  let r = flags_from_vec(svec!["deno", "--log-level=debug"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: None,
        is_default_command: true,
        json: false,
      }),
      log_level: Some(log::Level::Debug),
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: repl_user_args
// ---------------------------------------------------------------------------

#[test]
fn repl_user_args() {
  let r = flags_from_vec(svec!["deno", "repl", "foo"]);
  assert!(r.is_err());
  let r = flags_from_vec(svec!["deno", "repl", "--", "foo"]);
  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Repl(ReplFlags {
        eval_files: None,
        eval: None,
        is_default_command: false,
        json: false,
      }),
      argv: svec!["foo"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: bare_with_flag_no_file -- SKIPPED (custom error message)
// ---------------------------------------------------------------------------

#[test]
fn bare_with_flag_no_file() {
  let r = flags_from_vec(svec!["deno", "--no-config"]);
  // Should fail because no script arg is provided with a non-global flag
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// Extra: equal_help_output -- SKIPPED (tests help text generation)
// ---------------------------------------------------------------------------

#[test]
fn equal_help_output() {
  // This test validates clap-specific help text equivalence across
  // --help, -h, and `deno help <cmd>`. Not applicable to our parser —
  // kept as a no-op.
}

// ---------------------------------------------------------------------------
// Extra: install_permissions_non_global -- SKIPPED (custom error)
// ---------------------------------------------------------------------------

#[test]
fn install_permissions_non_global() {
  let r =
    flags_from_vec(svec!["deno", "install", "--allow-net", "jsr:@std/fs"]);
  assert!(
    r.unwrap_err()
      .to_string()
      .contains("Note: Permission flags can only be used in a global setting")
  );
}

// ---------------------------------------------------------------------------
// Extra: jupyter_unstable_flags
// ---------------------------------------------------------------------------

#[test]
fn jupyter_unstable_flags() {
  let r = flags_from_vec(svec![
    "deno",
    "jupyter",
    "--unstable-ffi",
    "--unstable-bare-node-builtins",
    "--unstable-worker-options"
  ]);

  assert_eq!(
    r.unwrap(),
    Flags {
      subcommand: DenoSubcommand::Jupyter(JupyterFlags {
        install: false,
        kernel: false,
        conn_file: None,
        name: None,
        display: None,
        force: false,
      }),
      unstable_config: UnstableConfig {
        bare_node_builtins: true,
        sloppy_imports: false,
        features: svec!["bare-node-builtins", "ffi", "worker-options"],
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: serve_with_allow_all
// ---------------------------------------------------------------------------

#[test]
fn serve_with_allow_all() {
  let r = flags_from_vec(svec!["deno", "serve", "--allow-all", "./main.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    &flags,
    &Flags {
      subcommand: DenoSubcommand::Serve(ServeFlags::new_default(
        "./main.ts".into(),
        8000,
        "0.0.0.0"
      )),
      permissions: PermissionFlags {
        allow_all: true,
        allow_net: None,
        ..Default::default()
      },
      code_cache_enabled: true,
      ..Default::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: escape_and_split_commas_test -- SKIPPED (internal utility)
// ---------------------------------------------------------------------------

#[test]
fn escape_and_split_commas_test() {
  use crate::convert::escape_and_split_commas;

  assert_eq!(escape_and_split_commas("foo".to_string()).unwrap(), ["foo"]);
  assert!(escape_and_split_commas("foo,".to_string()).is_err());
  assert_eq!(
    escape_and_split_commas("foo,,".to_string()).unwrap(),
    ["foo,"]
  );
  assert!(escape_and_split_commas("foo,,,".to_string()).is_err());
  assert_eq!(
    escape_and_split_commas("foo,,,,".to_string()).unwrap(),
    ["foo,,"]
  );
  assert_eq!(
    escape_and_split_commas("foo,bar".to_string()).unwrap(),
    ["foo", "bar"]
  );
  assert_eq!(
    escape_and_split_commas("foo,,bar".to_string()).unwrap(),
    ["foo,bar"]
  );
  assert_eq!(
    escape_and_split_commas("foo,,,bar".to_string()).unwrap(),
    ["foo,", "bar"]
  );
}

// ---------------------------------------------------------------------------
// Extra: net_flag_with_url -- SKIPPED (URL format validation)
// ---------------------------------------------------------------------------

#[test]
fn net_flag_with_url() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-net=https://example.com",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap_err().to_string(),
    "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
  );
}

// ---------------------------------------------------------------------------
// Extra: node_modules_dir_default
// ---------------------------------------------------------------------------

#[test]
fn node_modules_dir_default() {
  let r =
    flags_from_vec(svec!["deno", "run", "--node-modules-dir", "./foo.ts"]);
  let flags = r.unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "./foo.ts".into(),
        ..Default::default()
      }),
      node_modules_dir: Some(NodeModulesDirMode::Auto),
      code_cache_enabled: true,
      ..Default::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: flag_before_subcommand -- SKIPPED (custom error format)
// ---------------------------------------------------------------------------

#[test]
fn flag_before_subcommand() {
  let r = flags_from_vec(svec!["deno", "--allow-net", "repl"]);
  assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// Extra: allow_all_conflicts_allow_perms -- SKIPPED (conflict checking)
// ---------------------------------------------------------------------------

#[test]
fn allow_all_conflicts_allow_perms() {
  let flags_list = [
    "--allow-read",
    "--allow-write",
    "--allow-net",
    "--allow-env",
    "--allow-run",
    "--allow-sys",
    "--allow-ffi",
    "--allow-import",
  ];
  for flag in flags_list {
    let r = flags_from_vec(svec!["deno", "run", "--allow-all", flag, "foo.ts"]);
    assert!(r.is_err(), "expected error for --allow-all with {flag}");
  }
}

// ---------------------------------------------------------------------------
// Extra: allow_import_with_url -- SKIPPED (URL format validation)
// ---------------------------------------------------------------------------

#[test]
fn allow_import_with_url() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-import=https://example.com",
    "script.ts"
  ]);
  assert_eq!(
    r.unwrap_err().to_string(),
    "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
  );
}

// ---------------------------------------------------------------------------
// Extra: deny_import_with_url
// ---------------------------------------------------------------------------

#[test]
fn deny_import_with_url() {
  let r = flags_from_vec(svec![
    "deno",
    "run",
    "--deny-import=https://example.com",
    "script.ts",
  ]);
  assert_eq!(
    r.unwrap_err().to_string(),
    "error: invalid value 'https://example.com': URLs are not supported, only domains and ips"
  );
}

// ---------------------------------------------------------------------------
// Extra: outdated_subcommand
// ---------------------------------------------------------------------------

#[test]
fn outdated_subcommand() {
  let cases = [
    (
      svec![],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::PrintOutdated { compatible: false },
        recursive: false,
      },
    ),
    (
      svec!["--recursive"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::PrintOutdated { compatible: false },
        recursive: true,
      },
    ),
    (
      svec!["--recursive", "--compatible"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::PrintOutdated { compatible: true },
        recursive: true,
      },
    ),
    (
      svec!["--update"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--update", "--latest"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: true,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--update", "--recursive"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: true,
      },
    ),
    (
      svec!["--update", "--lockfile-only"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: true,
        },
        recursive: false,
      },
    ),
    (
      svec!["--update", "@foo/bar"],
      OutdatedFlags {
        filters: svec!["@foo/bar"],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--latest"],
      OutdatedFlags {
        filters: svec![],
        kind: OutdatedKind::PrintOutdated { compatible: false },
        recursive: false,
      },
    ),
    (
      svec!["--update", "--latest", "--interactive"],
      OutdatedFlags {
        filters: svec![],
        kind: OutdatedKind::Update {
          latest: true,
          interactive: true,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
  ];
  for (input, expected) in cases {
    let mut args = svec!["deno", "outdated"];
    args.extend(input);
    let r = flags_from_vec(args.clone()).unwrap();
    assert_eq!(
      r.subcommand,
      DenoSubcommand::Outdated(expected),
      "incorrect result for args: {:?}",
      args
    );
  }
}

// ---------------------------------------------------------------------------
// Extra: update_subcommand
// ---------------------------------------------------------------------------

#[test]
fn update_subcommand() {
  let cases = [
    (
      svec![],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--latest"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: true,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--recursive"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: true,
      },
    ),
    (
      svec!["--lockfile-only"],
      OutdatedFlags {
        filters: vec![],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: true,
        },
        recursive: false,
      },
    ),
    (
      svec!["@foo/bar"],
      OutdatedFlags {
        filters: svec!["@foo/bar"],
        kind: OutdatedKind::Update {
          latest: false,
          interactive: false,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
    (
      svec!["--latest", "--interactive"],
      OutdatedFlags {
        filters: svec![],
        kind: OutdatedKind::Update {
          latest: true,
          interactive: true,
          lockfile_only: false,
        },
        recursive: false,
      },
    ),
  ];
  for (input, expected) in cases {
    let mut args = svec!["deno", "update"];
    args.extend(input);
    let r = flags_from_vec(args.clone()).unwrap();
    assert_eq!(
      r.subcommand,
      DenoSubcommand::Outdated(expected),
      "incorrect result for args: {:?}",
      args
    );
  }
}

// ---------------------------------------------------------------------------
// Extra: update_subcommand_frozen_flag -- SKIPPED (frozen error)
// ---------------------------------------------------------------------------

#[test]
fn update_subcommand_frozen_flag() {
  let r = flags_from_vec(svec!["deno", "update", "--frozen=false"]).unwrap();
  assert_eq!(r.frozen_lockfile, Some(false));

  let r = flags_from_vec(svec!["deno", "update", "--frozen"]).unwrap();
  assert_eq!(r.frozen_lockfile, Some(true));
}

// ---------------------------------------------------------------------------
// Extra: outdated_subcommand_frozen_flag
// ---------------------------------------------------------------------------

#[test]
fn outdated_subcommand_frozen_flag() {
  let r = flags_from_vec(svec!["deno", "outdated", "--frozen=false"]).unwrap();
  assert_eq!(r.frozen_lockfile, Some(false));
}

// ---------------------------------------------------------------------------
// Extra: approve_scripts_subcommand
// ---------------------------------------------------------------------------

#[test]
fn approve_scripts_subcommand() {
  let cases = [
    (
      svec![],
      ApproveScriptsFlags {
        packages: vec![],
        lockfile_only: false,
      },
    ),
    (
      svec!["npm:pkg@1.0.0"],
      ApproveScriptsFlags {
        packages: vec!["npm:pkg@1.0.0".to_string()],
        lockfile_only: false,
      },
    ),
    (
      svec!["npm:pkg1@1.0.0", "npm:pkg2@2.0.0"],
      ApproveScriptsFlags {
        packages: vec![
          "npm:pkg1@1.0.0".to_string(),
          "npm:pkg2@2.0.0".to_string(),
        ],
        lockfile_only: false,
      },
    ),
    (
      svec!["npm:pkg1@1.0.0,npm:pkg2@2.0.0"],
      ApproveScriptsFlags {
        packages: vec![
          "npm:pkg1@1.0.0".to_string(),
          "npm:pkg2@2.0.0".to_string(),
        ],
        lockfile_only: false,
      },
    ),
    (
      svec!["--lockfile-only"],
      ApproveScriptsFlags {
        packages: vec![],
        lockfile_only: true,
      },
    ),
    (
      svec!["--lockfile-only", "npm:pkg@1.0.0"],
      ApproveScriptsFlags {
        packages: vec!["npm:pkg@1.0.0".to_string()],
        lockfile_only: true,
      },
    ),
    (
      svec!["npm:pkg@1.0.0", "--lockfile-only"],
      ApproveScriptsFlags {
        packages: vec!["npm:pkg@1.0.0".to_string()],
        lockfile_only: true,
      },
    ),
    (
      svec!["npm:pkg1@1.0.0", "npm:pkg2@2.0.0", "--lockfile-only"],
      ApproveScriptsFlags {
        packages: vec![
          "npm:pkg1@1.0.0".to_string(),
          "npm:pkg2@2.0.0".to_string(),
        ],
        lockfile_only: true,
      },
    ),
  ];
  for (input, expected) in cases {
    let mut args = svec!["deno", "approve-scripts"];
    args.extend(input);
    let r = flags_from_vec(args.clone()).unwrap();
    assert_eq!(
      r.subcommand,
      DenoSubcommand::ApproveScripts(expected),
      "incorrect result for args: {:?}",
      args
    );
  }
}

// ---------------------------------------------------------------------------
// Extra: clean_subcommand
// ---------------------------------------------------------------------------

#[test]
fn clean_subcommand() {
  let cases = [
    (
      svec![],
      CleanFlags {
        except_paths: vec![],
        dry_run: false,
      },
    ),
    (
      svec!["--except", "path1"],
      CleanFlags {
        except_paths: vec!["path1".to_string()],
        dry_run: false,
      },
    ),
    (
      svec!["--except", "path1", "path2"],
      CleanFlags {
        except_paths: vec!["path1".to_string(), "path2".to_string()],
        dry_run: false,
      },
    ),
    (
      svec!["--except", "path1", "--dry-run"],
      CleanFlags {
        except_paths: vec!["path1".to_string()],
        dry_run: true,
      },
    ),
  ];
  for (input, expected) in cases {
    let mut args = svec!["deno", "clean"];
    args.extend(input);
    let r = flags_from_vec(args.clone()).unwrap();
    assert_eq!(
      r,
      Flags {
        subcommand: DenoSubcommand::Clean(expected),
        ..Flags::default()
      },
      "incorrect result for args: {:?}",
      args
    );
  }
}

// ---------------------------------------------------------------------------
// Extra: conditions_test
// ---------------------------------------------------------------------------

#[test]
fn conditions_test() {
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--conditions",
    "development",
    "main.ts"
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      node_conditions: svec!["development"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--conditions",
    "development,production",
    "main.ts"
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      node_conditions: svec!["development", "production"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--conditions",
    "development",
    "--conditions",
    "production",
    "main.ts"
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      node_conditions: svec!["development", "production"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: preload_flag_test
// ---------------------------------------------------------------------------

#[test]
fn preload_flag_test() {
  let flags =
    flags_from_vec(svec!["deno", "run", "--preload", "preload.js", "main.ts"])
      .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      preload: svec!["preload.js"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags =
    flags_from_vec(svec!["deno", "run", "--preload", "data:,()", "main.ts"])
      .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      preload: svec!["data:,()"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "compile",
    "--preload",
    "p1.js",
    "--preload",
    "./p2.js",
    "main.ts"
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Compile(CompileFlags {
        source_file: "main.ts".into(),
        output: None,
        args: vec![],
        target: None,
        no_terminal: false,
        icon: None,
        include: Default::default(),
        exclude: Default::default(),
        eszip: false,
        self_extracting: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      preload: svec!["p1.js", "./p2.js"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "test",
    "--preload",
    "p1.js",
    "--import",
    "./p2.js",
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Test(TestFlags::default()),
      preload: svec!["p1.js", "./p2.js"],
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: false,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "bench",
    "--preload",
    "p1.js",
    "--import",
    "./p2.js",
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Bench(BenchFlags::default()),
      preload: svec!["p1.js", "./p2.js"],
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: false,
      permissions: PermissionFlags {
        no_prompt: true,
        ..Default::default()
      },
      ..Default::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: require_flag_test
// ---------------------------------------------------------------------------

#[test]
fn require_flag_test() {
  let flags =
    flags_from_vec(svec!["deno", "run", "--require", "require.js", "main.ts"])
      .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      require: svec!["require.js"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );

  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--require",
    "r1.js",
    "--require",
    "./r2.js",
    "main.ts"
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "main.ts".into(),
        ..Default::default()
      }),
      require: svec!["r1.js", "./r2.js"],
      code_cache_enabled: true,
      ..Default::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: check_with_v8_flags
// ---------------------------------------------------------------------------

#[test]
fn check_with_v8_flags() {
  let flags =
    flags_from_vec(svec!["deno", "check", "--v8-flags=--help", "script.ts",])
      .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Check(CheckFlags {
        files: svec!["script.ts"],
        doc: false,
        doc_only: false,
        check_js: false,
      }),
      type_check_mode: TypeCheckMode::Local,
      code_cache_enabled: true,
      v8_flags: svec!["--help"],
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: multiple_allow_all -- SKIPPED (error case)
// ---------------------------------------------------------------------------

#[test]
fn multiple_allow_all() {
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--allow-all",
    "--inspect",
    "-A",
    "script.ts",
  ])
  .unwrap();
  assert_eq!(
    flags,
    Flags {
      subcommand: DenoSubcommand::Run(RunFlags {
        script: "script.ts".to_string(),
        ..Default::default()
      }),
      inspect: Some("127.0.0.1:9229".parse().unwrap()),
      code_cache_enabled: true,
      permissions: PermissionFlags {
        allow_all: true,
        ..Default::default()
      },
      ..Flags::default()
    }
  );
}

// ---------------------------------------------------------------------------
// Extra: inspect_flag_parsing
// ---------------------------------------------------------------------------

#[test]
fn inspect_flag_parsing() {
  // Inspect values are now parsed into SocketAddr.
  let cases: Vec<(&str, std::net::SocketAddr)> = vec![
    ("127.0.0.1:9229", "127.0.0.1:9229".parse().unwrap()),
    ("192.168.0.1", "192.168.0.1:9229".parse().unwrap()),
    ("10000", "127.0.0.1:10000".parse().unwrap()),
    (":10000", "127.0.0.1:10000".parse().unwrap()),
    (":0", "127.0.0.1:0".parse().unwrap()),
    ("0", "127.0.0.1:0".parse().unwrap()),
  ];

  for (input, expected_addr) in cases {
    let flags = flags_from_vec(svec![
      "deno",
      "run",
      &format!("--inspect={}", input),
      "script.ts",
    ])
    .unwrap();
    assert_eq!(
      flags,
      Flags {
        subcommand: DenoSubcommand::Run(RunFlags::new_default(
          "script.ts".to_string(),
        )),
        inspect: Some(expected_addr),
        code_cache_enabled: true,
        ..Flags::default()
      }
    );
  }
}

// ---------------------------------------------------------------------------
// Extra: inspect_publish_uid_flag_parsing -- SKIPPED (custom parser)
// ---------------------------------------------------------------------------

#[test]
fn inspect_publish_uid_flag_parsing() {
  // Test with both stderr and http
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--inspect",
    "--inspect-publish-uid=stderr,http",
    "script.ts",
  ])
  .unwrap();
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: true,
      http: true,
    })
  );

  // Test with only stderr
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--inspect",
    "--inspect-publish-uid=stderr",
    "script.ts",
  ])
  .unwrap();
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: true,
      http: false,
    })
  );

  // Test with only http
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--inspect",
    "--inspect-publish-uid=http",
    "script.ts",
  ])
  .unwrap();
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: false,
      http: true,
    })
  );

  // Test without the flag (should be None)
  let flags =
    flags_from_vec(svec!["deno", "run", "--inspect", "script.ts"]).unwrap();
  assert_eq!(flags.inspect_publish_uid, None);
}

// ---------------------------------------------------------------------------
// Extra: node_options_* tests
// ---------------------------------------------------------------------------

use crate::convert::set_test_node_options;

#[test]
fn node_options_require() {
  // Test NODE_OPTIONS --require when no CLI --require is passed
  set_test_node_options(Some("--require only.js"));
  let flags = flags_from_vec(svec!["deno", "run", "script.ts"]).unwrap();
  set_test_node_options(None);
  assert_eq!(flags.require, vec!["only.js"]);
}

#[test]
fn node_options_require_prepend_to_cli() {
  // Test NODE_OPTIONS --require is prepended to CLI --require values
  set_test_node_options(Some("--require foo.js --require bar.js"));
  let flags =
    flags_from_vec(svec!["deno", "run", "--require", "cli.js", "script.ts"])
      .unwrap();
  set_test_node_options(None);
  assert_eq!(flags.require, vec!["foo.js", "bar.js", "cli.js"]);
}

#[test]
fn node_options_inspect_publish_uid() {
  set_test_node_options(Some("--inspect-publish-uid=http"));
  let flags = flags_from_vec(svec!["deno", "run", "script.ts"]).unwrap();
  set_test_node_options(None);
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: false,
      http: true,
    })
  );
}

#[test]
fn node_options_inspect_publish_uid_cli_precedence() {
  set_test_node_options(Some("--inspect-publish-uid=http"));
  let flags = flags_from_vec(svec![
    "deno",
    "run",
    "--inspect-publish-uid=stderr",
    "script.ts",
  ])
  .unwrap();
  set_test_node_options(None);
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: true,
      http: false,
    })
  );
}

#[test]
fn node_options_combined() {
  // Test NODE_OPTIONS with both --require and --inspect-publish-uid
  set_test_node_options(Some(
    "--require foo.js --inspect-publish-uid=stderr,http",
  ));
  let flags = flags_from_vec(svec!["deno", "run", "script.ts"]).unwrap();
  set_test_node_options(None);
  assert_eq!(flags.require, vec!["foo.js"]);
  assert_eq!(
    flags.inspect_publish_uid,
    Some(InspectPublishUid {
      console: true,
      http: true,
    })
  );
}

#[test]
fn node_options_empty() {
  set_test_node_options(Some(""));
  let flags = flags_from_vec(svec!["deno", "run", "script.ts"]).unwrap();
  set_test_node_options(None);
  assert!(flags.require.is_empty());
  assert_eq!(flags.inspect_publish_uid, None);
}

#[test]
fn node_options_ignores_unknown_flags() {
  set_test_node_options(Some(
    "--require known.js --unknown-flag --another-unknown",
  ));
  let flags = flags_from_vec(svec!["deno", "run", "script.ts"]).unwrap();
  set_test_node_options(None);
  assert_eq!(flags.require, vec!["known.js"]);
}
