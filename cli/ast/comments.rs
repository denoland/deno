// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::rc::Rc;
use swc_common::BytePos;
use swc_common::comments::Comment;
use swc_common::comments::Comments;
use swc_common::comments::SingleThreadedComments;
use swc_common::comments::SingleThreadedCommentsMapInner;
use std::cell::RefCell;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct MultiThreadedComments {
  leading: Arc<SingleThreadedCommentsMapInner>,
  trailing: Arc<SingleThreadedCommentsMapInner>,
}

impl MultiThreadedComments {
  pub fn from_single_threaded(comments: SingleThreadedComments) -> Self {
    let (leading, trailing) = comments.take_all();
    let leading = Arc::new(Rc::try_unwrap(leading).unwrap().into_inner());
    let trailing = Arc::new(Rc::try_unwrap(trailing).unwrap().into_inner());
    MultiThreadedComments { leading, trailing }
  }

  pub fn as_single_threaded(&self) -> SingleThreadedComments {
    let leading = Rc::new(RefCell::new((*self.leading).to_owned()));
    let trailing = Rc::new(RefCell::new((*self.trailing).to_owned()));
    SingleThreadedComments::from_leading_and_trailing(leading, trailing)
  }

  pub fn get_vec(&self) -> Vec<Comment> {
    let mut comments = Vec::new();

    for value in self.leading.values() {
      comments.extend(value.clone());
    }

    for value in self.trailing.values() {
      comments.extend(value.clone());
    }

    comments
  }
}

impl Comments for MultiThreadedComments {
  fn add_leading(&self, _pos: BytePos, _cmt: Comment) {
    panic_readonly();
  }

  fn add_leading_comments(
    &self,
    _pos: BytePos,
    _comments: Vec<Comment>,
  ) {
    panic_readonly();
  }

  fn has_leading(&self, pos: BytePos) -> bool {
    self.leading.contains_key(&pos)
  }

  fn move_leading(&self, _from: BytePos, _to: BytePos) {
    panic_readonly();
  }

  fn take_leading(&self, _pos: BytePos) -> Option<Vec<Comment>> {
    panic_readonly();
  }

  fn get_leading(&self, pos: BytePos) -> Option<Vec<Comment>> {
    self.leading.get(&pos).map(|c| c.clone())
  }

  fn add_trailing(&self, _pos: BytePos, _cmt: Comment) {
    panic_readonly();
  }

  fn add_trailing_comments(
    &self,
    _pos: BytePos,
    _comments: Vec<Comment>,
  ) {
    panic_readonly();
  }

  fn has_trailing(&self, pos: BytePos) -> bool {
    self.trailing.contains_key(&pos)
  }

  fn move_trailing(&self, _from: BytePos, _to: BytePos) {
    panic_readonly();
  }

  fn take_trailing(&self, _pos: BytePos) -> Option<Vec<Comment>> {
    panic_readonly();
  }

  fn get_trailing(&self, pos: BytePos) -> Option<Vec<Comment>> {
    self.trailing.get(&pos).map(|c| c.clone())
  }

  fn add_pure_comment(&self, _pos: BytePos) {
    panic_readonly();
  }
}

fn panic_readonly() -> ! {
  panic!("MultiThreadedComments do not support write operations")
}
