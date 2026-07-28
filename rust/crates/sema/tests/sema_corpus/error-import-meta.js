// "'import.meta' is currently unsupported" (cpp:860-866), reported only when
// compile_ is set — which it is for `hermesc -dump-sema`.
import.meta;
function f() {
  return import.meta;
}
