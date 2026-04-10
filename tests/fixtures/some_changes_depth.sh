#!/usr/bin/env bash
set -eu -o pipefail

git init -q first
(cd first
  seq 1 10 >> 1.txt
  git add 1.txt
  git commit -q -m c1

  seq 1 10 >> 2.txt
  git add 2.txt
  git commit -q -m c2

  seq 3 7 >> 1.txt
  seq 3 6 >> 2.txt
)

git init -q second
(cd second
  seq 1 10 >> 1.txt
  git add 1.txt
  git commit -q -m c1

  seq 1 10 >> 2.txt
  git add 2.txt
  git commit -q -m c2

  seq 3 7 >> 1.txt
  seq 3 6 >> 2.txt
)
