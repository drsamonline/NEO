# `io`

60 functions over files and streams: `fopen`, `fclose`, `read_file`, `write_file`,
`append_file`, `fread_all`, and buffered readers.

They lower to `__builtin_*` calls. On the C target the ones that are not implemented
trap through `nova_unsupported` rather than silently doing nothing — the module
declares more than the runtime shim provides, and `DEV_PLAN.md` says which.
