// Copyright 2018-2026 the Deno authors. MIT license.

//! Content token handles and their mutation ops.
//!
//! # Safety
//!
//! `TokenPtr` stores a lifetime-erased pointer to a `lol_html` content token
//! (`Element`, `TextChunk`, ...) that lives on the stack of the parked
//! rewriter thread, inside the content handler closure that dispatched it.
//! Dereferencing it is sound if and only if:
//!
//! 1. The rewriter thread is parked in `ThreadCtx::dispatch` waiting for the
//!    response to exactly this dispatch. The thread only unparks when
//!    `HtmlRewriterTransform::finish_token` sends the response, which also
//!    removes the `TokenPtr` from `current_token`, so the pointer can never
//!    be dereferenced after the thread resumes.
//! 2. Only one dispatch is outstanding at a time (guaranteed by the strictly
//!    sequential dispatch/park protocol), so the main thread has exclusive
//!    access to the token while it holds the pointer.
//!
//! Every op below goes through `with_current_token`, which additionally
//! checks the token generation so stale JS wrappers get a `TypeError`
//! instead of reaching a dangling pointer.

use std::ptr::NonNull;

use deno_core::op2;
use lol_html::html_content::Comment;
use lol_html::html_content::ContentType;
use lol_html::html_content::Doctype;
use lol_html::html_content::DocumentEnd;
use lol_html::html_content::EndTag;
use lol_html::html_content::TextChunk;
use lol_html::send::Element;

use crate::rewriter::HtmlRewriterError;
use crate::rewriter::HtmlRewriterTransform;
use crate::rewriter::TokenKind;

