// Recursion-depth diagnostic GEOMETRY pin, `check_recursion` site.
// 126 `typeof` levels is N* for this shape, and the token the limit trips
// on is a MULTI-CHARACTER identifier, which is the only way to catch the
// difference between C++ `recursionDepthExceeded`'s `error(tok_->
// getStartLoc(), ...)` (JSParserImpl.cpp:348-352 -> the SMLoc overload at
// JSParserImpl.h:472-474, a bare `^`) and reporting the token's full range
// (`^~~~~`). Every single-character-trip pin renders identically either way.
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof typeof
typeof typeof typeof typeof typeof xyzzy;
