// Date.now() in a comment is no read
const now = new Date().toISOString();
const t0 = performance.now();
const fixed = new Date(0);
const text = "Date.now()";
export const elapsed = Date.now() - t0;
const stamp = `run-${Date.now()}`;
const asText = Date();
const bare = new Date;
const clock = Date.now;
const global = new globalThis.Date();
const started = process.hrtime.bigint();
const plain = `Date.now() ${stamp}`;
