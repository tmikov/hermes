// visit(ImportDeclarationNode *) (cpp:874-890) under the compile = false
// entry point: the error still fires (it is not compile_-gated), and the
// parser-entry tool dumps anyway — so this file pins what the driver corpus
// cannot see, the Decl::Kind::Import decls that extractIdentsFromDecl's
// ImportDeclaration arm (cpp:2348-2361) makes for all three specifier
// shapes: `d` (ImportDefaultSpecifier), `b` (ImportSpecifier's _local) and
// `ns` (ImportNamespaceSpecifier). It also pins that the specifier children
// walk reaches an ImportSpecifier's _imported: `a` resolves as an ordinary
// UndeclaredGlobalProperty next to `b`'s Import decl.
import d, {a as b} from 'm';
import * as ns from 'n';
