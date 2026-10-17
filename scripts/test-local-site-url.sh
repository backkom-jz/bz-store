#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

printf '%s\n' 'SITE_URL=http://bz-store.local' > "$tmp/stale.env"
if bash "$root/scripts/check-local-site-url.sh" "$tmp/stale.env" > "$tmp/stale.out" 2>&1; then
  echo 'expected old SITE_URL to be rejected' >&2
  exit 1
fi
grep -Fq 'SITE_URL 必须设为 http://bz-store.local:8088' "$tmp/stale.out"

printf '%s\n' 'SITE_URL=http://bz-store.local:8088' > "$tmp/current.env"
bash "$root/scripts/check-local-site-url.sh" "$tmp/current.env"

# Exercise the actual startup script without contacting Docker or cloud services.
mkdir -p "$tmp/startup/scripts" "$tmp/startup/bin"
cp "$root/scripts/dev.sh" "$root/scripts/check-local-site-url.sh" "$tmp/startup/scripts/"
cp "$tmp/stale.env" "$tmp/startup/.env"
cat > "$tmp/startup/bin/docker" <<'SH'
#!/usr/bin/env bash
touch "$DEVSTORE_TEST_DOCKER_MARKER"
exit 93
SH
chmod +x "$tmp/startup/bin/docker"
if PATH="$tmp/startup/bin:$PATH" DEVSTORE_TEST_DOCKER_MARKER="$tmp/docker-called" \
  bash "$tmp/startup/scripts/dev.sh" > "$tmp/startup.out" 2>&1; then
  echo 'expected dev.sh to reject stale SITE_URL before startup' >&2
  exit 1
fi
grep -Fq 'SITE_URL 必须设为 http://bz-store.local:8088' "$tmp/startup.out"
if [[ -e "$tmp/docker-called" ]]; then
  echo 'dev.sh must validate SITE_URL before calling Docker' >&2
  exit 1
fi

echo 'local site URL checks passed'
