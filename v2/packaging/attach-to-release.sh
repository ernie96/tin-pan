#!/bin/sh
# Signs a macOS or Linux build and adds it to the GitHub release that release-v2.yml makes for the same tag.
# That job builds on Windows and creates the release, so this waits for it (up to 40 minutes) before adding
# the file. Run from the repository root: v2/packaging/attach-to-release.sh v2/target/release/TinPan.dmg
# Needs GH_TOKEN, UPDATE_SIGNING_KEY and GITHUB_REF_NAME.
set -eu
file=$1
tag=$GITHUB_REF_NAME

version=$(sed -n 's/^version = "\(.*\)"/\1/p' v2/Cargo.toml | head -n 1)
[ "v$version" = "$tag" ] || { echo "Tag $tag does not match version $version in v2/Cargo.toml."; exit 1; }
[ -n "${UPDATE_SIGNING_KEY:-}" ] || { echo 'The UPDATE_SIGNING_KEY secret is missing; an unsigned build could not be installed by the updater.'; exit 1; }

# The same key and scheme as TinPan.exe, checked against the key built into the app.
dotnet run --project build/Signer -c Release -- sign "$file"
public_key=$(sed -n 's/.*PUBLIC_KEY: &str = "\([^"]*\)".*/\1/p' v2/crates/tinpan-core/src/update.rs)
dotnet run --project build/Signer -c Release -- verify "$public_key" "$file"

tries=0
until gh release view "$tag" >/dev/null 2>&1; do
    tries=$((tries + 1))
    [ "$tries" -le 80 ] || { echo "No release for $tag after 40 minutes; did Release 2.x fail?"; exit 1; }
    echo "Waiting for the $tag release..."
    sleep 30
done
gh release upload "$tag" "$file" "$file.sig" --clobber
echo "Added $(basename "$file") to $tag"
