#!/bin/sh
# Open the source-linked project in an installed editor.
atlas_project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 1
if command -v rust-vn-editor-launch >/dev/null 2>&1; then
    exec rust-vn-editor-launch --graph "$atlas_project/main.rvn"
fi
if command -v rust-vn-editor >/dev/null 2>&1; then
    exec rust-vn-editor --graph "$atlas_project/main.rvn"
fi
atlas_data_dir=${XDG_DATA_HOME:-${HOME:?}/.local/share}
atlas_runtime="$atlas_data_dir/rust-vn-editor/current"
if [ ! -x "$atlas_runtime/rust-vn-editor" ]; then
    printf '%s\n' 'Éditeur rust-VN introuvable. Installez l’application et ouvrez ce dossier comme projet.' >&2
    exit 1
fi
export LD_LIBRARY_PATH="$atlas_runtime/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
atlas_editor="$atlas_runtime/rust-vn-editor"
if [ -x "$atlas_runtime/rust-vn-editor-launch" ]; then
    atlas_editor="$atlas_runtime/rust-vn-editor-launch"
fi
exec "$atlas_editor" --graph "$atlas_project/main.rvn"
