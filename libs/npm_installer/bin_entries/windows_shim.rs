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

use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;

use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsWrite;

use crate::BinEntriesError;
use crate::bin_entries::EntrySetupOutcome;
use crate::bin_entries::relative_path;

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
  // lifted from npm/cmd-shim
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

  // these are all kinda hard to read, look at the unit tests to see what they generate
  pub fn generate_cmd(&self) -> String {
    let target_win = self.target_win();
    let shebang_data = self.shebang.as_ref().map(
      |Shebang {
         program,
         args,
         vars,
       }| (program.replace('\\', "/"), args.as_str(), vars.as_str()),
    );

    capacity_builder::StringBuilder::build(|builder| {
      builder.append("@ECHO off\r\n");
      builder.append("GOTO start\r\n");
      builder.append(":find_dp0\r\n");
      builder.append("SET dp0=%~dp0\r\n");
      builder.append("EXIT /b\r\n");
      builder.append(":start\r\n");
      builder.append("SETLOCAL\r\n");
      builder.append("CALL :find_dp0\r\n");

      match &shebang_data {
        None => {
          builder.append("\"%dp0%\\");
          builder.append(&target_win);
          builder.append("\" %*\r\n");
        }
        Some((prog, args, vars)) => {
          for var in vars.split_whitespace().filter(|v| v.contains('=')) {
            builder.append("SET ");
            builder.append(var);
            builder.append("\r\n");
          }
          builder.append("\r\n");
          builder.append("IF EXIST \"%dp0%\\");
          builder.append(prog);
          builder.append(".exe\" (\r\n");
          builder.append("  SET \"_prog=%dp0%\\");
          builder.append(prog);
          builder.append(".exe\"\r\n");
          builder.append(") ELSE (\r\n");
          builder.append("  SET \"_prog=");
          builder.append(prog);
          builder.append("\"\r\n");
          builder.append("  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n");
          builder.append(")\r\n");
          builder.append("\r\n");
          builder.append("endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\" ");
          builder.append(*args);
          builder.append(" \"%dp0%\\");
          builder.append(&target_win);
          builder.append("\" %*\r\n");
        }
      }
    })
    .unwrap()
  }

  // these are all kinda hard to read, look at the unit tests to see what they generate
  pub fn generate_sh(&self) -> String {
    let shebang_data = self.shebang.as_ref().map(
      |Shebang {
         program,
         args,
         vars,
       }| (program.replace('\\', "/"), args.as_str(), vars.as_str()),
    );

    capacity_builder::StringBuilder::build(|builder| {
      builder.append("#!/bin/sh\n");
      builder.append(
        "basedir=$(dirname \"$(echo \"$0\" | sed -e 's,\\\\,/,g')\")\n",
      );
      builder.append("\n");
      builder.append("case `uname` in\n");
      builder.append("    *CYGWIN*|*MINGW*|*MSYS*)\n");
      builder.append("        if command -v cygpath > /dev/null 2>&1; then\n");
      builder.append("            basedir=`cygpath -w \"$basedir\"`\n");
      builder.append("        fi\n");
      builder.append("    ;;\n");
      builder.append("esac\n");
      builder.append("\n");

      match &shebang_data {
        None => {
          builder.append("exec \"$basedir/");
          builder.append(&self.target);
          builder.append("\" \"$@\"\n");
        }
        Some((prog, args, vars)) => {
          builder.append("if [ -x \"$basedir/");
          builder.append(prog);
          builder.append("\" ]; then\n");
          builder.append("  exec ");
          builder.append(*vars);
          builder.append("\"$basedir/");
          builder.append(prog);
          builder.append("\" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" \"$@\"\n");
          builder.append("else\n");
          builder.append("  exec ");
          builder.append(*vars);
          builder.append(prog);
          builder.append(" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" \"$@\"\n");
          builder.append("fi\n");
        }
      }
    })
    .unwrap()
  }

  // these are all kinda hard to read, look at the unit tests to see what they generate
  pub fn generate_pwsh(&self) -> String {
    let shebang_data = self.shebang.as_ref().map(
      |Shebang {
         program,
         args,
         vars: _,
       }| (program.replace('\\', "/"), args.as_str()),
    );

    capacity_builder::StringBuilder::build(|builder| {
      builder.append("#!/usr/bin/env pwsh\n");
      builder.append(
        "$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n",
      );
      builder.append("\n");
      builder.append("$exe=\"\"\n");
      builder.append(
        "if ($PSVersionTable.PSVersion -lt \"6.0\" -or $IsWindows) {\n",
      );
      builder.append("  $exe=\".exe\"\n");
      builder.append("}\n");

      match &shebang_data {
        None => {
          builder.append("  if ($MyInvocation.ExpectingInput) {\n");
          builder.append("    $input | & \"$basedir/");
          builder.append(&self.target);
          builder.append("\"   $args\n");
          builder.append("  } else {\n");
          builder.append("    & \"$basedir/");
          builder.append(&self.target);
          builder.append("\"   $args\n");
          builder.append("  }\n");
          builder.append("exit $LASTEXITCODE\n");
        }
        Some((prog, args)) => {
          builder.append("$ret=0\n");
          builder.append("if (Test-Path \"$basedir/");
          builder.append(prog);
          builder.append("$exe\") {\n");
          builder.append("  if ($MyInvocation.ExpectingInput) {\n");
          builder.append("    $input | & \"$basedir/");
          builder.append(prog);
          builder.append("$exe\" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" $args\n");
          builder.append("  } else {\n");
          builder.append("    & \"$basedir/");
          builder.append(prog);
          builder.append("$exe\" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" $args\n");
          builder.append("  }\n");
          builder.append("  $ret=$LASTEXITCODE\n");
          builder.append("} else {\n");
          builder.append("  if ($MyInvocation.ExpectingInput) {\n");
          builder.append("    $input | & \"");
          builder.append(prog);
          builder.append("$exe\" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" $args\n");
          builder.append("  } else {\n");
          builder.append("    & \"");
          builder.append(prog);
          builder.append("$exe\" ");
          builder.append(*args);
          builder.append(" \"$basedir/");
          builder.append(&self.target);
          builder.append("\" $args\n");
          builder.append("  }\n");
          builder.append("  $ret=$LASTEXITCODE\n");
          builder.append("}\n");
          builder.append("exit $ret\n");
        }
      }
    })
    .unwrap()
  }
}

pub fn set_up_bin_shim<'a>(
  sys: &(impl FsOpen + FsWrite + FsMetadata),
  package: &'a deno_npm::NpmResolutionPackage,
  extra: &'a deno_npm::NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &'a str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &'a Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  let shim_path = bin_node_modules_dir_path.join(bin_name);
  let target_file = package_path.join(bin_script);

  let target_file = if !sys.fs_exists_no_err(&target_file) {
    let target_file_exe = target_file.with_extension("exe");
    if !sys.fs_exists_no_err(&target_file_exe) {
      return Ok(EntrySetupOutcome::MissingEntrypoint {
        bin_name,
        package_path,
        entrypoint: target_file,
        package,
        extra,
      });
    }
    target_file_exe
  } else {
    target_file
  };

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
