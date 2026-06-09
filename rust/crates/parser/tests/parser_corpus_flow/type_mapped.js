type A = { [K in T]: V };
type B = { +[K in T]?: V };
type C = { [K in T]+?: V };
type D = { [K in T]-?: V };
type E = { [K in T]?: V };
