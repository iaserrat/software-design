---
type: llm
---

PASS if the reply answers no (or not reliably) and names at least two specific gaps in the interface of Buffer.copy, such as what the return value means, what happens when the range extends past the end of the buffer, whether `dest` must have room for `length` bytes, or the units of `offset`.
FAIL if it says the API is clear enough, or lists only generic advice without naming gaps in this method.
