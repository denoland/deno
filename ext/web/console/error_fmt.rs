// Copyright 2018-2026 the Deno authors. MIT license.

//! Error formatting ported from `01_console.js` (`formatError`,
//! `getStackString`, `improveStack`, stack-frame collapsing, cwd marking).

use deno_core::v8;

use super::inspect::*;

/// `ObjectPrototypeToString(value)`.
fn object_proto_to_string<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: v8::Local<'s, v8::Value>,
) -> String {
  let tag: Option<String> = match v8::Local::<v8::Object>::try_from(value) {
    Ok(obj) => {
      let sym = v8::Symbol::get_to_string_tag(scope);
      v8::tc_scope!(tc, scope);
      match obj.get(tc, sym.into()) {
        Some(v) if v.is_string() => Some(v.to_rust_string_lossy(tc)),
        _ => None,
      }
    }
    Err(_) => None,
  };
  let builtin = if let Some(tag) = tag {
    tag
  } else if value.is_array() {
    "Array".to_string()
  } else if value.is_function() {
    "Function".to_string()
  } else if value.is_native_error() {
    "Error".to_string()
  } else if value.is_date() {
    "Date".to_string()
  } else if value.is_reg_exp() {
    "RegExp".to_string()
  } else {
    "Object".to_string()
  };
  format!("[object {builtin}]")
}

/// `ErrorPrototypeToString(error)`; exceptions from `name`/`message` getters
/// propagate.
fn error_proto_to_string<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  error: v8::Local<'s, v8::Object>,
) -> R<String> {
  let name_val = js_get_str(scope, error, "name")?;
  let name = if name_val.is_undefined() {
    "Error".to_string()
  } else {
    let s = {
      v8::tc_scope!(tc, scope);
      match name_val.to_string(tc) {
        Some(s) => Ok(s.to_rust_string_lossy(tc)),
        None => {
          let exc = tc.exception();
          Err(grab_err(tc, exc))
        }
      }
    };
    s?
  };
  let message_val = js_get_str(scope, error, "message")?;
  let message = if message_val.is_undefined() {
    String::new()
  } else {
    let s = {
      v8::tc_scope!(tc, scope);
      match message_val.to_string(tc) {
        Some(s) => Ok(s.to_rust_string_lossy(tc)),
        None => {
          let exc = tc.exception();
          Err(grab_err(tc, exc))
        }
      }
    };
    s?
  };
  if name.is_empty() {
    return Ok(message);
  }
  if message.is_empty() {
    return Ok(name);
  }
  Ok(format!("{name}: {message}"))
}

/// `getStackString(ctx, error)`.
pub fn get_stack_string<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'s>,
  ctx: &mut Ctx<'s>,
  error: v8::Local<'s, v8::Object>,
) -> R<String> {
  let stack = {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "stack");
    // If `stack` is a getter that throws, ignore the error.
    obj_get_swallow(tc, error, key.into())
  };
  if let Some(stack) = stack {
    if stack.boolean_value(scope) {
      if stack.is_string() {
        return Ok(stack.to_rust_string_lossy(scope));
      }
      ctx.seen.push(error.into());
      ctx.indentation_lvl += 4;
      let result = format_value(scope, intr, ctx, stack, f64::NAN, false)?;
      ctx.indentation_lvl -= 4;
      ctx.seen.pop();
      let prefix = error_proto_to_string(scope, error)?;
      return Ok(format!("{prefix}\n    {result}"));
    }
  }
  error_proto_to_string(scope, error)
}

fn obj_get_swallow<'s>(
  tc: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: v8::Local<'s, v8::Value>,
) -> Option<v8::Local<'s, v8::Value>> {
  obj.get(tc, key)
}

