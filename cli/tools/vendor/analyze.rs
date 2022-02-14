use deno_ast::swc::ast::Program;
use deno_ast::swc::visit::noop_visit_type;
use deno_ast::swc::visit::Visit;
use deno_ast::swc::visit::VisitWith;
use deno_ast::ParsedSource;

/// Gets if the parsed source has a default export.
pub fn has_default_export(source: &ParsedSource) -> bool {
  let mut visitor = DefaultExportFinder {
    has_default_export: false,
  };
  let program = source.program();
  let program: &Program = &program;
  program.visit_with(&mut visitor);
  visitor.has_default_export
}

struct DefaultExportFinder {
  has_default_export: bool,
}

impl<'a> Visit for DefaultExportFinder {
  noop_visit_type!();

  fn visit_export_default_decl(
    &mut self,
    _: &deno_ast::swc::ast::ExportDefaultDecl,
  ) {
    self.has_default_export = true;
  }
}
