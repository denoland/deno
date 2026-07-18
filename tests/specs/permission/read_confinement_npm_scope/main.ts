// Importing the package grants read on its own folder in the global npm cache.
// The package (read-scope-self) reads its own bundled data file (allowed) and a
// different cached package's file (denied), proving the grant is scoped to the
// packages this program actually imports.
import "npm:@denotest/read-scope-self";