/// `improveStack(stack, constructor, name, tag)`. `name` is the raw value of
/// `err.name` (may be a non-string).
fn improve_stack(
  mut stack: String,
  constructor: Option<&str>,
  name: &NameInfo,
  tag: &str,
) -> String {
  // let len = name.length;
  let len: Option<usize> = name.utf16_len;

  if !name.is_string {
    // stack = replace(stack, `${name}`, `${name} [${prefix - last char}]`)
    let prefix = get_prefix(constructor, tag, "Error", "");
    let prefix_trimmed = &prefix[..prefix.len().saturating_sub(1)];
    let replacement = format!("{} [{}]", name.display, prefix_trimmed);
    if let Some(pos) = stack.find(&name.display) {
      stack = format!(
        "{}{}{}",
        &stack[..pos],
        replacement,
        &stack[pos + name.display.len()..]
      );
    }
  }

  let name_check = name.is_string
    && name.display.ends_with("Error")
    && stack.starts_with(&name.display)
    && (stack.len() == name.display.len()
      || stack[name.display.len()..].starts_with(':')
      || stack[name.display.len()..].starts_with('\n'));

  if constructor.is_none() || name_check {
    let mut fallback = "Error".to_string();
    let mut len = len;
    if constructor.is_none() {
      let start = match_null_proto_error_start(&stack);
      fallback = start.clone().unwrap_or_default();
      len = Some(fallback.encode_utf16().count());
      if fallback.is_empty() {
        fallback = "Error".to_string();
      }
    }
    let prefix_full = get_prefix(constructor, tag, &fallback, "");
    let prefix = &prefix_full[..prefix_full.len().saturating_sub(1)];
    // JS: `if (name !== prefix)` — a non-string name never equals a string.
    if name.display != prefix || !name.is_string {
      if prefix.contains(&name.display) {
        if len == Some(0) {
          stack = format!("{prefix}: {stack}");
        } else {
          // Non-string name: `StringPrototypeSlice(stack, undefined)` is
          // the full string (byte_len 0).
          let byte_len = match len {
            Some(l) => utf16_to_byte_len(&stack, l),
            None => 0,
          };
          stack = format!("{prefix}{}", &stack[byte_len..]);
        }
      } else {
        let byte_len = match len {
          Some(l) => utf16_to_byte_len(&stack, l),
          None => 0,
        };
        stack = format!("{prefix} [{}]{}", name.display, &stack[byte_len..]);
      }
    }
  }
  stack
}

/// `^([A-Z][a-z_ A-Z0-9[\]()-]+)(?::|\n {4}at)` or `^([a-z_A-Z0-9-]*Error)$`
fn match_null_proto_error_start(stack: &str) -> Option<String> {
  // First pattern.
  let chars: Vec<char> = stack.chars().collect();
  if let Some(first) = chars.first() {
    if first.is_ascii_uppercase() {
      let mut end = 1;
      while end < chars.len() {
        let c = chars[end];
        if c.is_ascii_lowercase()
          || c == '_'
          || c == ' '
          || c.is_ascii_uppercase()
          || c.is_ascii_digit()
          || matches!(c, '[' | ']' | '(' | ')' | '-')
        {
          end += 1;
        } else {
          break;
        }
      }
      // The matched group needs at least 2 chars ([A-Z] + one more), then a
      // `:` or `\n    at` must follow. The `+` is greedy with backtracking:
      // find the longest end where the suffix matches.
      let mut e = end;
      while e >= 2 {
        let rest: String = chars[e..].iter().collect();
        if rest.starts_with(':') || rest.starts_with("\n    at") {
          return Some(chars[..e].iter().collect());
        }
        e -= 1;
      }
    }
  }
  // Second pattern: entire stack is `[a-z_A-Z0-9-]*Error`.
  if stack.ends_with("Error")
    && stack
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
  {
    return Some(stack.to_string());
  }
  None
}

fn utf16_to_byte_len(s: &str, utf16_len: usize) -> usize {
  let mut count = 0usize;
  for (byte_idx, c) in s.char_indices() {
    if count >= utf16_len {
      return byte_idx;
    }
    count += c.len_utf16();
  }
  s.len()
}

struct NameInfo {
  display: String,
  is_string: bool,
  utf16_len: Option<usize>,
}

