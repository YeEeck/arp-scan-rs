#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_tag="arp-scan-rs/windows-gnu:latest"
artifact_dir="$repo_root/dist/windows-x64"
artifact_path="$artifact_dir/arp-scan-rs.exe"
temp_artifact_path="$artifact_path.tmp"
container_id=""

cleanup() {
    if [[ -n "$container_id" ]]; then
        docker rm -f "$container_id" >/dev/null 2>&1 || true
    fi

    rm -f "$temp_artifact_path"
}

trap cleanup EXIT

mkdir -p "$artifact_dir"
rm -f "$temp_artifact_path"

docker build \
    -f "$repo_root/docker/windows-gnu/Dockerfile" \
    -t "$image_tag" \
    "$repo_root"

container_id="$(docker create "$image_tag")"
docker cp "$container_id:/out/arp-scan-rs.exe" "$temp_artifact_path"
mv "$temp_artifact_path" "$artifact_path"

echo "Built Windows executable: $artifact_path"
