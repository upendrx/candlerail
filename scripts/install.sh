#!/bin/sh
# Installs the latest candlerail release on macOS or Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/upendrx/candlerail/main/scripts/install.sh | sh
#
# Environment:
#   CANDLERAIL_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   CANDLERAIL_VERSION      a release tag such as v0.2.0 (default: latest)
set -eu

repo="upendrx/candlerail"
dir="${CANDLERAIL_INSTALL_DIR:-$HOME/.local/bin}"
version="${CANDLERAIL_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

case "$(uname -s)" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *) fail "unsupported system $(uname -s); on Windows use scripts/install.ps1" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *) fail "unsupported processor $(uname -m); build from source instead (see docs/getting-started.md)" ;;
esac
target="$arch-$os"

if [ "$version" = "latest" ]; then
  url="https://github.com/$repo/releases/latest/download/candlerail-$target.tar.gz"
else
  url="https://github.com/$repo/releases/download/$version/candlerail-$target.tar.gz"
fi

command -v curl > /dev/null 2>&1 || fail "curl is required"
command -v tar > /dev/null 2>&1 || fail "tar is required"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "Downloading candlerail ($target)..."
curl -fsSL "$url" -o "$tmp/candlerail.tar.gz" || fail "download failed: $url"
tar xzf "$tmp/candlerail.tar.gz" -C "$tmp"

mkdir -p "$dir"
install -m 755 "$tmp/candlerail-$target/candlerail" "$dir/candlerail"
say "Installed $("$dir/candlerail" --version) to $dir/candlerail"

case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    say ""
    say "$dir is not on your PATH. Add it by running:"
    say "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.profile && . ~/.profile"
    ;;
esac

say ""
say "Start it with:"
say "  candlerail serve"
say "then open http://127.0.0.1:8787 in your browser."