/// `getDuplicateErrorFrameRanges(frames)`.
fn get_duplicate_error_frame_ranges(frames: &[String]) -> Vec<usize> {
  let mut result: Vec<usize> = Vec::new();
  let mut line_to_positions: Vec<(usize, Vec<usize>)> = Vec::new();
  let mut index_of: std::collections::HashMap<&str, usize> =
    std::collections::HashMap::new();

  for (i, frame) in frames.iter().enumerate() {
    match index_of.get(frame.as_str()) {
      Some(&idx) => line_to_positions[idx].1.push(i),
      None => {
        index_of.insert(frame.as_str(), line_to_positions.len());
        line_to_positions.push((i, vec![i]));
      }
    }
  }

  let minimum_duplicate_range = 3usize;
  if frames.len() - line_to_positions.len() <= minimum_duplicate_range {
    return result;
  }

  let mut i = 0usize;
  while i + minimum_duplicate_range < frames.len() {
    let positions =
      &line_to_positions[*index_of.get(frames[i].as_str()).unwrap()].1;
    if positions.len() == 1 || *positions.last().unwrap() == i {
      i += 1;
      continue;
    }
    let current = positions.iter().position(|&p| p == i).unwrap() + 1;
    if current == positions.len() {
      i += 1;
      continue;
    }

    let mut range = positions[positions.len() - 1] - i;
    if range < minimum_duplicate_range {
      i += 1;
      continue;
    }
    let mut extra_steps: Option<Vec<usize>> = None;
    if current + 1 < positions.len() {
      // GCD of candidate distances.
      let mut gcd_range = 0usize;
      let mut steps: Vec<usize> = Vec::new();
      for &p in &positions[current..] {
        let mut distance = p - i;
        while distance != 0 {
          let remainder = if distance == 0 { 0 } else { gcd_range % distance };
          if gcd_range != 0 {
            if !steps.contains(&gcd_range) {
              steps.push(gcd_range);
            }
          }
          gcd_range = distance;
          distance = remainder;
        }
        if gcd_range == 1 {
          break;
        }
      }
      range = gcd_range;
      steps.retain(|&s| s != range);
      if !steps.is_empty() {
        extra_steps = Some(steps);
      }
    }
    let mut max_range = range;
    let mut max_duplicates = 0usize;
    let mut duplicate_ranges = 0usize;

    let mut next_start = i + range;
    loop {
      let mut equal_frames = 0usize;
      for j in 0..range {
        if frames.get(i + j) != frames.get(next_start + j) {
          break;
        }
        equal_frames += 1;
      }
      if equal_frames != range {
        let has_extra = extra_steps
          .as_ref()
          .map(|s| !s.is_empty())
          .unwrap_or(false);
        if !has_extra {
          break;
        }
        if duplicate_ranges != 0
          && max_range * max_duplicates < range * duplicate_ranges
        {
          max_range = range;
          max_duplicates = duplicate_ranges;
        }
        range = extra_steps.as_mut().unwrap().pop().unwrap();
        next_start = i;
        duplicate_ranges = 0;
        next_start += range;
        continue;
      }
      duplicate_ranges += 1;
      next_start += range;
    }

    if max_duplicates != 0
      && max_range * max_duplicates >= range * duplicate_ranges
    {
      range = max_range;
      duplicate_ranges = max_duplicates;
    }

    if duplicate_ranges * range >= 3 {
      result.push(i + range);
      result.push(range);
      result.push(duplicate_ranges);
      i += range * (duplicate_ranges + 1) - 1;
    }
    i += 1;
  }

  result
}

/// `identicalSequenceRange(a, b)`.
fn identical_sequence_range(a: &[String], b: &[String]) -> (usize, usize) {
  if a.len() < 4 {
    return (0, 0);
  }
  for i in 0..a.len() - 3 {
    if let Some(pos) = b.iter().position(|x| *x == a[i]) {
      let rest = b.len() - pos;
      if rest > 3 {
        let mut len = 1usize;
        let max_len = (a.len() - i).min(rest);
        while max_len > len && a[i + len] == b[pos + len] {
          len += 1;
        }
        if len > 3 {
          return (len, i);
        }
      }
    }
  }
  (0, 0)
}

