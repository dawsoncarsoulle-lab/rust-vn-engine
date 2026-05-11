# RVN Zed Extension

Language support for RVN visual novel scripts in Zed.

## Features

- Associates `.rvn` files with the `RVN` language.
- Uses `tree-sitter-rvn` for parsing.
- Highlights RVN keywords, labels, dialogue speakers, strings, comments,
  numbers, booleans, properties, operators, and method-style calls.
- Provides line comments, bracket matching, auto-closing pairs, indentation, and
  a basic outline.
- Includes small snippets for labels, choices, conditionals, and dialogue.

This MVP deliberately does not include a language server. Diagnostics, goto
definition, document symbols from the Rust parser, and completion should be
added later through `rvn_lsp` or a thin wrapper around `rvn check`.

## Local Installation

1. Generate the Tree-sitter parser:

   ```sh
   cd ../tree-sitter-rvn
   npm install
   npm run generate
   ```

2. In Zed, run `zed: install dev extension`.
3. Select this `rvn-zed-extension` directory.
4. Open a `.rvn` file and verify that Zed selects the `RVN` language.

The local development manifest points to the sibling grammar with a `file://`
URL. Zed still treats this as a Git grammar source, so `tree-sitter-rvn` must be
a standalone Git repository and `[grammars.rvn].rev` must point to a commit in
that repository.

After changing the grammar:

```sh
cd ../tree-sitter-rvn
npm run generate
npm test
git add .
git commit -m "Update RVN grammar"
git rev-parse HEAD
```

Then copy the new SHA into `[grammars.rvn].rev` in `extension.toml`.

If the repository is moved, update `[grammars.rvn].repository` in
`extension.toml`, or publish `tree-sitter-rvn` separately and pin a remote
commit hash.
