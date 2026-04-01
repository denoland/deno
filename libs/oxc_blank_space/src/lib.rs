// Copyright 2024 oxc_blank_space authors. MIT license.
//
// A Rust implementation of the ts-blank-space concept using OXC's parser.
// Strips TypeScript type syntax by replacing it with whitespace,
// preserving exact line/column positions so no source map is needed.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::scope::ScopeFlags;

/// Errors that can occur during blank-space transformation.
#[derive(Debug)]
pub enum BlankSpaceError {
    UnsupportedSyntax { span: Span, message: String },
    ParseError(String),
}

impl std::fmt::Display for BlankSpaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlankSpaceError::UnsupportedSyntax { span, message } => {
                write!(f, "Unsupported syntax at {}..{}: {}", span.start, span.end, message)
            }
            BlankSpaceError::ParseError(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl std::error::Error for BlankSpaceError {}

/// An operation to apply to the output.
#[derive(Debug)]
enum Op {
    /// Replace a byte range with spaces (preserving newlines).
    Blank(Span),
    /// Set a single byte to a specific character.
    SetChar { pos: u32, ch: u8 },
}

/// Transform TypeScript source to JavaScript by replacing type annotations
/// with whitespace. The output has identical line/column positions for all
/// runtime code — no source map needed.
pub fn blank_space(source: &str) -> Result<String, Vec<BlankSpaceError>> {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let ret = Parser::new(&allocator, source, source_type).parse();

    if ret.panicked {
        return Err(vec![BlankSpaceError::ParseError("Parser panicked".to_string())]);
    }

    let mut collector = Collector {
        source,
        ops: Vec::new(),
        errors: Vec::new(),
    };
    collector.visit_program(&ret.program);

    if !collector.errors.is_empty() {
        return Err(collector.errors);
    }

    // Post-process: remove SetChar(';') ops where the position's `needs_semi_before`
    // would find a character that is itself inside a blanked span.
    // This prevents false `;` insertion between adjacent blanked statements.
    let blank_spans: Vec<Span> = collector
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::Blank(s) => Some(*s),
            _ => None,
        })
        .collect();

    let is_in_blanked_span = |pos: u32| -> bool {
        blank_spans.iter().any(|s| s.start <= pos && pos < s.end)
    };

    // For each SetChar(';'), verify the "previous non-ws" char isn't inside a blanked span
    collector.ops.retain(|op| {
        if let Op::SetChar { pos, ch: b';' } = op {
            // Find the last non-ws/comment char before this position
            let prev = find_last_significant_char(source, *pos);
            if let Some(prev_pos) = prev {
                if is_in_blanked_span(prev_pos as u32) {
                    return false; // Remove this `;` — previous is blanked
                }
            }
        }
        true
    });

    let mut output = source.as_bytes().to_vec();

    // Apply blank ops first, then set-char ops (so set-char wins)
    for op in &collector.ops {
        if let Op::Blank(span) = op {
            blank_bytes(&mut output, span.start as usize, span.end as usize);
        }
    }
    for op in &collector.ops {
        if let Op::SetChar { pos, ch } = op {
            output[*pos as usize] = *ch;
        }
    }

    Ok(String::from_utf8(output).expect("blanking only replaces ASCII with ASCII"))
}

/// Replace bytes in range with spaces, preserving newlines.
fn blank_bytes(output: &mut [u8], start: usize, end: usize) {
    for byte in &mut output[start..end] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

// ============================================================================
// ASI helpers
// ============================================================================

/// Tokens that cause ASI hazards when they appear at the start of the next line.
fn is_asi_hazard(ch: u8) -> bool {
    matches!(ch, b'(' | b'[' | b'`')
}

/// Check if the last non-whitespace, non-comment character before `pos` is NOT `;`.
/// If so, a `;` may be needed for ASI safety.
fn needs_semi_before(source: &str, pos: u32) -> bool {
    let bytes = source.as_bytes();
    let mut i = pos as usize;
    loop {
        if i == 0 {
            return false;
        }
        i -= 1;
        let b = bytes[i];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            continue;
        }
        // Check for block comment ending `*/`
        if b == b'/' && i > 0 && bytes[i - 1] == b'*' {
            if let Some(start) = find_block_comment_start(bytes, i - 1) {
                i = start;
                continue;
            }
        }
        // Check for end of line comment: if we're on a non-ws char, check if
        // this line has a `//` before this position.
        // Find the start of the current line
        let line_start = bytes[..=i].iter().rposition(|&b| b == b'\n').map(|p| p + 1).unwrap_or(0);
        if let Some(comment_pos) = find_line_comment(bytes, line_start, i) {
            // Everything from `//` to end of line is comment — skip to before `//`
            i = comment_pos;
            continue;
        }
        return b != b';';
    }
}

