// "async generators are unsupported" (cpp:1719-1725): hermesc leaves
// -Xasync-generators at its default false, which is what the port's
// ENABLE_ASYNC_GENERATORS constant records.
async function* g() {
  yield 1;
}
var h = async function* () {
  yield 2;
};
