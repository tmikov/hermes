// visit(ImportDeclarationNode *) (cpp:874-890). The "'import' statement
// requires module mode" error is NOT compile_-gated (cpp:876-879), unlike
// all three export errors — a bug-for-bug asymmetry the port preserves.
// hermesc has no -commonjs here, so getUseCJSModules() is false and the
// error fires; the post-walk gate then suppresses the dump (exit 2).
import {a} from 'm';
