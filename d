[1mdiff --git a/cli/flags.rs b/cli/flags.rs[m
[1mindex 5ae2f902..b6eedadc 100644[m
[1m--- a/cli/flags.rs[m
[1m+++ b/cli/flags.rs[m
[36m@@ -535,14 +535,8 @@[m [mfn eval_parse(flags: &mut Flags, matches: &clap::ArgMatches) {[m
   flags.allow_write = Some(vec![]);[m
   flags.allow_plugin = true;[m
   flags.allow_hrtime = true;[m
[31m-  // TODO(@satyarohith): remove this flag in 2.0.[m
[31m-  let as_typescript = matches.is_present("ts");[m
[31m-  let ext = if as_typescript {[m
[31m-    "ts".to_string()[m
[31m-  } else {[m
[31m-    matches.value_of("ext").unwrap().to_string()[m
[31m-  };[m
 [m
[32m+[m[32m  let ext = matches.value_of("ext").unwrap().to_string();[m
   let print = matches.is_present("print");[m
   let mut code: Vec<String> = matches[m
     .values_of("code_arg")[m
[36m@@ -1027,16 +1021,6 @@[m [mTo evaluate as TypeScript:[m
 [m
 This command has implicit access to all permissions (--allow-all).",[m
     )[m
[31m-    .arg([m
[31m-      // TODO(@satyarohith): remove this argument in 2.0.[m
[31m-      Arg::with_name("ts")[m
[31m-        .long("ts")[m
[31m-        .short("T")[m
[31m-        .help("Treat eval input as TypeScript")[m
[31m-        .takes_value(false)[m
[31m-        .multiple(false)[m
[31m-        .hidden(true),[m
[31m-    )[m
     .arg([m
       Arg::with_name("ext")[m
         .long("ext")[m
[36m@@ -2439,7 +2423,7 @@[m [mmod tests {[m
   #[test][m
   fn eval_typescript() {[m
     let r =[m
[31m-      flags_from_vec(svec!["deno", "eval", "-T", "'console.log(\"hello\")'"]);[m
[32m+[m[32m      flags_from_vec(svec!["deno", "eval", "--ext=ts", "'console.log(\"hello\")'"]);[m
     assert_eq!([m
       r.unwrap(),[m
       Flags {[m
