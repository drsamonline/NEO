# JavaScript backend

Emits one `.js` file from SIR. Runs under `node`; CI executes the output for
`examples/hello.nova` and every parity program.

Supports what the parity corpus exercises: integers, strings, arrays and closures.
Where it cannot express a signature it should reject rather than emit something
plausible — see `BACKEND_API.md`.

```bash
nova-compiler emit examples/hello.nova --target js > hello.js && node hello.js
```
