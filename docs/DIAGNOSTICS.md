# Diagnostics

Diagnostics contain a stage-specific message, a byte span, and a source label.
The CLI renders the source name, one-based line and column, the containing line,
and a caret underline. Spans use UTF-8 byte offsets internally and rendering
computes character columns, so a preceding multibyte character does not corrupt
the displayed column.

