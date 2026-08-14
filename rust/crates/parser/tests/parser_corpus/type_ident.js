// Without -parse-flow, `type` is a plain identifier; this locks the no-leak
// guarantee that the Flow declaration gate stays off by default.
var type = 1;
type;
type
X;
