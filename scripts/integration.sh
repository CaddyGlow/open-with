#!/usr/bin/env bash
bin=./target/release/openit
folder=$(mktemp -d)
fn=$folder/test
echo "Hello World" >$fn.txt
echo '{"test": true}' >$fn.json
$bin $fn.txt --json | jq '.mimetype' | grep -q "text/plain"
$bin $fn.json --json | jq '.mimetype' | grep -q "application/json"
