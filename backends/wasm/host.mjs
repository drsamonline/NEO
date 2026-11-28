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

// `printf("%g")` for a double: six significant digits, scientific notation when the exponent is
// below -4 or at least 6, and trailing zeros removed. Written out because the two obvious
// alternatives are both wrong: `String(x)` never uses `%g`'s exponent rule, and `toPrecision(6)`
// leaves the zeros `%g` strips.
function formatG(value) {
  if (Number.isNaN(value)) return "nan";
  if (!Number.isFinite(value)) return value > 0 ? "inf" : "-inf";
  if (value === 0) return "0";

  const exponent = Math.floor(Math.log10(Math.abs(value)));
  if (exponent < -4 || exponent >= 6) {
    // `toExponential(5)` gives the mantissa with `%g`'s five decimals; `%e` pads the exponent to
    // two digits and always carries a sign.
    const [mantissa, power] = value.toExponential(5).split("e");
    const stripped = mantissa.includes(".") ? mantissa.replace(/\.?0+$/, "") : mantissa;
    const sign = power.startsWith("-") ? "-" : "+";
    const digits = Math.abs(Number(power)).toString().padStart(2, "0");
    return `${stripped}e${sign}${digits}`;
  }

  // Fixed notation with the decimals `%g` would keep, then the same stripping.
  const decimals = Math.max(0, 5 - exponent);
  const fixed = value.toFixed(decimals);
  return fixed.includes(".") ? fixed.replace(/\.?0+$/, "") : fixed;
}

const imports = {
  env: {
    print_i32(value) {
      console.log(value | 0);
    },
    // A float, formatted the way the C runtime formats one: `printf("%g")`, six significant
    // digits with trailing zeros removed. `String(value)` is not the same function — it prints
    // `0.00001` where C prints `1e-05` — and a backend whose output differs from C's is not a
    // backend that agrees with it.
    print_f64(value) {
      console.log(formatG(value));
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
