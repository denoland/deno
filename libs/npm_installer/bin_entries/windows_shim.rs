// Copyright 2018-2025 the Deno authors. MIT license.

// Windows shim generation ported from https://github.com/npm/cmd-shim
// Original code licensed under the ISC License:
//
// The ISC License
//
// Copyright (c) npm, Inc. and Contributors
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
// IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use std::fmt::Write;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::FsOpen;
use sys_traits::FsWrite;

use crate::BinEntriesError;
use crate::bin_entries::EntrySetupOutcome;
use crate::bin_entries::relative_path;

macro_rules! writeln {
  ($($arg:tt)*) => {
    {
      let _ = std::writeln!($($arg)*);
    }
  };
}

// note: parts of logic and pretty much all of the shims ported from https://github.com/npm/cmd-shim
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Shebang {
  pub program: String,
  pub args: String,
  pub vars: String,
}

fn parse_shebang(s: &str) -> Option<Shebang> {
  let s = s.trim();
  if !s.starts_with("#!") {
    return None;
  }
  // lifted from npm/cmd-shimj
  let regex = lazy_regex::regex!(
    r"^#!\s*(?:/usr/bin/env\s+(?:-S\s+)?((?:[^ \t=]+=[^ \t=]+\s+)*))?([^ \t\r\n]+)(.*)$"
  );
  let captures = regex.captures(s)?;
  Some(Shebang {
    vars: captures
      .get(1)
      .map(|m| m.as_str().to_string())
      .unwrap_or_default(),
    program: captures.get(2)?.as_str().to_string(),
    args: captures
      .get(3)
      .map(|m| m.as_str().trim().to_string())
      .unwrap_or_default(),
  })
}

