// A never-loaded file whose only function is an arrow assigned to a const:
// pins the AST function synthesis in UncoveredCollector::visit_var_declarator.
export const double = (x: number): number => x * 2;