pub(crate) enum TokenPtr {
  Element(NonNull<Element<'static, 'static>>),
  Text(NonNull<TextChunk<'static>>),
  Comment(NonNull<Comment<'static>>),
  Doctype(NonNull<Doctype<'static>>),
  DocumentEnd(NonNull<DocumentEnd<'static>>),
  EndTag(NonNull<EndTag<'static>>),
}

// SAFETY: the pointee lives on the parked rewriter thread and is only
// accessed from the main thread while the rewriter thread is parked; see the
// module documentation.
unsafe impl Send for TokenPtr {}

impl TokenPtr {
  pub(crate) fn element(el: &mut Element<'_, '_>) -> Self {
    Self::Element(NonNull::from(el).cast())
  }

  pub(crate) fn text(text: &mut TextChunk<'_>) -> Self {
    Self::Text(NonNull::from(text).cast())
  }

  pub(crate) fn comment(comment: &mut Comment<'_>) -> Self {
    Self::Comment(NonNull::from(comment).cast())
  }

  pub(crate) fn doctype(doctype: &mut Doctype<'_>) -> Self {
    Self::Doctype(NonNull::from(doctype).cast())
  }

  pub(crate) fn document_end(end: &mut DocumentEnd<'_>) -> Self {
    Self::DocumentEnd(NonNull::from(end).cast())
  }

  pub(crate) fn end_tag(end_tag: &mut EndTag<'_>) -> Self {
    Self::EndTag(NonNull::from(end_tag).cast())
  }
}

fn with_current_token<T>(
  transform: &HtmlRewriterTransform,
  generation: u32,
  f: impl FnOnce(&mut TokenPtr) -> Result<T, HtmlRewriterError>,
) -> Result<T, HtmlRewriterError> {
  let mut current = transform.current_token.borrow_mut();
  match current.as_mut() {
    Some(current) if current.generation == generation => f(&mut current.token),
    _ => Err(HtmlRewriterError::StaleToken),
  }
}

fn with_element<T>(
  transform: &HtmlRewriterTransform,
  generation: u32,
  f: impl FnOnce(&mut Element<'_, '_>) -> Result<T, HtmlRewriterError>,
) -> Result<T, HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::Element(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    f(unsafe { ptr.as_mut() })
  })
}

#[op2]
#[string]
pub fn op_html_rewriter_element_tag_name(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<String, HtmlRewriterError> {
  with_element(transform, generation, |el| Ok(el.tag_name()))
}

#[op2(fast)]
pub fn op_html_rewriter_element_set_tag_name(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
) -> Result<(), HtmlRewriterError> {
  with_element(transform, generation, |el| {
    el.set_tag_name(name)
      .map_err(|err| HtmlRewriterError::Mutation(err.to_string()))
  })
}

#[op2]
#[string]
pub fn op_html_rewriter_element_namespace_uri(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<String, HtmlRewriterError> {
  with_element(transform, generation, |el| {
    Ok(el.namespace_uri().to_string())
  })
}

#[op2]
#[serde]
pub fn op_html_rewriter_element_attributes(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<Vec<(String, String)>, HtmlRewriterError> {
  with_element(transform, generation, |el| {
    Ok(
      el.attributes()
        .iter()
        .map(|attr| (attr.name(), attr.value()))
        .collect(),
    )
  })
}

#[op2]
#[string]
pub fn op_html_rewriter_element_get_attribute(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
) -> Result<Option<String>, HtmlRewriterError> {
  with_element(transform, generation, |el| Ok(el.get_attribute(name)))
}

#[op2(fast)]
pub fn op_html_rewriter_element_has_attribute(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
) -> Result<bool, HtmlRewriterError> {
  with_element(transform, generation, |el| Ok(el.has_attribute(name)))
}

#[op2(fast)]
pub fn op_html_rewriter_element_set_attribute(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
  #[string] value: &str,
) -> Result<(), HtmlRewriterError> {
  with_element(transform, generation, |el| {
    el.set_attribute(name, value)
      .map_err(|err| HtmlRewriterError::Mutation(err.to_string()))
  })
}

#[op2(fast)]
pub fn op_html_rewriter_element_remove_attribute(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
) -> Result<(), HtmlRewriterError> {
  with_element(transform, generation, |el| {
    el.remove_attribute(name);
    Ok(())
  })
}

#[op2(fast)]
pub fn op_html_rewriter_element_on_end_tag(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  handler_id: u32,
) -> Result<(), HtmlRewriterError> {
  let ctx = transform.ctx.clone();
  with_element(transform, generation, |el| {
    let handlers = el.end_tag_handlers().ok_or_else(|| {
      HtmlRewriterError::Mutation(
        "Element does not have an end tag".to_string(),
      )
    })?;
    handlers.push(Box::new(move |end_tag: &mut EndTag<'_>| {
      ctx.dispatch(handler_id, TokenKind::EndTag, TokenPtr::end_tag(end_tag))
    }));
    Ok(())
  })
}

const CONTENT_BEFORE: u32 = 0;
const CONTENT_AFTER: u32 = 1;
const CONTENT_PREPEND: u32 = 2;
const CONTENT_APPEND: u32 = 3;
const CONTENT_REPLACE: u32 = 4;
const CONTENT_SET_INNER_CONTENT: u32 = 5;

#[op2(fast)]
pub fn op_html_rewriter_token_content(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  method: u32,
  #[string] content: &str,
  html: bool,
) -> Result<(), HtmlRewriterError> {
  let content_type = if html {
    ContentType::Html
  } else {
    ContentType::Text
  };
  with_current_token(transform, generation, |token| {
    match token {
      TokenPtr::Element(ptr) => {
        // SAFETY: see the module documentation.
        let el = unsafe { ptr.as_mut() };
        match method {
          CONTENT_BEFORE => el.before(content, content_type),
          CONTENT_AFTER => el.after(content, content_type),
          CONTENT_PREPEND => el.prepend(content, content_type),
          CONTENT_APPEND => el.append(content, content_type),
          CONTENT_REPLACE => el.replace(content, content_type),
          CONTENT_SET_INNER_CONTENT => {
            el.set_inner_content(content, content_type)
          }
          _ => return Err(HtmlRewriterError::InvalidTokenOperation),
        }
      }
      TokenPtr::Text(ptr) => {
        // SAFETY: see the module documentation.
        let text = unsafe { ptr.as_mut() };
        match method {
          CONTENT_BEFORE => text.before(content, content_type),
          CONTENT_AFTER => text.after(content, content_type),
          CONTENT_REPLACE => text.replace(content, content_type),
          _ => return Err(HtmlRewriterError::InvalidTokenOperation),
        }
      }
      TokenPtr::Comment(ptr) => {
        // SAFETY: see the module documentation.
        let comment = unsafe { ptr.as_mut() };
        match method {
          CONTENT_BEFORE => comment.before(content, content_type),
          CONTENT_AFTER => comment.after(content, content_type),
          CONTENT_REPLACE => comment.replace(content, content_type),
          _ => return Err(HtmlRewriterError::InvalidTokenOperation),
        }
      }
      TokenPtr::DocumentEnd(ptr) => {
        // SAFETY: see the module documentation.
        let end = unsafe { ptr.as_mut() };
        match method {
          CONTENT_APPEND => end.append(content, content_type),
          _ => return Err(HtmlRewriterError::InvalidTokenOperation),
        }
      }
      TokenPtr::EndTag(ptr) => {
        // SAFETY: see the module documentation.
        let end_tag = unsafe { ptr.as_mut() };
        match method {
          CONTENT_BEFORE => end_tag.before(content, content_type),
          CONTENT_AFTER => end_tag.after(content, content_type),
          CONTENT_REPLACE => end_tag.replace(content, content_type),
          _ => return Err(HtmlRewriterError::InvalidTokenOperation),
        }
      }
      TokenPtr::Doctype(_) => {
        return Err(HtmlRewriterError::InvalidTokenOperation);
      }
    }
    Ok(())
  })
}

#[op2(fast)]
pub fn op_html_rewriter_token_remove(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  keep_content: bool,
) -> Result<(), HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    match token {
      TokenPtr::Element(ptr) => {
        // SAFETY: see the module documentation.
        let el = unsafe { ptr.as_mut() };
        if keep_content {
          el.remove_and_keep_content();
        } else {
          el.remove();
        }
      }
      // SAFETY: see the module documentation.
      TokenPtr::Text(ptr) => unsafe { ptr.as_mut() }.remove(),
      // SAFETY: see the module documentation.
      TokenPtr::Comment(ptr) => unsafe { ptr.as_mut() }.remove(),
      // SAFETY: see the module documentation.
      TokenPtr::Doctype(ptr) => unsafe { ptr.as_mut() }.remove(),
      // SAFETY: see the module documentation.
      TokenPtr::EndTag(ptr) => unsafe { ptr.as_mut() }.remove(),
      TokenPtr::DocumentEnd(_) => {
        return Err(HtmlRewriterError::InvalidTokenOperation);
      }
    }
    Ok(())
  })
}

#[op2(fast)]
pub fn op_html_rewriter_token_removed(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<bool, HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let removed = match token {
      // SAFETY: see the module documentation.
      TokenPtr::Element(ptr) => unsafe { ptr.as_ref() }.removed(),
      // SAFETY: see the module documentation.
      TokenPtr::Text(ptr) => unsafe { ptr.as_ref() }.removed(),
      // SAFETY: see the module documentation.
      TokenPtr::Comment(ptr) => unsafe { ptr.as_ref() }.removed(),
      // SAFETY: see the module documentation.
      TokenPtr::Doctype(ptr) => unsafe { ptr.as_ref() }.removed(),
      // SAFETY: see the module documentation.
      TokenPtr::EndTag(ptr) => unsafe { ptr.as_ref() }.removed(),
      TokenPtr::DocumentEnd(_) => {
        return Err(HtmlRewriterError::InvalidTokenOperation);
      }
    };
    Ok(removed)
  })
}