fn resolve_shebang(sys: &impl FsOpen, path: &Path) -> Option<Shebang> {
  let file = sys
    .fs_open(path, sys_traits::OpenOptions::new().read())
    .ok()?;
  let mut reader = std::io::BufReader::new(file);
  let mut line = String::new();
  reader.read_line(&mut line).ok()?;
  parse_shebang(&line)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShimData {
  pub shebang: Option<Shebang>,
  pub target: String,
}

impl ShimData {
  pub fn new(target: impl Into<String>, shebang: Option<Shebang>) -> Self {
    Self {
      shebang,
      target: target.into().replace('\\', "/"),
    }
  }

  fn target_win(&self) -> String {
    self.target.replace('/', "\\")
  }

  pub fn generate_cmd(&self) -> String {
    let mut s = String::with_capacity(512);
    s.push_str(concat!(
      "@ECHO off\r\n",
      "GOTO start\r\n",
      ":find_dp0\r\n",
      "SET dp0=%~dp0\r\n",
      "EXIT /b\r\n",
      ":start\r\n",
      "SETLOCAL\r\n",
      "CALL :find_dp0\r\n",
    ));

    let target_win = self.target_win();
    match &self.shebang {
      None => {
        writeln!(s, "\"%dp0%\\{}\" %*\r", target_win);
      }
      Some(Shebang {
        program,
        args,
        vars,
      }) => {
        let prog = program.replace('\\', "/");
        for var in vars.split_whitespace().filter(|v| v.contains('=')) {
          writeln!(s, "SET {}\r", var);
        }
        let long_prog = format!("%dp0%\\{}.exe", prog);
        writeln!(s, "\r");
        writeln!(s, "IF EXIST \"{}\" (\r", long_prog);
        writeln!(s, "  SET \"_prog={}\"\r", long_prog);
        writeln!(s, ") ELSE (\r");
        writeln!(s, "  SET \"_prog={}\"\r", prog);
        writeln!(s, "  SET PATHEXT=%PATHEXT:;.JS;=;%\r");
        writeln!(s, ")\r");
        writeln!(s, "\r");
        writeln!(
          s,
          "endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\" {} \"%dp0%\\{}\" %*\r",
          args, target_win
        );
      }
    }
    s
  }

  pub fn generate_sh(&self) -> String {
    let mut s = String::with_capacity(512);
    s.push_str(concat!(
      "#!/bin/sh\n",
      "basedir=$(dirname \"$(echo \"$0\" | sed -e 's,\\\\,/,g')\")\n",
      "\n",
      "case `uname` in\n",
      "    *CYGWIN*|*MINGW*|*MSYS*)\n",
      "        if command -v cygpath > /dev/null 2>&1; then\n",
      "            basedir=`cygpath -w \"$basedir\"`\n",
      "        fi\n",
      "    ;;\n",
      "esac\n",
      "\n",
    ));

    let target = format!("\"$basedir/{}\"", self.target);
    match &self.shebang {
      None => {
        writeln!(s, "exec {} \"$@\"", target);
      }
      Some(Shebang {
        program,
        args,
        vars,
      }) => {
        let prog = program.replace('\\', "/");
        let long_prog = format!("\"$basedir/{}\"", prog);
        writeln!(s, "if [ -x {} ]; then", long_prog);
        writeln!(s, "  exec {}{} {} {} \"$@\"", vars, long_prog, args, target);
        writeln!(s, "else");
        writeln!(s, "  exec {}{} {} {} \"$@\"", vars, prog, args, target);
        writeln!(s, "fi");
      }
    }
    s
  }

  pub fn generate_pwsh(&self) -> String {
    let mut s = String::with_capacity(1024);
    s.push_str(concat!(
      "#!/usr/bin/env pwsh\n",
      "$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n",
      "\n",
      "$exe=\"\"\n",
      "if ($PSVersionTable.PSVersion -lt \"6.0\" -or $IsWindows) {\n",
      "  $exe=\".exe\"\n",
      "}\n",
    ));

    let target = format!("\"$basedir/{}\"", self.target);
    match &self.shebang {
      None => {
        Self::write_pwsh_exec(&mut s, &target, "", "");
        writeln!(s, "exit $LASTEXITCODE");
      }
      Some(Shebang { program, args, .. }) => {
        let prog = program.replace('\\', "/");
        let long_prog = format!("\"$basedir/{}$exe\"", prog);
        let short_prog = format!("\"{}$exe\"", prog);
        writeln!(s, "$ret=0");
        writeln!(s, "if (Test-Path {}) {{", long_prog);
        Self::write_pwsh_exec(&mut s, &long_prog, args, &target);
        writeln!(s, "  $ret=$LASTEXITCODE");
        writeln!(s, "}} else {{");
        Self::write_pwsh_exec(&mut s, &short_prog, args, &target);
        writeln!(s, "  $ret=$LASTEXITCODE");
        writeln!(s, "}}");
        writeln!(s, "exit $ret");
      }
    }
    s
  }

  fn write_pwsh_exec(s: &mut String, prog: &str, args: &str, target: &str) {
    writeln!(s, "  if ($MyInvocation.ExpectingInput) {{");
    writeln!(s, "    $input | & {} {} {} $args", prog, args, target);
    writeln!(s, "  }} else {{");
    writeln!(s, "    & {} {} {} $args", prog, args, target);
    writeln!(s, "  }}");
  }
}

pub fn set_up_bin_shim<'a>(
  sys: &(impl FsOpen + FsWrite),
  bin_name: &'a str,
  bin_script: &'a str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &'a Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  let shim_path = bin_node_modules_dir_path.join(bin_name);
  let target_file = package_path.join(bin_script);

  let rel_target =
    relative_path(bin_node_modules_dir_path, &target_file).unwrap();
  let shebang = resolve_shebang(sys, &target_file);
  let shim = ShimData::new(rel_target.to_string_lossy(), shebang);

  let write_shim = |path: PathBuf, contents: &str| {
    sys
      .fs_write(&path, contents)
      .map_err(|err| BinEntriesError::SetUpBin {
        name: bin_name.to_string(),
        path,
        source: Box::new(err.into()),
      })
  };

  write_shim(shim_path.with_extension("cmd"), &shim.generate_cmd())?;
  write_shim(shim_path.clone(), &shim.generate_sh())?;
  write_shim(shim_path.with_extension("ps1"), &shim.generate_pwsh())?;

  Ok(EntrySetupOutcome::Success)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Trims the minimum indent from each line of a multiline string,
  /// removing leading and trailing blank lines.
  fn trim_indent(text: &str) -> String {
    trim_indent_with(text, "\n")
  }

  fn trim_indent_crlf(text: &str) -> String {
    trim_indent_with(text, "\r\n")
  }

  fn trim_indent_with(text: &str, line_ending: &str) -> String {
    let text = text.strip_prefix('\n').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    let min_indent = lines
      .iter()
      .filter(|line| !line.trim().is_empty())
      .map(|line| line.len() - line.trim_start().len())
      .min()
      .unwrap_or(0);

    lines
      .iter()
      .map(|line| {
        if line.len() <= min_indent {
          line.trim_start()
        } else {
          &line[min_indent..]
        }
      })
      .collect::<Vec<_>>()
      .join(line_ending)
  }

  #[test]
  fn test_parse_shebang_node() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env node\n"),
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_with_args() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env node --experimental"),
      Some(Shebang {
        program: "node".into(),
        args: "--experimental".into(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_with_env_vars() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env -S NODE_ENV=production node"),
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: "NODE_ENV=production ".into(), // trailing space is intentional
      })
    );
  }

  #[test]
  fn test_parse_shebang_direct_path() {
    assert_eq!(
      parse_shebang("#!/bin/bash"),
      Some(Shebang {
        program: "/bin/bash".into(),
        args: String::new(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_invalid() {
    assert_eq!(parse_shebang("not a shebang"), None);
    assert_eq!(parse_shebang(""), None);
  }

  #[test]
  fn test_sh_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_sh(),
      trim_indent(
        r#"
        #!/bin/sh
        basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

        case `uname` in
            *CYGWIN*|*MINGW*|*MSYS*)
                if command -v cygpath > /dev/null 2>&1; then
                    basedir=`cygpath -w "$basedir"`
                fi
            ;;
        esac

        exec "$basedir/../pkg/bin/cli.js" "$@"
        "#
      )
    );
  }

  #[test]
  fn test_sh_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_sh(),
      trim_indent(
        r#"
        #!/bin/sh
        basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

        case `uname` in
            *CYGWIN*|*MINGW*|*MSYS*)
                if command -v cygpath > /dev/null 2>&1; then
                    basedir=`cygpath -w "$basedir"`
                fi
            ;;
        esac

        if [ -x "$basedir/node" ]; then
          exec "$basedir/node"  "$basedir/../pkg/bin/cli.js" "$@"
        else
          exec node  "$basedir/../pkg/bin/cli.js" "$@"
        fi
        "#
      )
    );
  }

  #[test]
  fn test_cmd_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_cmd(),
      trim_indent_crlf(
        r#"
        @ECHO off
        GOTO start
        :find_dp0
        SET dp0=%~dp0
        EXIT /b
        :start
        SETLOCAL
        CALL :find_dp0
        "%dp0%\..\pkg\bin\cli.js" %*
        "#
      )
    );
  }

  #[test]
  fn test_cmd_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_cmd(),
      trim_indent_crlf(
        r#"
        @ECHO off
        GOTO start
        :find_dp0
        SET dp0=%~dp0
        EXIT /b
        :start
        SETLOCAL
        CALL :find_dp0

        IF EXIST "%dp0%\node.exe" (
          SET "_prog=%dp0%\node.exe"
        ) ELSE (
          SET "_prog=node"
          SET PATHEXT=%PATHEXT:;.JS;=;%
        )

        endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\..\pkg\bin\cli.js" %*
        "#
      )
    );
  }

  #[test]
  fn test_pwsh_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_pwsh(),
      trim_indent(
        r#"
        #!/usr/bin/env pwsh
        $basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent

        $exe=""
        if ($PSVersionTable.PSVersion -lt "6.0" -or $IsWindows) {
          $exe=".exe"
        }
          if ($MyInvocation.ExpectingInput) {
            $input | & "$basedir/../pkg/bin/cli.js"   $args
          } else {
            & "$basedir/../pkg/bin/cli.js"   $args
          }
        exit $LASTEXITCODE
        "#
      )
    );
  }

  #[test]
  fn test_pwsh_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_pwsh(),
      trim_indent(
        r#"
        #!/usr/bin/env pwsh
        $basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent

        $exe=""
        if ($PSVersionTable.PSVersion -lt "6.0" -or $IsWindows) {
          $exe=".exe"
        }
        $ret=0
        if (Test-Path "$basedir/node$exe") {
          if ($MyInvocation.ExpectingInput) {
            $input | & "$basedir/node$exe"  "$basedir/../pkg/bin/cli.js" $args
          } else {
            & "$basedir/node$exe"  "$basedir/../pkg/bin/cli.js" $args
          }
          $ret=$LASTEXITCODE
        } else {
          if ($MyInvocation.ExpectingInput) {
            $input | & "node$exe"  "$basedir/../pkg/bin/cli.js" $args
          } else {
            & "node$exe"  "$basedir/../pkg/bin/cli.js" $args
          }
          $ret=$LASTEXITCODE
        }
        exit $ret
        "#
      )
    );
  }

  #[test]
  fn test_shim_with_args_and_vars() {
    let shim = ShimData::new(
      "bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: "--experimental-modules".into(),
        vars: "NODE_ENV=prod ".into(), // trailing space is intentional
      }),
    );

    let sh = shim.generate_sh();
    assert!(
      sh.contains("NODE_ENV=prod \"$basedir/node\" --experimental-modules")
    );
    assert!(sh.contains("NODE_ENV=prod node --experimental-modules"));

    let cmd = shim.generate_cmd();
    assert!(cmd.contains("SET NODE_ENV=prod\r\n"));
    assert!(cmd.contains("\"%_prog%\" --experimental-modules"));

    let pwsh = shim.generate_pwsh();
    assert!(pwsh.contains("\"$basedir/node$exe\" --experimental-modules"));
  }
}
