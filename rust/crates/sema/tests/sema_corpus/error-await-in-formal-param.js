// "'await' not allowed in a formal parameter" (cpp:1512-1519, ES14.0 15.8.1).
async function f(x = await 1) {}
async function g(a, b = await a) {}
