#!/usr/bin/env bash
set -euo pipefail
task_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
task_output="$(mktemp -d "${TMPDIR:-/tmp}/rvn-menu-preview-designs.XXXXXX")"
cd "$task_repo"
printf 'Comparaisons : %s\n' "$task_output"
cargo test -p rvn_ui --test shipped_designs > "$task_output/tests.log" 2>&1
cargo build --release -p rvn_bevy > "$task_output/build.log" 2>&1
task_index=0
for task_design in Sobre Illustré Science-fiction; do
    for task_size in 1280x720 1920x1080 2560x1080; do
        task_index=$((task_index+1))
        task_project="$task_output-project-$task_index"
        task_capture="$task_output/$task_design-$task_size"
        mkdir "$task_capture"
        cargo run -p rvn_ui --example shipped_design_fixture -- "$task_design" "$task_project" "${task_size%x*}" "${task_size#*x}" >> "$task_output/fixtures.log" 2>&1
        env RUST_LOG=error RVN_QA_OUTPUT="$task_capture" RVN_QA_MENU_DESIGN=1 \
            timeout 40s "$task_repo/target/release/rvn_bevy" "$task_project" > "$task_capture/run.log" 2>&1
        test -s "$task_capture/design.txt"
        cmp "$task_project/menus.rvnui" "$task_repo/examples/menu-designs/$task_design/menus.rvnui"
        printf 'Validé : %s — demandé %s — ' "$task_design" "$task_size"
        cat "$task_capture/design.txt"
    done
done
printf 'Comparaisons terminées : %s\n' "$task_output"
