// An arrow's `arguments` resolves to the nearest non-arrow ancestor's
// `arguments` declaration (SemContext::funcArgumentsDecl), and the enclosing
// function records containsArrowFunctions/
// containsArrowFunctionsUsingArguments (cpp:270-274) — the two flags are
// invisible to -dump-sema, so tests/resolver.rs pins those separately.
function outer() {
  var a = () => arguments;
  var b = () => () => arguments;
  return a;
}
// At global scope `arguments` is just a global property.
var top = () => arguments;
