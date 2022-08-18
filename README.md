# mdbook-trace

A traceable document preprocessor for mdbook.

`mdbook-trace` provides a markdown extension to define a *trace* from the current document to a record in some external document. Each trace generates a footnote in the current page and is added to a *trace table* for the target document.

## Installation
```sh
cargo install mdbook-trace
```

## Usage

Add an entry to `book.toml` for each target document you'll be tracing to:

```toml
[preprocessor.trace.targets.mydoc]
name = "My Document"
```

Then, in your markdown add a new trace:
```markdown
Some traceable text {{#trace mydoc:ID-1.2 }}
```

This will be rendered as

> Some traceable text[^1]
>
> ---
>
> [^1]: My Document ID-1.2

Finally, update your markdown to generate a trace table for the target:
```markdown
# My Document
{{#tracematrix mydoc }}
```

This will be rendered as

> | Record | Traces |
> |--------|--------|
> | ID-1.2 | [1.1]() |


## Configuration
Below is the full set of preprocessor configuration options and their default values:
```toml
[preprocessor.trace]
# If "offset", traces in a page with child pages are numbered starting from the last child page.
# e.g. a reference in page 1 with 2 child pages would be numbered 1.3
# If "allow-duplicates", traces in a page are always numbered starting from 1. This may result in a trace having the same number as a child page.
parent-numbering = "offset"
# Use fully-qualified trace number as in-page footnote number
# e.g. "Some text^1" becomes "Some text^1.2.1"
qualified-footnotes = false
# Add chapter numbers to page titles
# e.g. "My Chapter" becomes "1.1 My Chapter"
chapter-numbers = false
# Insert a horizontal rule between the body of a page and the list of generated footnotes.
footnote-divider = false
# Heading used for the first column of generated trace tables.
record-heading = "Record"
# Heading used for the second column of generated trace tables.
trace-heading = "Traces"
```
