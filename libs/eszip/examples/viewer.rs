// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use futures::io::AllowStdIo;
use futures::io::BufReader;
use std::env;
use std::fs;
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let args: Vec<String> = env::args().collect();
  let (eszip_path, output_dir) = match parse_args(args) {
    Ok(result) => result,
    Err(err) => {
      eprintln!("Error: {}", err);
      print_help();
      return;
    }
  };

  let file = std::fs::File::open(&eszip_path).unwrap();
  let bufreader = BufReader::new(AllowStdIo::new(file));
  let (eszip, loader) = eszip::EszipV2::parse(bufreader).await.unwrap();

  let fut = async move {
    for (specifier, module) in eszip {
      if module.specifier == specifier {
        // skip extracting data specifiers.
        if specifier.starts_with("data:") {
          continue;
        }
        let source = module.source().await.expect("source already taken");
        let source = std::str::from_utf8(&source).unwrap();

        if let Some(ref output_dir) = output_dir {
          let specifier = specifier
            .trim_start_matches("file:///")
            .trim_start_matches("http://")
            .trim_start_matches("https://");
          let file_path = output_dir.join(
            PathBuf::from(&specifier)
              .strip_prefix("/")
              .unwrap_or(&PathBuf::from(&specifier)),
          );
          if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create directory");
          }
          fs::write(&file_path, source).expect("Failed to write file");
          println!("Extracted {}", file_path.display());
        } else {
          println!("Specifier: {specifier}",);
          println!("Kind: {kind:?}", kind = module.kind);
          println!("---");
          println!("{source}");

          let source_map = module.source_map().await;
          if let Some(source_map) = source_map {
            let source_map = std::str::from_utf8(&source_map).unwrap();
            println!("---");
            println!("{source_map}");
          }

          println!("============");
        }
      }
    }

    Ok(())
  };

  tokio::try_join!(loader, fut).unwrap();
}

fn print_help() {
  println!("Usage:");
  println!("  viewer <eszip_path>");
  println!("  viewer --output <output_dir> <eszip_path>");
  println!("  viewer -o <output_dir> <eszip_path>");
}

fn parse_args(args: Vec<String>) -> Result<(PathBuf, Option<PathBuf>), String> {
  let mut output_dir = None;
  let mut eszip_path = None;
  let mut args_iter = args.into_iter().skip(1);
  while let Some(arg) = args_iter.next() {
    match arg.as_str() {
      "--output" | "-o" => {
        output_dir = Some(PathBuf::from(
          args_iter.next().ok_or("Missing output directory")?,
        ));
      }
      _ if eszip_path.is_none() => {
        eszip_path = Some(PathBuf::from(arg));
      }
      _ => return Err(format!("Unknown argument: {}", arg)),
    }
  }
  let eszip_path = eszip_path.ok_or("Missing eszip path")?;
  Ok((eszip_path, output_dir))
}
