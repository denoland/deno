use std;
use std::sync::mpsc::channel;

use hyper::{Client, Uri};
use hyper::rt::{self, Future, Stream};

/**
 * The CodeFetch message is used to load HTTP javascript resources and expects a synchronous response,
 * this utility method supports that.
*/
pub fn http_code_fetch(module_name: &str) -> std::io::Result<String> {
  let url = module_name.parse::<Uri>().unwrap();
  let (sender, receiver) = channel();
  let client = Client::new();

  println!("Downloading {}", url);

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