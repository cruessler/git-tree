#!/usr/bin/env bash
set -eu -o pipefail

git init -q

seq 1 10 >> 1.txt
git add 1.txt
git commit -q -m c1

seq 1 10 >> 2.txt
git add 2.txt
git commit -q -m c2

mkdir -p a/b/c
seq 1 10 >> a/b/c/3.txt
git add a/b/c/3.txt
git commit -q -m c3

seq 1 10 >> 4.txt
seq 1 10 >> a/b/c/5.txt

git rm 1.txt
rm 2.txt
# When, in a sub-directory, all tracked files are removed via `git rm`, the
# topmost parent folder of those removed files is treated as untracked by `git
# status`, `gix status` and the `gitoxide`-based version of `git-tree`, but not
# by the `git2`-based version of `git-tree`.
git rm a/b/c/3.txt

# When there are other untracked files in a file in which all tracked files
# have been removed, the topmost parent folder is not shown as untracked.
mkdir -p e/f
seq 1 10 >> e/f/4.txt
git add e/f/4.txt
seq 1 10 >> e/f/6.txt
git add e/f/6.txt
git commit -q -m c4

seq 1 10 >> e/f/7.txt
git rm e/f/4.txt
