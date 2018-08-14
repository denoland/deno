use std;
use std::sync::mpsc::channel;

use hyper::{Client, Uri};
use hyper::rt::{self, Future, Stream};

pub fn fetch_http_code_sync(url: Uri) -> std::io::Result<String> {
  let (sender, receiver) = channel();
  let client = Client::new();
    
  rt::run(client
    .get(url)
    .and_then(move |res| {
      sender.send(res).unwrap();
      Ok(())
    }).map_err(|err| {
      println!("error: {}", err);
    })
  );
    
  let result = receiver.recv().unwrap();
  let body = result.into_body().concat2().wait().unwrap();
  
  return Ok(::std::str::from_utf8(&body).unwrap().to_string());
}