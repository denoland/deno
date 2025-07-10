// Copyright 2018-2025 the Deno authors. MIT license.

use deno_resolver::cjs::analyzer::DenoCjsCodeAnalyzer;
use deno_resolver::npm::DenoInNpmPackageChecker;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::analyze::CjsModuleExportAnalyzer;

use crate::npm::CliNpmResolver;
use crate::sys::CliSys;

pub type CliCjsCodeAnalyzer = DenoCjsCodeAnalyzer<CliSys>;

pub type CliCjsModuleExportAnalyzer = CjsModuleExportAnalyzer<
  CliCjsCodeAnalyzer,
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliNodeResolver = deno_runtime::deno_node::NodeResolver<
  DenoInNpmPackageChecker,
  CliNpmResolver,
  CliSys,
>;
pub type CliPackageJsonResolver = node_resolver::PackageJsonResolver<CliSys>;
