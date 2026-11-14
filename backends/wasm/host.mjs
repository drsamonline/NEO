// Minimal host for NOVA WebAssembly modules.
//
// NOVA's WASM target imports two functions from `env`:
//   print_i32(value)      - print an integer followed by a newline
//   print_str(ptr, len)   - print `len` UTF-8 bytes starting at `ptr`
// String literals live in the module's linear memory.
//
// Usage: node host.mjs <module.wasm>
// Exit status is the value returned by `main`, or 0 when it returns nothing.

import fs from "node:fs";

const [file] = process.argv.slice(2);
if (!file) {
  console.error("usage: node host.mjs <module.wasm>");
  process.exit(2);
}

const decoder = new TextDecoder();
let memory = null;

const imports = {
  env: {
    print_i32(value) {
      console.log(value | 0);
    },
    print_str(pointer, length) {
      if (!memory) {
        throw new Error("print_str called before the module was instantiated");
      }
      const bytes = new Uint8Array(memory.buffer, pointer, length);
      console.log(decoder.decode(bytes));
    },
    // Strings whose length is only known at run time (for example a closure
    // result) are NUL-terminated in linear memory.
    print_cstr(pointer) {
      if (!memory) {
        throw new Error("print_cstr called before the module was instantiated");
      }
      const bytes = new Uint8Array(memory.buffer);
      let end = pointer;
      while (end < bytes.length && bytes[end] !== 0) end += 1;
      console.log(decoder.decode(bytes.subarray(pointer, end)));
    },
  },
};

const { instance } = await WebAssembly.instantiate(fs.readFileSync(file), imports);
memory = instance.exports.memory;

if (typeof instance.exports.main !== "function") {
  console.error("module exports no `main` function");
  process.exit(1);
}

const result = instance.exports.main();
process.exit(typeof result === "number" ? result : 0);
