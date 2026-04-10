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

seq 1 10 >> a/b/c/4.txt
git add a/b/c/4.txt
git commit -q -m c4

seq 3 5 >> 1.txt
git add 1.txt

seq 3 6 >> 2.txt

seq 3 7 >> a/b/c/3.txt
git add a/b/c/3.txt

seq 3 8 >> a/b/c/4.txt

seq 1 5 >> 5.txt
git add 5.txt
seq 6 8 >> 5.txt
