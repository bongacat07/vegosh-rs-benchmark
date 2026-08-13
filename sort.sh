#!/bin/bash

mkdir -p fuzz_result/fuzz_48
mkdir -p fuzz_result/fuzz_75
mkdir -p fuzz_result/fuzz_87

for file in fuzz_48_*.csv; do
    [ -e "$file" ] && mv "$file" fuzz_result/fuzz_48/
done

for file in fuzz_75_*.csv; do
    [ -e "$file" ] && mv "$file" fuzz_result/fuzz_75/
done

for file in fuzz_87_*.csv; do
    [ -e "$file" ] && mv "$file" fuzz_result/fuzz_87/
done