#[op2]
#[serde]
pub fn op_html_rewriter_text_info(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<(String, bool), HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::Text(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    let text = unsafe { ptr.as_ref() };
    Ok((text.as_str().to_string(), text.last_in_text_node()))
  })
}

#[op2]
#[string]
pub fn op_html_rewriter_comment_text(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<String, HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::Comment(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    Ok(unsafe { ptr.as_ref() }.text())
  })
}

#[op2(fast)]
pub fn op_html_rewriter_set_comment_text(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] text: &str,
) -> Result<(), HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::Comment(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    unsafe { ptr.as_mut() }
      .set_text(text)
      .map_err(|err| HtmlRewriterError::Mutation(err.to_string()))
  })
}

type DoctypeInfo = (Option<String>, Option<String>, Option<String>);

#[op2]
#[serde]
pub fn op_html_rewriter_doctype_info(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<DoctypeInfo, HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::Doctype(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    let doctype = unsafe { ptr.as_ref() };
    Ok((doctype.name(), doctype.public_id(), doctype.system_id()))
  })
}

#[op2]
#[string]
pub fn op_html_rewriter_end_tag_name(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
) -> Result<String, HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::EndTag(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    Ok(unsafe { ptr.as_ref() }.name())
  })
}

#[op2(fast)]
pub fn op_html_rewriter_set_end_tag_name(
  #[cppgc] transform: &HtmlRewriterTransform,
  generation: u32,
  #[string] name: &str,
) -> Result<(), HtmlRewriterError> {
  with_current_token(transform, generation, |token| {
    let TokenPtr::EndTag(ptr) = token else {
      return Err(HtmlRewriterError::InvalidTokenOperation);
    };
    // SAFETY: see the module documentation.
    unsafe { ptr.as_mut() }.set_name_str(name.to_string());
    Ok(())
  })
}