/// `getStackFrames(ctx, err, stack)`.
fn get_stack_frames<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'s>,
  ctx: &mut Ctx<'s>,
  err: v8::Local<'s, v8::Object>,
  stack: &str,
) -> R<Vec<String>> {
  let mut frames: Vec<String> =
    stack.split('\n').map(|s| s.to_string()).collect();

  let cause = {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "cause");
    err.get(tc, key.into())
  };

  // Remove stack frames identical to frames in cause.
  if let Some(cause) = cause {
    let is_error = cause.is_native_error()
      || is_error_instance(scope, intr, cause);
    if !cause.is_null_or_undefined() && is_error {
      if let Ok(cause_obj) = v8::Local::<v8::Object>::try_from(cause) {
        let cause_stack = get_stack_string(scope, intr, ctx, cause_obj)?;
        if let Some(cause_stack_start) = cause_stack.find("\n    at") {
          let cause_frames: Vec<String> = cause_stack
            [cause_stack_start + 1..]
            .split('\n')
            .map(|s| s.to_string())
            .collect();
          let (len, offset) = identical_sequence_range(&frames, &cause_frames);
          if len > 0 {
            let skipped = len - 2;
            let msg =
              format!("    ... {skipped} lines matching cause stack trace ...");
            let styled = ctx.stylize(scope, &msg, "undefined")?;
            let end = (offset + 1 + skipped).min(frames.len());
            frames.splice(offset + 1..end, [styled]);
          }
        }
      }
    }
  }

  // Remove recursive repetitive stack frames in long stacks.
  if frames.len() > 10 {
    let ranges = get_duplicate_error_frame_ranges(&frames);
    let mut i = ranges.len() as i64 - 3;
    while i >= 0 {
      let idx = i as usize;
      let offset = ranges[idx];
      let length = ranges[idx + 1];
      let duplicate_ranges = ranges[idx + 2];
      let msg = format!(
        "    ... collapsed {} duplicate lines matching above {}",
        length * duplicate_ranges,
        if duplicate_ranges > 1 {
          format!("{length} lines {duplicate_ranges} times...")
        } else {
          "lines ...".to_string()
        }
      );
      let styled = ctx.stylize(scope, &msg, "undefined")?;
      let end = (offset + length * duplicate_ranges).min(frames.len());
      frames.splice(offset..end, [styled]);
      i -= 3;
    }
  }

  Ok(frames)
}

fn is_error_instance<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  intr: &Intrinsics<'s>,
  value: v8::Local<'s, v8::Value>,
) -> bool {
  is_prototype_of(scope, intr.error_prototype.into(), value)
}

/// `markNodeModules(ctx, line)`.
fn mark_node_modules<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  line: &str,
) -> R<String> {
  let mut temp_line = String::new();
  let mut last_pos = 0usize;
  let mut search_from = 0usize;

  loop {
    let Some(rel) = line.get(search_from..).and_then(|s| s.find("node_modules"))
    else {
      break;
    };
    let node_module_position = search_from + rel;

    let separator = if node_module_position == 0 {
      None
    } else {
      line[..node_module_position].chars().last()
    };
    let after = line[node_module_position + 12..].chars().next();

    let sep_ok = matches!(separator, Some('/') | Some('\\'));
    let after_ok = matches!(after, Some('/') | Some('\\'));
    if !sep_ok || !after_ok {
      search_from = node_module_position + 1;
      continue;
    }
    let separator = separator.unwrap();

    let module_start = node_module_position + 13; // include trailing separator
    temp_line.push_str(&line[last_pos..module_start]);

    let mut module_end = line[module_start..]
      .find(separator)
      .map(|p| module_start + p);
    if line[module_start..].starts_with('@') {
      // Namespaced modules have an extra slash: @namespace/package
      if let Some(me) = module_end {
        module_end = line[me + 1..].find(separator).map(|p| me + 1 + p);
      }
    }
    let Some(module_end) = module_end else {
      // JS would compute indexOf == -1 and slice(start, -1)-ish chaos;
      // practically module_end is always found in real frames. Bail out.
      temp_line.push_str(&line[module_start..]);
      last_pos = line.len();
      break;
    };

    let node_module = &line[module_start..module_end];
    temp_line.push_str(&ctx.stylize(scope, node_module, "module")?);

    last_pos = module_end;
    search_from = module_end;
  }

  if last_pos != 0 {
    Ok(format!("{temp_line}{}", &line[last_pos..]))
  } else {
    Ok(line.to_string())
  }
}

