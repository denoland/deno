// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::unsync::JoinHandle;

pub struct BackgroundProcessor<TData> {
  sender: tokio::sync::mpsc::UnboundedSender<TData>,
  task: JoinHandle<()>,
}

impl<TData: Send + Sync + 'static> BackgroundProcessor<TData> {
  pub fn new(
    process_func: impl Fn(TData) -> () + Send + Sync + Clone + 'static,
  ) -> Self {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let task = deno_core::unsync::spawn(async move {
      while let Some(data) = receiver.recv().await {
        let process_func = process_func.clone();
        deno_core::unsync::spawn_blocking(move || {
          process_func(data);
        })
        .await
        .unwrap()
      }
    });

    Self { sender, task }
  }

  pub async fn shutdown(self) {
    drop(self.sender);
    self.task.await.unwrap();
  }

  pub fn send(&self, data: TData) {
    self.sender.send(data).unwrap();
  }
}
