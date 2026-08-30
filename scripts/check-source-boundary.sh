#!/bin/sh
set -eu

cargo_bin=${LENSO_CARGO_BIN:-cargo}
metadata_json=$($cargo_bin metadata --locked --format-version 1)

check_one_source() {
  package_name=$1
  expected_revision=$2
  matching_sources=$(printf '%s' "$metadata_json" | jq -r --arg package_name "$package_name" \
    '.packages[] | select(.name == $package_name) | (.source // "path")' | sort -u)
  source_count=$(printf '%s\n' "$matching_sources" | awk 'NF { count += 1 } END { print count + 0 }')
  if [ "$source_count" -ne 1 ]; then
    printf '%s resolved from %s sources\n' "$package_name" "$source_count" >&2
    exit 1
  fi
  case "$matching_sources" in
    *"$expected_revision"*) ;;
    *)
      printf '%s did not resolve from revision %s\n' "$package_name" "$expected_revision" >&2
      exit 1
      ;;
  esac
  printf '%s\t%s\n' "$package_name" "$matching_sources"
}

check_one_source lenso 89815107385475c8b5be378bdcf5e21aa74e02f0
check_one_source lenso-native-adapter 89815107385475c8b5be378bdcf5e21aa74e02f0
check_one_source lenso-app-plan 8599db7e4a214ed92f32089f81d14c833d4becf6
check_one_source lenso-kernel 8599db7e4a214ed92f32089f81d14c833d4becf6
check_one_source lenso-contract-codegen 9d2774c767fd64a6cc63a5ac04360e9cb92816da
check_one_source lenso-contract-runtime 9d2774c767fd64a6cc63a5ac04360e9cb92816da
check_one_source lenso-plugin-authoring 9d2774c767fd64a6cc63a5ac04360e9cb92816da
