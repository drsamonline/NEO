# `io`

60 functions over files and streams: `fopen`, `fclose`, `read_file`, `write_file`,
`append_file`, `fread_all`, and buffered readers.

**49 of its 59 functions can run.** The ten that cannot are not a backlog of the same
kind of work: `serialize`/`deserialize` need a decision about the format, `args` needs the
emitted `main` to take `argc`/`argv`, and the rest are the stream types.

Everything that touches a file works: opening, reading, writing, seeking, closing, the
path helpers, and `stat`. `stat` composes its `FileMetadata` in NOVA from scalar builtins
rather than asking the runtime for the struct, because the runtime does not know a layout
the language defines.

That is not a bug to fix by deleting it: the module documents what file handling
should look like. It is a claim not to make, which is why
[`../README.md`](../README.md) carries the numbers and `tools/stdlib-check.py` keeps
them true.
