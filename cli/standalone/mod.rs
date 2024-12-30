// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

pub mod binary;
pub mod serialization;
pub mod sys;
pub mod virtual_fs;

pub const MODULE_NOT_FOUND: &str = "Module not found";
pub const UNSUPPORTED_SCHEME: &str = "Unsupported scheme";
