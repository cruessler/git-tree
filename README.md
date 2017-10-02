# git-tree

git-tree is a small command line utility for showing the status of untracked
and modified files in a git repository as a tree.

## Installation

Because it uses `#![feature(advanced_slice_patterns, slice_patterns)]`, it
currently (2017-10) requires a nightly compiler. See
https://github.com/rust-lang/rust/issues/23121 and
https://doc.rust-lang.org/1.8.0/book/slice-patterns.html.

Provided you have `cargo` installed, installation is as easy as

```
cargo install --git https://github.com/cruessler/git-tree
```

This will download the source code and compile the binary which can then be
found in `~/.cargo/bin`. If that’s in your `$PATH`, you can type `git-tree
--help` to get an overview of the available commands.