/// Find `//` in bytes[start..=end] and return its position, or None.
fn find_line_comment(bytes: &[u8], start: usize, end: usize) -> Option<usize> {
    // Simple scan — look for `//` that's not inside a string
    let mut j = start;
    while j < end {
        if bytes[j] == b'/' && j + 1 <= end && bytes[j + 1] == b'/' {
            return Some(j);
        }
        // Skip string literals to avoid false positives
        if bytes[j] == b'\'' || bytes[j] == b'"' || bytes[j] == b'`' {
            let quote = bytes[j];
            j += 1;
            while j <= end && bytes[j] != quote {
                if bytes[j] == b'\\' {
                    j += 1; // skip escaped char
                }
                j += 1;
            }
        }
        j += 1;
    }
    None
}

fn find_block_comment_start(bytes: &[u8], star_pos: usize) -> Option<usize> {
    let mut j = star_pos;
    loop {
        if j == 0 {
            return None;
        }
        j -= 1;
        if bytes[j] == b'*' && j > 0 && bytes[j - 1] == b'/' {
            return Some(j - 1);
        }
    }
}

/// Find the byte position of the last significant (non-ws, non-comment) char before `pos`.
fn find_last_significant_char(source: &str, pos: u32) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = pos as usize;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        let b = bytes[i];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            continue;
        }
        if b == b'/' && i > 0 && bytes[i - 1] == b'*' {
            if let Some(start) = find_block_comment_start(bytes, i - 1) {
                i = start;
                continue;
            }
        }
        let line_start = bytes[..=i]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        if let Some(comment_pos) = find_line_comment(bytes, line_start, i) {
            i = comment_pos;
            continue;
        }
        return Some(i);
    }
}

/// Check if the first non-whitespace, non-comment character after `pos` is an ASI hazard.
fn next_is_asi_hazard(source: &str, pos: u32) -> bool {
    let bytes = source.as_bytes();
    let mut i = pos as usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            i += 1;
            continue;
        }
        // Skip line comments
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Skip block comments
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        return is_asi_hazard(b);
    }
    false
}

// ============================================================================
// Collector
// ============================================================================

struct Collector<'a> {
    source: &'a str,
    ops: Vec<Op>,
    errors: Vec<BlankSpaceError>,
}

