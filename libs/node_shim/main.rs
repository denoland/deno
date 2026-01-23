// Copyright the Deno authors. MIT license.

use exec::execvp;
use node_shim::{TranslateOptions, translate_to_deno_args};
use std::env;
use std::process::{self, Stdio};

fn main() {
    let args = env::args().skip(1).collect::<Vec<String>>();

    let parsed_args = match node_shim::parse_args(args) {
        Ok(parsed_args) => parsed_args,
        Err(e) => {
            if e.len() == 1 {
                eprintln!("Error: {}", e[0]);
            } else if e.len() > 1 {
                eprintln!("Errors: {}", e.join(", "));
            }
            process::exit(1);
        }
    };

    // Handle --help specially for CLI
    if parsed_args.options.print_help {
        println!("This is a shim that translates Node CLI arguments to Deno CLI arguments.");
        println!("Use exactly like you would use Node.js, but it will run with Deno.");
        process::exit(0);
    }

    let options = TranslateOptions::for_node_cli();
    let result = translate_to_deno_args(parsed_args, &options);

    // Set DENO_TLS_CA_STORE if needed
    if result.use_system_ca {
        unsafe { std::env::set_var("DENO_TLS_CA_STORE", "system") };
    }

    let mut deno_args = result.deno_args;

    // Handle entrypoint resolution for run commands
    if deno_args.len() >= 3 && deno_args.get(1) == Some(&"run".to_string()) {
        // Find the entrypoint (first non-flag arg after "run")
        let mut entrypoint_idx = None;
        for (i, arg) in deno_args.iter().enumerate().skip(2) {
            if !arg.starts_with('-') && !arg.starts_with("--") {
                entrypoint_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = entrypoint_idx {
            let entrypoint = &deno_args[idx];
            let resolved = resolve_entrypoint(entrypoint);
            deno_args[idx] = resolved;
        }
    }

    if std::env::var("NODE_SHIM_DEBUG").is_ok() {
        eprintln!("deno {:?}", deno_args);
        process::exit(0);
    }

    // Execute deno with the translated arguments
    let err = execvp("deno", &deno_args);
    eprintln!("Failed to execute deno: {}", err);
    process::exit(1);
}

fn resolve_entrypoint(entrypoint: &str) -> String {
    let cwd = env::current_dir().unwrap();
    // If the entrypoint is either an absolute path, or a relative path that exists,
    // return it as is.
    if cwd.join(entrypoint).symlink_metadata().is_ok() {
        return entrypoint.to_string();
    }

    let url = url::Url::from_file_path(cwd.join("$file.js")).unwrap();

    // Otherwise, shell out to `deno` to try to resolve the entrypoint.
    let output = process::Command::new("deno")
        .arg("eval")
        .arg("--no-config")
        .arg(include_str!("./resolve.js"))
        .arg(url.to_string())
        .arg(format!("./{}", entrypoint))
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .expect("Failed to execute deno resolve script");
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }
    let resolved_path = String::from_utf8(output.stdout)
        .expect("Failed to parse deno resolve output")
        .trim()
        .to_string();
    resolved_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use node_shim::parse_args;

    /// Macro to create a Vec<String> from string literals
    macro_rules! svec {
        ($($x:expr),* $(,)?) => {
            vec![$($x.to_string()),*]
        };
    }

    /// Test that takes a `input: ["node"]` and `expected: ["deno", "repl", "-A", "--"] `
    macro_rules! test {
        ($name:ident, $input:tt , $expected:tt) => {
            #[test]
            fn $name() {
                let parsed_args = parse_args(svec! $input).unwrap();
                let options = TranslateOptions::for_node_cli();
                let result = translate_to_deno_args(parsed_args, &options);
                assert_eq!(result.deno_args, svec! $expected);
            }
        };
    }

    test!(test_repl_no_args, [], ["node", "repl", "-A", "--"]);

    test!(
        test_run_script,
        ["foo.js"],
        [
            "node",
            "run",
            "-A",
            "--unstable-node-globals",
            "--unstable-bare-node-builtins",
            "--unstable-detect-cjs",
            "--node-modules-dir=manual",
            "--no-config",
            "foo.js"
        ]
    );
}
