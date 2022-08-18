# mdbook-trace

![https://crates.io/crates/mdbook-trace](https://img.shields.io/crates/v/mdbook-trace)

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
# The trace numbering strategy for a page with subchapters.
# If "allow-duplicates", number traces as normal.
# This will result in traces with the same number as subchapters.
# (e.g. the first trace and first subchapter of chapter 1 will both be numbered 1.1)
# If "offset", offset trace numbers from the last subchapter.
# (e.g. if chapter 1 has 2 subchapters, the first trace will be numbered 1.3)
# If "zero", insert a ".0" qualifier before traces in a page with subchapters.
# (e.g. if chapter 1 has 1 subchapter, the first trace will be 1.0.1).
parent-numbering = "zero"
```
