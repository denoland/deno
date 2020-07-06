use crate::colors;
use crate::ops::fs_events::create_resource;
use crate::state::State;
use deno_core::ErrBox;
use futures::future::poll_fn;
use futures::Future;
use notify::RecursiveMode;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::select;

type WatchFuture =
  Pin<Box<dyn Future<Output = std::result::Result<(), deno_core::ErrBox>>>>;

pub async fn watch_func<F>(
  watch_paths: &[PathBuf],
  closure: F,
) -> Result<(), ErrBox>
where
  F: Fn() -> WatchFuture,
{
  async fn error_handling(func: WatchFuture) {
    let result = func.await;
    if let Err(err) = result {
      let msg = format!("{}: {}", colors::red_bold("error"), err.to_string(),);
      eprintln!("{}", msg);
    }
  }

  if watch_paths.is_empty() {
    let func = closure();
    func.await?;
  } else {
    loop {
      let func = error_handling(closure());
      let mut is_file_changed = false;
      select! {
        _ = file_watcher(watch_paths) => {
            is_file_changed = true;
            println!("File change detected! Restarting!");
          },
        _ = func => { },
      };
      if !is_file_changed {
        println!("Process terminated! Restarting on file change...");
        file_watcher(watch_paths).await?;
        println!("File change detected! Restarting!");
      }
    }
  }
  Ok(())
}

pub async fn file_watcher(
  paths: &[PathBuf],
) -> Result<serde_json::Value, deno_core::ErrBox> {
  loop {
    let mut resource =
      create_resource(paths, RecursiveMode::Recursive, None::<&State>)?;
    let f = poll_fn(move |cx| {
      resource
        .receiver
        .poll_recv(cx)
        .map(|maybe_result| match maybe_result {
          Some(Ok(value)) => {
            println!("{:?}", value);
            Ok(json!({ "value": value, "done": false }))
          }
          Some(Err(err)) => Err(err),
          None => Ok(json!({ "done": true })),
        })
    });
    let res = f.await?;
    if res["value"].is_object() && res["value"]["kind"].is_string() {
      let kind = &res["value"]["kind"];
      if kind == "create" || kind == "modify" || kind == "remove" {
        return Ok(res);
      }
    }
  }
}
