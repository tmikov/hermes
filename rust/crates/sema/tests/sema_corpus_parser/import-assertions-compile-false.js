// The FALSE side of the compile_ gate on the import-assertions error
// (SemanticResolver.cpp:882-885: `if (compile_ && !importDecl->_attributes.
// empty())`). Under this pair's compile = false entry point the attribute
// list is non-empty and the "import assertions are not supported" error is
// nevertheless NOT emitted -- the only diagnostic is the ungated module-mode
// one from cpp:876-880. A port that dropped the compile_ half of that
// condition would still pass module-imports.js (no attributes there), so
// this file is what makes the gate itself observable.
import 'b.js' with {type:'json'};
