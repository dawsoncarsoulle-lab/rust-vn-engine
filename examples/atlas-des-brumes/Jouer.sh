#!/bin/sh
# Use a runtime from PATH, or the standard per-user editor installation.
atlas_project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 1
if command -v rvn >/dev/null 2>&1; then
    exec rvn run "$atlas_project"
fi
atlas_data_dir=${XDG_DATA_HOME:-${HOME:?}/.local/share}
atlas_runtime="$atlas_data_dir/rust-vn-editor/current"
if [ -x "$atlas_runtime/rvn" ]; then
    export LD_LIBRARY_PATH="$atlas_runtime/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    exec "$atlas_runtime/rvn" run "$atlas_project"
fi
printf '%s\n' 'Moteur rust-VN introuvable. Installez rvn dans PATH ou ouvrez main.rvn dans l’éditeur.' >&2
exit 1
