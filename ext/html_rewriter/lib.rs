// Copyright 2018-2026 the Deno authors. MIT license.

mod rewriter;
mod tokens;

pub use rewriter::HtmlRewriterError;

deno_core::extension!(
  deno_html_rewriter,
  deps = [deno_webidl, deno_web, deno_fetch],
  ops = [
    rewriter::op_html_rewriter_parse_selector,
    rewriter::op_html_rewriter_start,
    rewriter::op_html_rewriter_write,
    rewriter::op_html_rewriter_end,
    rewriter::op_html_rewriter_pump,
    rewriter::op_html_rewriter_pump_sync,
    rewriter::op_html_rewriter_token_done,
    rewriter::op_html_rewriter_token_error,
    rewriter::op_html_rewriter_abort,
    tokens::op_html_rewriter_element_tag_name,
    tokens::op_html_rewriter_element_set_tag_name,
    tokens::op_html_rewriter_element_namespace_uri,
    tokens::op_html_rewriter_element_attributes,
    tokens::op_html_rewriter_element_get_attribute,
    tokens::op_html_rewriter_element_has_attribute,
    tokens::op_html_rewriter_element_set_attribute,
    tokens::op_html_rewriter_element_remove_attribute,
    tokens::op_html_rewriter_element_on_end_tag,
    tokens::op_html_rewriter_token_content,
    tokens::op_html_rewriter_token_remove,
    tokens::op_html_rewriter_token_removed,
    tokens::op_html_rewriter_text_info,
    tokens::op_html_rewriter_comment_text,
    tokens::op_html_rewriter_set_comment_text,
    tokens::op_html_rewriter_doctype_info,
    tokens::op_html_rewriter_end_tag_name,
    tokens::op_html_rewriter_set_end_tag_name,
  ],
  lazy_loaded_js = ["01_html_rewriter.js"],
);