impl Collector<'_> {
    fn blank(&mut self, span: Span) {
        if span.start < span.end {
            self.ops.push(Op::Blank(span));
        }
    }

    fn set_char(&mut self, pos: u32, ch: u8) {
        self.ops.push(Op::SetChar { pos, ch });
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(BlankSpaceError::UnsupportedSyntax {
            span,
            message: message.into(),
        });
    }

    /// Blank a full statement and insert `;` if needed for ASI safety.
    fn blank_statement(&mut self, span: Span, semi_pos: u32) {
        self.blank(span);
        if needs_semi_before(self.source, semi_pos) {
            self.set_char(semi_pos, b';');
        }
    }

    /// Blank an `as`/`satisfies` suffix and insert `;` if next token is ASI hazard.
    fn blank_as_satisfies(&mut self, outer_span: Span, expr: &Expression<'_>) {
        let expr_end = expr.span().end;
        self.blank(Span::new(expr_end, outer_span.end));
        // Check if next non-blank after the outer span is an ASI hazard
        if next_is_asi_hazard(self.source, outer_span.end) {
            self.set_char(expr_end, b';');
        }
    }

    fn blank_implements_clause(&mut self, implements: &[TSClassImplements<'_>]) {
        if implements.is_empty() {
            return;
        }
        let first = implements.first().unwrap().span();
        let last = implements.last().unwrap().span();
        let before = &self.source[..first.start as usize];
        if let Some(kw_start) = before.rfind("implements") {
            self.blank(Span::new(kw_start as u32, last.end));
        }
    }

    fn blank_accessibility(&mut self, accessibility: &TSAccessibility, search_start: u32) {
        let keyword = match accessibility {
            TSAccessibility::Private => "private",
            TSAccessibility::Protected => "protected",
            TSAccessibility::Public => "public",
        };
        let start = search_start as usize;
        let region = &self.source[start..];
        if let Some(offset) = region.find(keyword) {
            let abs_start = (start + offset) as u32;
            self.blank(Span::new(abs_start, abs_start + keyword.len() as u32));
            // Check if blanking this modifier creates an ASI hazard
            // (e.g., `public ["computed"]` → `["computed"]` could be indexing)
            let after_kw = abs_start + keyword.len() as u32;
            if next_is_asi_hazard(self.source, after_kw) && needs_semi_before(self.source, abs_start) {
                self.set_char(abs_start, b';');
            }
        }
    }

    fn blank_keyword_between(&mut self, keyword: &str, after_pos: u32, before_pos: u32) {
        let region = &self.source[after_pos as usize..before_pos as usize];
        if let Some(offset) = region.find(keyword) {
            let abs_start = after_pos as usize + offset;
            self.blank(Span::new(abs_start as u32, abs_start as u32 + keyword.len() as u32));
        }
    }

    fn blank_char_after(&mut self, ch: u8, after_pos: u32, limit: u32) {
        let end = std::cmp::min(after_pos + limit, self.source.len() as u32);
        let bytes = self.source[after_pos as usize..end as usize].as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == ch {
                let pos = after_pos + i as u32;
                self.blank(Span::new(pos, pos + 1));
                return;
            }
            // Skip block comments
            if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }
            return; // non-whitespace, non-comment, non-target
        }
    }

    fn blank_type_specifier_in_list(&mut self, specifier_span: Span) {
        // OXC includes the `type` keyword in the specifier span for inline type imports/exports.
        // We need to blank the specifier plus any trailing comma.
        let start = specifier_span.start;
        let after = &self.source[specifier_span.end as usize..];
        let mut end = specifier_span.end as usize;
        let after_bytes = after.as_bytes();
        let mut i = 0;
        // Skip whitespace and comments after specifier
        while i < after_bytes.len() {
            if after_bytes[i] == b' ' || after_bytes[i] == b'\t' {
                i += 1;
            } else if after_bytes[i] == b'/' && i + 1 < after_bytes.len() && after_bytes[i + 1] == b'*' {
                // Skip block comment
                i += 2;
                while i + 1 < after_bytes.len() {
                    if after_bytes[i] == b'*' && after_bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            } else {
                break;
            }
        }
        if i < after_bytes.len() && after_bytes[i] == b',' {
            end = specifier_span.end as usize + i + 1;
        }
        self.blank(Span::new(start, end as u32));
    }

    /// Find a specific character after a position, returning its position + 1.
    fn find_char_after(&self, ch: u8, after_pos: u32, limit: u32) -> Option<u32> {
        let end = std::cmp::min(after_pos + limit, self.source.len() as u32);
        let region = self.source[after_pos as usize..end as usize].as_bytes();
        for (i, &b) in region.iter().enumerate() {
            if b == ch {
                return Some(after_pos + i as u32 + 1);
            }
        }
        None
    }

    fn blank_this_param(&mut self, this_param: &TSThisParameter<'_>) {
        let span = this_param.span;
        let after = &self.source[span.end as usize..];
        let after_bytes = after.as_bytes();
        let mut end = span.end as usize;
        let mut i = 0;
        while i < after_bytes.len() && (after_bytes[i] == b' ' || after_bytes[i] == b'\t') {
            i += 1;
        }
        if i < after_bytes.len() && after_bytes[i] == b',' {
            end = span.end as usize + i + 1;
        }
        self.blank(Span::new(span.start, end as u32));
    }

    /// Handle arrow function type params with multi-line merge.
    /// When type params span multiple lines or are on a different line from `(`,
    /// transform `<` → `(` and absorb the original `(`.
    fn handle_arrow_type_params(
        &mut self,
        type_params: &TSTypeParameterDeclaration<'_>,
        params: &FormalParameters<'_>,
        return_type: Option<&TSTypeAnnotation<'_>>,
    ) {
        let tp_span = type_params.span;
        let p_span = params.span;

        // Check if merge mode is needed: type params span multiple lines
        // or there's a newline between type params and formal params.
        let region = &self.source[tp_span.start as usize..p_span.start as usize];
        let needs_merge = region.contains('\n');

        if needs_merge {
            // Merge mode: <TypeParams>(Params): RetType → (Params)
            // `<` → `(`
            self.set_char(tp_span.start, b'(');
            // Blank type param contents (between < and >) — the > is at tp_span.end - 1
            self.blank(Span::new(tp_span.start + 1, tp_span.end));
            // Blank the `(` of params (absorbed into merged parens).
            // But preserve any comments between `>` and `(`
            self.blank(Span::new(p_span.start, p_span.start + 1));

            // Walk params (blank type annotations inside)
            self.visit_formal_parameters(params);

            if let Some(rt) = return_type {
                // Blank from `)` through end of return type
                self.blank(Span::new(p_span.end - 1, rt.span.end));
                // Place `)` at the last byte of return type
                self.set_char(rt.span.end - 1, b')');
            }
            // If no return type, `)` stays at original position
        } else {
            // Normal mode: just blank <TypeParams>
            self.blank(tp_span);
            self.visit_formal_parameters(params);
            if let Some(rt) = return_type {
                self.blank(rt.span);
            }
        }
    }

    fn is_function_overload(func: &Function<'_>) -> bool {
        func.body.is_none()
    }

}