/// `markCwd(ctx, line, workingDirectory)`.
fn mark_cwd<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  ctx: &mut Ctx<'s>,
  line: &str,
  working_directory: &str,
) -> R<String> {
  let Some(mut cwd_start_pos) = line.find(working_directory) else {
    return Ok(line.to_string());
  };
  let mut temp_line = String::new();
  let mut cwd_length = working_directory.len();
  if cwd_start_pos >= 7 && &line[cwd_start_pos - 7..cwd_start_pos] == "file://"
  {
    cwd_length += 7;
    cwd_start_pos -= 7;
  }
  let start = if cwd_start_pos > 0
    && line[..cwd_start_pos].ends_with('(')
  {
    cwd_start_pos - 1
  } else {
    cwd_start_pos
  };
  let strip_close = start != cwd_start_pos && line.ends_with(')');
  let end = if strip_close { line.len() - 1 } else { line.len() };
  let working_directory_end_pos =
    (cwd_start_pos + cwd_length + 1).min(line.len());
  let cwd_slice = &line[start..working_directory_end_pos];

  temp_line.push_str(&line[..start]);
  temp_line.push_str(&ctx.stylize(scope, cwd_slice, "undefined")?);
  if working_directory_end_pos < end {
    temp_line.push_str(&line[working_directory_end_pos..end]);
  }
  if strip_close {
    temp_line.push_str(&ctx.stylize(scope, ")", "undefined")?);
  }
  Ok(temp_line)
}

// --- frame regexes -----------------------------------------------------

fn frame_after_at(line: &str) -> Option<&str> {
  line.strip_prefix("    at ")
}

/// `^ {4}at (?:[^/\\(]+ \(|)<prefix>.+:\d+:\d+\)?$`
fn match_module_frame<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
  let rest = frame_after_at(line)?;
  // Optional "funcName (" where funcName has no '/', '\' or '('.
  let body = if let Some(paren) = rest.find(" (") {
    let func = &rest[..paren];
    if !func.is_empty()
      && !func.contains('/')
      && !func.contains('\\')
      && !func.contains('(')
    {
      &rest[paren + 2..]
    } else {
      rest
    }
  } else {
    rest
  };
  let spec = body.strip_prefix(prefix)?;
  // `.+:\d+:\d+\)?$`
  let spec = spec.strip_suffix(')').unwrap_or(spec);
  // Trailing :col
  let (rest1, col) = spec.rsplit_once(':')?;
  if col.is_empty() || !col.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  let (module_path, lineno) = rest1.rsplit_once(':')?;
  if lineno.is_empty() || !lineno.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  if module_path.is_empty() {
    return None;
  }
  Some(module_path)
}

fn is_core_module_frame(line: &str) -> bool {
  match_module_frame(line, "node:").is_some()
}

fn is_ext_module_frame(line: &str) -> bool {
  match_module_frame(line, "ext:").is_some()
}

