// Flow type-arguments on a JSX opening tag (jsx.cpp:124-132). This is the only
// JSX production not otherwise exercised by the standalone `-parse-jsx` corpus:
// it requires the leading `<TypeArgs>` after the tag name, parsed via
// parseTypeArgsFlow. Differential runs BOTH `-parse-jsx` and `-parse-flow`.
var x = <Foo<T> />;
var y = <C<A, B>>child</C>;
