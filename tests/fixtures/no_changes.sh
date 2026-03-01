#!/usr/bin/env bash
set -eu -o pipefail

git init -q

seq 1 10 >> 1.txt
git add 1.txt
git commit -q -m c1

seq 1 10 >> 2.txt
git add 2.txt
git commit -q -m c2
