// Copyright 2018-2026 the Deno authors. MIT license.

use deno_runtime::deno_permissions::prompter::GetFormattedStackFn;
use deno_runtime::deno_permissions::prompter::PermissionPrompter;
use deno_runtime::deno_permissions::prompter::PromptResponse;
use tokio::sync::mpsc;

use crate::ops::jupyter::PendingInputRequest;
use crate::ops::jupyter::request_input_blocking;

/// Routes Deno permission prompts to the notebook frontend over Jupyter's
/// `stdin` channel instead of the controlling terminal (which a kernel does not
/// have). Without this, a notebook running under a restrictive
/// `permissions.jupyter` set would have every denied access fail immediately,
/// with no way for the user to grant it interactively.
pub struct JupyterPrompter {
  input_tx: mpsc::UnboundedSender<PendingInputRequest>,
}

impl JupyterPrompter {
  pub fn new(input_tx: mpsc::UnboundedSender<PendingInputRequest>) -> Self {
    Self { input_tx }
  }
}

impl PermissionPrompter for JupyterPrompter {
  fn prompt(
    &mut self,
    message: &str,
    name: &str,
    _api_name: Option<&str>,
    is_unary: bool,
    _get_stack: Option<GetFormattedStackFn>,
  ) -> PromptResponse {
    let options = if is_unary {
      format!("[y = allow once, A = allow all {name} access, n = deny]")
    } else {
      "[y = allow, n = deny]".to_string()
    };
    let prompt = format!("Deno requests {message}.\nAllow? {options}");

    // The frontend renders this on the stdin channel and returns the typed
    // answer. `None` means stdin is unavailable (the frontend doesn't support
    // it) or the user dismissed the prompt; both are treated as a denial, which
    // matches the terminal prompter's behavior when it can't prompt.
    match request_input_blocking(&self.input_tx, prompt, false)
      .as_deref()
      .map(str::trim)
    {
      Some("y") | Some("Y") => PromptResponse::Allow,
      Some("A") if is_unary => PromptResponse::AllowAll,
      _ => PromptResponse::Deny,
    }
  }
}
