// FLAGS: -fno-std-globals
// Pins two things at once: `-fno-std-globals` gating the libhermes
// ambient-decl load off (no 63 UndeclaredGlobalProperty decls in the dump)
// AND that an identifier with no local declaration and no ambient decl
// (`print`, normally one of the 63) still resolves as an
// UndeclaredGlobalProperty created on the fly, per
// `validateAndDeclareIdentifier`'s "not declared, no ambient decl either"
// path.
var x;
print;