impl<'a> Visit<'a> for Collector<'_> {
    // ===== Entire statement removal =====

    fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'a>) {
        self.blank_statement(it.span, it.span.start);
    }

    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'a>) {
        self.blank_statement(it.span, it.span.start);
    }

    fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
        if it.declare {
            self.blank_statement(it.span, it.span.start);
        } else {
            self.error(it.span, "enum declarations cannot be erased (convert to const object)");
        }
    }

    fn visit_ts_module_declaration(&mut self, it: &TSModuleDeclaration<'a>) {
        self.blank_statement(it.span, it.span.start);
    }

    fn visit_ts_global_declaration(&mut self, it: &TSGlobalDeclaration<'a>) {
        self.blank_statement(it.span, it.span.start);
    }

    fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
        // Blank entirely — these appear inside namespace blocks which are fully blanked
        self.blank(it.span);
    }

    // ===== Import/Export =====

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        if it.import_kind.is_type() {
            self.blank(it.span);
            return;
        }
        if let Some(specifiers) = &it.specifiers {
            for spec in specifiers {
                if let ImportDeclarationSpecifier::ImportSpecifier(s) = spec {
                    if s.import_kind.is_type() {
                        self.blank_type_specifier_in_list(s.span);
                    }
                }
            }
        }
    }

    fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'a>) {
        // If the exported declaration is a function overload or declare function,
        // blank the entire export statement.
        if let ExportDefaultDeclarationKind::FunctionDeclaration(func) = &it.declaration {
            if func.declare || func.body.is_none() {
                self.blank(it.span);
                return;
            }
        }
        // If it's a TS declaration (interface, type alias), blank entirely
        if let ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) = &it.declaration {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        walk::walk_export_default_declaration(self, it);
    }

    fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
        if it.export_kind.is_type() {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        // Check if declaration is type-only (type alias, interface)
        if let Some(decl) = &it.declaration {
            if decl.is_typescript_syntax() {
                self.blank_statement(it.span, it.span.start);
                return;
            }
        }
        for spec in &it.specifiers {
            if spec.export_kind.is_type() {
                self.blank_type_specifier_in_list(spec.span);
            }
        }
        if let Some(decl) = &it.declaration {
            self.visit_declaration(decl);
        }
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        if it.export_kind.is_type() {
            self.blank(it.span);
        }
    }

    // ===== Functions =====

    fn visit_function(&mut self, it: &Function<'a>, _flags: ScopeFlags) {
        if it.declare {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        // Function overload (no body) — blank without ASI (uses blankExact in ts-blank-space)
        if Collector::is_function_overload(it) {
            self.blank(it.span);
            return;
        }
        if let Some(return_type) = &it.return_type {
            self.blank(return_type.span);
        }
        if let Some(type_params) = &it.type_parameters {
            self.blank(type_params.span);
        }
        if let Some(this_param) = &it.this_param {
            self.blank_this_param(this_param);
        }
        self.visit_formal_parameters(&it.params);
        if let Some(body) = &it.body {
            self.visit_function_body(body);
        }
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        if let Some(type_params) = &it.type_parameters {
            self.handle_arrow_type_params(
                type_params,
                &it.params,
                it.return_type.as_deref(),
            );
        } else {
            if let Some(return_type) = &it.return_type {
                // Check if return type spans to a different line from params
                let region = &self.source[it.params.span.end as usize..return_type.span.end as usize];
                if region.contains('\n') {
                    // Move `)` to end of return type
                    let close_paren = it.params.span.end - 1;
                    self.blank(Span::new(close_paren, return_type.span.end));
                    self.set_char(return_type.span.end - 1, b')');
                } else {
                    self.blank(return_type.span);
                }
            }
            self.visit_formal_parameters(&it.params);
        }
        self.visit_function_body(&it.body);
    }

    fn visit_formal_parameter(&mut self, it: &FormalParameter<'a>) {
        if let Some(type_ann) = &it.type_annotation {
            self.blank(type_ann.span);
        }
        if let Some(acc) = &it.accessibility {
            self.blank_accessibility(acc, it.span.start);
        }
        if it.readonly {
            self.blank_keyword_between("readonly", it.span.start, it.pattern.span().start);
        }
        if it.r#override {
            self.blank_keyword_between("override", it.span.start, it.pattern.span().start);
        }
        if it.optional {
            self.blank_char_after(b'?', it.pattern.span().end, 20);
        }
        self.visit_binding_pattern(&it.pattern);
        if let Some(init) = &it.initializer {
            self.visit_expression(init);
        }
    }

    // ===== Variables =====

    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        if it.declare {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let Some(type_ann) = &it.type_annotation {
            self.blank(type_ann.span);
        }
        if it.definite {
            self.blank_char_after(b'!', it.id.span().end, 5);
        }
        self.visit_binding_pattern(&it.id);
        if let Some(init) = &it.init {
            self.visit_expression(init);
        }
    }

    // ===== Classes =====

    fn visit_class(&mut self, it: &Class<'a>) {
        if it.declare {
            // `declare class` — blank entire thing including decorators
            let start = if it.decorators.is_empty() {
                it.span.start
            } else {
                it.decorators.first().unwrap().span.start
            };
            self.blank_statement(Span::new(start, it.span.end), start);
            return;
        }
        if it.r#abstract {
            self.blank_abstract(it.span.start);
        }
        if let Some(type_params) = &it.type_parameters {
            self.blank(type_params.span);
        }
        self.blank_implements_clause(&it.implements);
        // Walk decorators
        for decorator in &it.decorators {
            self.visit_decorator(decorator);
        }
        if let Some(super_class) = &it.super_class {
            self.visit_expression(super_class);
        }
        if let Some(super_type_params) = &it.super_type_arguments {
            self.blank(super_type_params.span);
        }
        self.visit_class_body(&it.body);
    }

    fn visit_class_body(&mut self, it: &ClassBody<'a>) {
        for elem in &it.body {
            self.visit_class_element(elem);
        }
    }

    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        // `declare` fields are entirely type-only
        if it.declare {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        // Abstract properties — blank entirely
        if it.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        if let Some(type_ann) = &it.type_annotation {
            self.blank(type_ann.span);
        }
        if let Some(acc) = &it.accessibility {
            self.blank_accessibility(acc, it.span.start);
        }
        if it.readonly {
            self.blank_keyword_between("readonly", it.span.start, it.key.span().start);
        }
        if it.r#override {
            self.blank_keyword_between("override", it.span.start, it.key.span().start);
        }
        // For computed properties like `[expr]?`, key span is just `expr` (no brackets).
        // Search for `?`/`!` after the key span, skipping `]`.
        let search_after = if it.computed {
            self.find_char_after(b']', it.key.span().end, 10)
                .unwrap_or(it.key.span().end)
        } else {
            it.key.span().end
        };
        if it.optional {
            self.blank_char_after(b'?', search_after, 10);
        }
        if it.definite {
            self.blank_char_after(b'!', search_after, 10);
        }
        self.visit_property_key(&it.key);
        if let Some(value) = &it.value {
            self.visit_expression(value);
        }
        for decorator in &it.decorators {
            self.visit_decorator(decorator);
        }
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        // Abstract methods — blank entirely
        if it.r#type == MethodDefinitionType::TSAbstractMethodDefinition {
            self.blank_statement(it.span, it.span.start);
            return;
        }
        if let Some(acc) = &it.accessibility {
            self.blank_accessibility(acc, it.span.start);
        }
        if it.r#override {
            self.blank_keyword_between("override", it.span.start, it.key.span().start);
        }
        if it.optional {
            self.blank_char_after(b'?', it.key.span().end, 5);
        }
        self.visit_property_key(&it.key);
        self.visit_function(&it.value, ScopeFlags::empty());
        for decorator in &it.decorators {
            self.visit_decorator(decorator);
        }
    }

    // Index signatures `[key: string]: any;` — blank entirely (no ASI)
    fn visit_ts_index_signature(&mut self, it: &TSIndexSignature<'a>) {
        self.blank(it.span);
    }

    // ===== Type expressions =====

    fn visit_ts_as_expression(&mut self, it: &TSAsExpression<'a>) {
        self.blank_as_satisfies(it.span, &it.expression);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'a>) {
        self.blank_as_satisfies(it.span, &it.expression);
        self.visit_expression(&it.expression);
    }

    fn visit_ts_type_assertion(&mut self, it: &TSTypeAssertion<'a>) {
        let expr_start = it.expression.span().start;
        self.blank(Span::new(it.span.start, expr_start));
        self.visit_expression(&it.expression);
    }

    fn visit_ts_non_null_expression(&mut self, it: &TSNonNullExpression<'a>) {
        let expr_end = it.expression.span().end;
        self.blank(Span::new(expr_end, it.span.end));
        self.visit_expression(&it.expression);
    }

    fn visit_ts_instantiation_expression(&mut self, it: &TSInstantiationExpression<'a>) {
        self.blank(it.type_arguments.span);
        self.visit_expression(&it.expression);
    }

    // ===== Catch-all for type annotations reached via default walk =====

    fn visit_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'a>) {
        self.blank(it.span);
    }

    fn visit_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'a>) {
        self.blank(it.span);
    }

    fn visit_ts_type_parameter_instantiation(&mut self, it: &TSTypeParameterInstantiation<'a>) {
        self.blank(it.span);
    }
}

