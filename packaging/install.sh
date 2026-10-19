#!/usr/bin/env sh
# Builds Samādhi and installs it for the current user (no root needed):
#   binary  -> ~/.local/bin/samadhi
#   icons   -> ~/.local/share/icons/hicolor/<size>/apps/samadhi.png
#   launcher -> ~/.local/share/applications
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
bin=$HOME/.local/bin

cargo build --release --manifest-path "$root/Cargo.toml"

install -Dm755 "$root/target/release/samadhi" "$bin/samadhi"
for dir in "$root"/assets/icon/hicolor/*/; do
    size=$(basename "$dir")
    install -Dm644 "$dir/apps/samadhi.png" "$data/icons/hicolor/$size/apps/samadhi.png"
done
# Remove the earlier vector icon, which would otherwise take precedence.
rm -f "$data/icons/hicolor/scalable/apps/samadhi.svg"
# Point the launcher at the icon file directly: some launchers (e.g. COSMIC)
# do not search a per-user icon theme that lacks its own index.theme.
install -d "$data/applications"
sed "s|^Icon=.*|Icon=$data/icons/hicolor/256x256/apps/samadhi.png|" \
    "$root/packaging/samadhi.desktop" > "$data/applications/samadhi.desktop"

# Refresh caches when the tools are available; harmless if they are not.
update-desktop-database "$data/applications" 2>/dev/null || true
# A cache in the per-user theme would shadow icons it does not list.
rm -f "$data/icons/hicolor/icon-theme.cache"

echo "Installed. Launch Samādhi from your app menu, or run: samadhi [file.md]"
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "Note: $bin is not on your PATH." ;;
esac
