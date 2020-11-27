#!/bin/bash
set -e
cd "`dirname $0`"
wasm-pack build
mkdir -p release
cp pkg/contract_samples_token_bank_bg.wasm ./release/