/// `^ {4}at (?:__node_internal_\S+|eventLoopTick|denoErrorToNodeError|__drainNextTickAndMacrotasks) `
fn is_filtered_ext_frame(line: &str) -> bool {
  let Some(rest) = frame_after_at(line) else {
    return false;
  };
  for name in [
    "eventLoopTick",
    "denoErrorToNodeError",
    "__drainNextTickAndMacrotasks",
  ] {
    if rest.starts_with(name)
      && rest[name.len()..].starts_with(' ')
    {
      return true;
    }
  }
  if let Some(after) = rest.strip_prefix("__node_internal_") {
    if let Some(space) = after.find(' ') {
      if space > 0 {
        return true;
      }
    }
  }
  false
}

// --- formatError --------------------------------------------------------

/// `formatError(err, constructor, tag, ctx, keys)`.
pub fn format_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  intr: &Intrinsics<'s>,
  ctx: &mut Ctx<'s>,
  err: v8::Local<'s, v8::Object>,
  constructor: Option<&str>,
  tag: &str,
  keys: &mut Vec<v8::Local<'s, v8::Value>>,
) -> R<String> {
  let mut stack = match get_stack_string(scope, intr, ctx, err) {
    Ok(s) => s,
    Err(_) => return Ok(object_proto_to_string(scope, err.into())),
  };

  let mut message: Option<v8::Local<'s, v8::Value>> = None;
  let mut message_is_getter_that_throws = false;
  {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "message");
    match err.get(tc, key.into()) {
      Some(v) => message = Some(v),
      None => message_is_getter_that_throws = true,
    }
  }
  let mut name: Option<v8::Local<'s, v8::Value>> = None;
  let mut name_is_getter_that_throws = false;
  {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "name");
    match err.get(tc, key.into()) {
      Some(v) => name = Some(v),
      None => name_is_getter_that_throws = true,
    }
  }

  let key_string = |scope: &mut v8::PinScope<'s, '_>,
                    key: &v8::Local<'s, v8::Value>|
   -> Option<String> {
    if key.is_string() {
      Some(key.to_rust_string_lossy(scope))
    } else {
      None
    }
  };

  if !ctx.show_hidden && !keys.is_empty() {
    // Remove "stack".
    if let Some(pos) = keys
      .iter()
      .position(|k| key_string(scope, k).as_deref() == Some("stack"))
    {
      keys.remove(pos);
    }

    if !message_is_getter_that_throws {
      let msg_val = message.unwrap_or_else(|| v8::undefined(scope).into());
      let msg_str = if msg_val.is_string() {
        Some(msg_val.to_rust_string_lossy(scope))
      } else {
        None
      };
      if let Some(pos) = keys
        .iter()
        .position(|k| key_string(scope, k).as_deref() == Some("message"))
      {
        // Only hide if it's a string and part of the original stack.
        let hide = match &msg_str {
          None => true,
          Some(m) => stack.contains(m.as_str()),
        };
        if hide {
          keys.remove(pos);
        }
      }
    }

    if !name_is_getter_that_throws {
      let name_val = name.unwrap_or_else(|| v8::undefined(scope).into());
      let name_str = if name_val.is_string() {
        Some(name_val.to_rust_string_lossy(scope))
      } else {
        None
      };
      if let Some(pos) = keys
        .iter()
        .position(|k| key_string(scope, k).as_deref() == Some("name"))
      {
        let hide = match &name_str {
          None => true,
          Some(n) => stack.contains(n.as_str()),
        };
        if hide {
          keys.remove(pos);
        }
      }
    }
  }

  // name ??= "Error";
  let name_info = {
    let name_val = if name_is_getter_that_throws {
      None
    } else {
      name
    };
    match name_val {
      Some(v) if !v.is_null_or_undefined() => {
        let display = v.to_rust_string_lossy(scope);
        let is_string = v.is_string();
        let utf16_len = if is_string {
          Some(display.encode_utf16().count())
        } else {
          None
        };
        NameInfo {
          display,
          is_string,
          utf16_len,
        }
      }
      _ => NameInfo {
        display: "Error".to_string(),
        is_string: true,
        utf16_len: Some(5),
      },
    }
  };

  // Push "cause" if present and not already listed.
  let has_cause = {
    v8::tc_scope!(tc, scope);
    let key = v8_str(tc, "cause");
    err.has(tc, key.into()).unwrap_or(false)
  };
  if has_cause {
    let already = keys
      .iter()
      .any(|k| key_string(scope, k).as_deref() == Some("cause"));
    if keys.is_empty() || !already {
      let cause_key = v8_str(scope, "cause");
      keys.push(cause_key.into());
    }
  }

  // Print errors aggregated into AggregateError.
  {
    let errors = {
      v8::tc_scope!(tc, scope);
      let key = v8_str(tc, "errors");
      err.get(tc, key.into())
    };
    if let Some(errors) = errors {
      if errors.is_array() {
        let already = keys
          .iter()
          .any(|k| key_string(scope, k).as_deref() == Some("errors"));
        if keys.is_empty() || !already {
          let errors_key = v8_str(scope, "errors");
          keys.push(errors_key.into());
        }
      }
    }
  }

  stack = improve_stack(stack, constructor, &name_info, tag);

  // Ignore the error message if it's contained in the stack.
  let mut pos = 0usize;
  if let Some(message) = message {
    if !message_is_getter_that_throws && message.is_string() {
      let msg = message.to_rust_string_lossy(scope);
      if !msg.is_empty() {
        if let Some(p) = stack.find(&msg) {
          pos = p + msg.len();
        }
      }
    }
  }
  // Wrap the error in brackets in case it has no stack trace.
  let stack_start = stack.get(pos..).and_then(|s| s.find("\n    at"))
    .map(|p| pos + p);
  match stack_start {
    None => {
      stack = format!("[{stack}]");
    }
    Some(stack_start) => {
      let mut new_stack = stack[..stack_start].to_string();
      let stack_frame_part = &stack[stack_start + 1..];
      let lines =
        get_stack_frames(scope, intr, ctx, err, stack_frame_part)?;
      if ctx.colors {
        // Highlight userland code and node modules.
        let working_directory = {
          let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
          let ret = js_call(scope, intr.get_cwd, undef, &[])?;
          if ret.is_string() {
            Some(ret.to_rust_string_lossy(scope))
          } else {
            None
          }
        };
        let mut esm_working_directory: Option<String> = None;
        for line in &lines {
          if is_filtered_ext_frame(line) && is_ext_module_frame(line) {
            continue;
          }
          // Frames in node:* / ext:* modules are builtin frames.
          if is_core_module_frame(line) || is_ext_module_frame(line) {
            let styled = ctx.stylize(scope, line, "undefined")?;
            new_stack.push_str(&format!("\n{styled}"));
          } else {
            new_stack.push('\n');
            let mut line = mark_node_modules(scope, ctx, line)?;
            if let Some(wd) = &working_directory {
              let new_line = mark_cwd(scope, ctx, &line, wd)?;
              if new_line == line {
                if esm_working_directory.is_none() {
                  esm_working_directory = Some(path_to_file_url_href(wd));
                }
                line = mark_cwd(
                  scope,
                  ctx,
                  &line,
                  esm_working_directory.as_ref().unwrap(),
                )?;
              } else {
                line = new_line;
              }
            }
            new_stack.push_str(&line);
          }
        }
      } else {
        for line in &lines {
          if is_filtered_ext_frame(line) && is_ext_module_frame(line) {
            continue;
          }
          new_stack.push_str(&format!("\n{line}"));
        }
      }
      stack = new_stack;
    }
  }

  // The message and the stack have to be indented as well.
  if ctx.indentation_lvl != 0 {
    let indentation = " ".repeat(ctx.indentation_lvl);
    stack = stack.replace('\n', &format!("\n{indentation}"));
  }
  Ok(stack)
}

fn path_to_file_url_href(path: &str) -> String {
  match deno_core::url::Url::from_file_path(path) {
    Ok(url) => url.to_string(),
    Err(_) => String::new(),
  }
}
