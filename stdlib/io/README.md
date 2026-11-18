# `io`

60 functions over files and streams: `fopen`, `fclose`, `read_file`, `write_file`,
`append_file`, `fread_all`, and buffered readers.

**18 of its 60 functions can run.** The other 42 call a builtin the compiler does not
implement — `__builtin_fopen`, `__builtin_fread_line`, `__builtin_stat`,
`__builtin_mkdir` and the rest — so most of this module cannot be called at all.
`parse_int`, `parse_float` and `parse_bool` are among the three that now work.

That is not a bug to fix by deleting it: the module documents what file handling
should look like. It is a claim not to make, which is why
[`../README.md`](../README.md) carries the numbers and `tools/stdlib-check.py` keeps
them true.
