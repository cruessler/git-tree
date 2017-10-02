# git-tree

git-tree is a small command line utility for showing the status of untracked
and modified files in a git repository as a tree.

## Installation

Because it uses `#![feature(advanced_slice_patterns, slice_patterns)]`, it
currently (2017-10) requires a nightly compiler. See
https://github.com/rust-lang/rust/issues/23121 and
https://doc.rust-lang.org/1.8.0/book/slice-patterns.html.
You can install Rust nightly with

```bash
curl -s https://static.rust-lang.org/rustup.sh | sh -s -- --channel=nightly
```
or, alternatively, update an existing installation with

```
rustup update nightly
```

Provided you have `cargo` installed, installation is as easy as

```
cargo install https://github.com/cruessler/git-tree
# or, if you are not using nightly as default
rustup run nightly cargo install --git https://www.github.com/cruessler/git-tree 
```

This will download the source code and compile the binary which can then be
found in `~/.cargo/bin`. If that’s in your `$PATH`, you can type `git-tree
--help` to get an overview of the available commands.
