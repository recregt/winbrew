#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 4 ]; then
	echo "usage: $0 <catalog-db> <metadata-json> <generated-dir> <archive-dir>" >&2
	exit 2
fi

catalog_db_path="$1"
metadata_path="$2"
generated_dir="$3"
archive_dir="$4"

require_file() {
	local path="$1"
	local label="$2"

	if [ ! -s "$path" ]; then
		echo "::error::$label is missing or empty: $path" >&2
		exit 1
	fi
}

require_file "$catalog_db_path" "catalog database"
require_file "$metadata_path" "catalog metadata"
require_file "$generated_dir/release_materialization.sql" "release materialization SQL"
require_file "$generated_dir/update_plans.sql" "update plans SQL"
require_file "$generated_dir/wrangler.production.jsonc" "wrangler config"

rm -rf "$archive_dir"
mkdir -p "$archive_dir"

cp "$catalog_db_path" "$archive_dir/catalog.db"
zstd --quiet --force --keep -19 "$catalog_db_path" -o "$archive_dir/catalog.db.zst"
cp "$metadata_path" "$archive_dir/metadata.json"

infra_staging_dir="$(mktemp -d)"
cleanup() {
	rm -rf "$infra_staging_dir"
}
trap cleanup EXIT

cp "$generated_dir/release_materialization.sql" "$infra_staging_dir/release_materialization.sql"
cp "$generated_dir/update_plans.sql" "$infra_staging_dir/update_plans.sql"
cp "$generated_dir/wrangler.production.jsonc" "$infra_staging_dir/wrangler.production.jsonc"

if [ -s "$generated_dir/patch_chain.json" ] && ! grep -Eq '^[[:space:]]*\[[[:space:]]*\][[:space:]]*$' "$generated_dir/patch_chain.json"; then
	cp "$generated_dir/patch_chain.json" "$infra_staging_dir/patch_chain.json"
else
	echo "patch chain is empty; skipping archive asset"
fi

tar -C "$infra_staging_dir" -czf "$archive_dir/infra.tar.gz" .