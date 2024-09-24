// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// don't use any non-std dependencies here so that this compiles really fast

use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::io::Write;
use std::process::Command;

pub fn main() {
  append_to_hosts("deno-local.test", "127.0.0.1");
}

fn append_to_hosts(host: &str, ip: &str) {
  let hosts_file = if cfg!(windows) {
    std::env::var("SystemRoot").unwrap() + r"\System32\drivers\etc\hosts"
  } else {
    "/etc/hosts".to_string()
  };

  let hosts_file_has_host =
    || std::fs::read_to_string(&hosts_file).unwrap().contains(host);
  if hosts_file_has_host() {
    return;
  }

  if std::io::stderr().is_terminal() && std::env::var("CI").is_err() {
    eprintln!(
      concat!(
        "\n\nNOTICE: Running Deno's test suite requires adding the entry '{} {}' to '{}'. ",
        "Are you ok with this?\n\nHit enter to confirm, or ctrl+c to exit.",
      ),
      ip,
      host,
      hosts_file
    );
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    // check again just in case something added to it in the meantime
    if hosts_file_has_host() {
      return;
    }
  }

  if cfg!(windows) {
    // we need admin rights, so launch a ps process that then launches an elevated ps process
    let ps_command = format!("Add-Content -Path $env:SystemRoot\\System32\\drivers\\etc\\hosts -Value \"`n{}`t{}\"", ip, host);
    let status = Command::new("powershell")
      .arg("-Command")
      .arg(format!(
        "Start-Process powershell -WindowStyle Hidden -Verb runAs -ArgumentList '-Command \"{}\"' -Wait; exit $LASTEXITCODE",
        ps_command.replace("\"", "\"\"")
      ))
      .status()
      .expect("Failed to execute PowerShell command");

    if !status.success() {
      panic!("Failed to add entry to hosts file. Please do so manually.");
    }
  } else {
    let mut file = OpenOptions::new().append(true).open(&hosts_file).unwrap();
    file
      .write_all(format!("{} {}\n", ip, host).as_bytes())
      .unwrap();
  }
}
