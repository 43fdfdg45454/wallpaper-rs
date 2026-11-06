#!/usr/bin/env bash
set -euo pipefail
declare site
(
  cd "$site"
  ctl repo keygen old --no-password >/dev/null
  ctl repo keygen new --no-password >/dev/null
  ctl repo init turn --name Turn --root-key root.pub --online-key online.pub --author studio=old.pub \
    --mirror https://mirror.example.org --root-days 90 >/dev/null 2>&1
  ctl repo sign-root turn --key root.key >/dev/null
  ctl repo rotate-root turn --author studio=new.pub >/dev/null 2>&1
  ctl repo sign-root turn --key root.key >/dev/null
  count="[len(a['keys']) for a in r['authors']], r['mirrors']" roots=turn/root
  [ "$(jq_ "$count" < $roots/2.json)" = "([1], ['https://mirror.example.org'])" ] ||
    fail "the new author key did not replace the old one: $(cat $roots/2.json)"
  [ "$(jq_ "r['root_days']" < turn/catalog.json)" = 90 ] || fail "the root forgot its lifetime"
  ctl repo rotate-root turn --author studio=new.pub --no-mirrors >/dev/null 2>&1 ||
    fail "restating the key of an author was refused"
  [ "$(jq_ "$count" < $roots/3.json)" = "([1], [])" ] || fail "the mirrors were not dropped: $(cat $roots/3.json)"
)
echo "root rotation checks passed"
