// Date.now() in a comment is no read
const now = new Date().toISOString();
const t0 = performance.now();
const fixed = new Date(0);
const text = "Date.now()";
export const elapsed = Date.now() - t0;