impl Collector<'_> {
    fn blank_abstract(&mut self, span_start: u32) {
        // The `abstract` keyword may be AT span_start (if the span includes it)
        // or before it. Search in both directions.
        let source = self.source.as_bytes();
        // Check if span starts with "abstract"
        if source[span_start as usize..].starts_with(b"abstract") {
            self.blank(Span::new(span_start, span_start + 8));
            return;
        }
        // Search before
        let search_start = span_start.saturating_sub(50) as usize;
        let region = &self.source[search_start..span_start as usize];
        if let Some(offset) = region.rfind("abstract") {
            let abs_start = search_start + offset;
            self.blank(Span::new(abs_start as u32, abs_start as u32 + 8));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_blank(input: &str, expected: &str) {
        let result = blank_space(input).unwrap();
        assert_eq!(
            result, expected,
            "\n--- input ---\n{input}\n--- expected ---\n{expected}\n--- got ---\n{result}"
        );
    }

    fn assert_error(input: &str) {
        let result = blank_space(input);
        assert!(result.is_err(), "Expected error for: {input}");
    }

    #[test]
    fn test_type_annotation_variable() {
        assert_blank("const x: number = 5;", "const x         = 5;");
    }

    #[test]
    fn test_type_annotation_function_param() {
        assert_blank("function foo(x: number) {}", "function foo(x        ) {}");
    }

    #[test]
    fn test_return_type() {
        assert_blank(
            "function foo(): string { return ''; }",
            "function foo()         { return ''; }",
        );
    }

    #[test]
    fn test_type_alias() {
        assert_blank("type Foo = string;", "                  ");
    }

    #[test]
    fn test_interface() {
        assert_blank(
            "interface Foo { bar: string; }",
            "                              ",
        );
    }

    #[test]
    fn test_import_type() {
        assert_blank(
            "import type { Foo } from 'bar';",
            "                               ",
        );
    }

    #[test]
    fn test_as_expression() {
        assert_blank("const x = y as number;", "const x = y          ;");
    }

    #[test]
    fn test_satisfies_expression() {
        assert_blank("const x = y satisfies Foo;", "const x = y              ;");
    }

    #[test]
    fn test_type_assertion() {
        assert_blank("const x = <number>y;", "const x =         y;");
    }

    #[test]
    fn test_non_null_assertion() {
        assert_blank("const x = y!;", "const x = y ;");
    }

    #[test]
    fn test_generic_function() {
        assert_blank(
            "function foo<T>(x: T): T { return x; }",
            "function foo   (x   )    { return x; }",
        );
    }

    #[test]
    fn test_enum_errors() {
        assert_error("enum Foo { A, B }");
    }

    #[test]
    fn test_declare_enum_ok() {
        assert_blank("declare enum Foo { A, B }", "                         ");
    }

    #[test]
    fn test_multiline_preserves_newlines() {
        let input = "interface Foo {\n  bar: string;\n  baz: number;\n}";
        let expected = "               \n              \n              \n ";
        assert_blank(input, expected);
    }

    #[test]
    fn test_class_implements() {
        assert_blank(
            "class Foo implements Bar {}",
            "class Foo                {}",
        );
    }

    #[test]
    fn test_export_type() {
        assert_blank(
            "export type { Foo } from 'bar';",
            "                               ",
        );
    }

    #[test]
    fn test_instantiation_expression() {
        assert_blank("const x = foo<string>;", "const x = foo        ;");
    }

    #[test]
    fn test_class_type_parameters() {
        assert_blank("class Foo<T> {}", "class Foo    {}");
    }

    #[test]
    fn test_arrow_with_return_type() {
        assert_blank(
            "const f = (x: number): string => '';",
            "const f = (x        )         => '';",
        );
    }

    #[test]
    fn test_declare_function() {
        assert_blank("declare function foo(): void;", "                             ");
    }

    #[test]
    fn test_declare_const() {
        assert_blank("declare const x: number;", "                        ");
    }

    #[test]
    fn test_no_runtime_change() {
        let input = "const x = 5;\nfunction foo(a, b) { return a + b; }\n";
        assert_blank(input, input);
    }

    // ASI tests
    #[test]
    fn test_asi_type_before_paren() {
        assert_blank("foo\ntype x = 1;\n(1);", "foo\n;          \n(1);");
    }

    #[test]
    fn test_asi_interface_before_paren() {
        assert_blank("foo\ninterface I {}\n(1);", "foo\n;             \n(1);");
    }

    #[test]
    fn test_no_asi_after_semicolon() {
        assert_blank("let x;\ninterface I {}\nlet y;", "let x;\n              \nlet y;");
    }

    #[test]
    fn test_asi_as_before_paren() {
        assert_blank(
            "foo as string\n(1);",
            "foo;         \n(1);",
        );
    }

    #[test]
    fn test_no_asi_as_before_plus() {
        assert_blank(
            "foo as string\n+ \"\";",
            "foo          \n+ \"\";",
        );
    }
}

#[cfg(test)]
mod regression {
    use super::*;
    #[test]
    fn test_computed_optional() {
        let src = "const kField = Symbol(\"kField\");\nclass Foo {\n  [kField]?: string;\n  bar!: number;\n}";
        let result = blank_space(src).unwrap();
        for (i, line) in result.lines().enumerate() {
            eprintln!("L{}: {:?}", i+1, line);
        }
        assert!(!result.contains('?'), "? should be blanked: {result}");
        assert!(!result.contains('!'), "! should be blanked: {result}");
    }
}
