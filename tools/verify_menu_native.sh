#!/usr/bin/env bash
# Creates disposable projects; never opens a user's story or save directory.
set -euo pipefail
task_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
task_output="$(mktemp -d "${TMPDIR:-/tmp}/rvn-menu-native.XXXXXX")"
cd "$task_repo"
printf 'Rapport et projets isolés : %s\n' "$task_output"
cargo test -p rvn_core -p rvn_ui -p rvn_bevy --lib > "$task_output/tests.log" 2>&1
cargo test -p rvn_ui --test measured_templates --test local_controls >> "$task_output/tests.log" 2>&1
cargo build --release -p rvn_bevy > "$task_output/build.log" 2>&1
cargo run -p rvn_ui --example pointer_fixture -- "$task_output/pointer-project" > "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example auto_height_fixture -- "$task_output/auto-height-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example local_controls_fixture -- "$task_output/local-controls-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example pointer_fixture -- "$task_output/controller-project" --controller >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example pointer_fixture -- "$task_output/disabled-control-project" --controller --disabled-control >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example controller_choices_fixture -- "$task_output/controller-choices-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example conditional_cards_fixture -- "$task_output/cards-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example pagination_fixture -- "$task_output/pagination-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example pagination_fixture -- "$task_output/column-pagination-project" --column-order >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example resume_thumbnail_fixture -- "$task_output/resume-thumbnails-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example text_states_fixture -- "$task_output/text-states-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example text_states_fixture -- "$task_output/image-states-project" --images >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example pointer_fixture -- "$task_output/navigation-loop-project" --navigation-loop >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example confirmation_fixture -- "$task_output/confirm-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example confirmation_fixture -- "$task_output/confirm-graph-project" --graphs >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example nested_scroll_fixture -- "$task_output/scroll-project" --dialogue >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example long_dialogue_fixture -- "$task_output/dialogue-project" >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example long_dialogue_fixture -- "$task_output/styled-scroll-project" --styled-scroll >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example animation_fixture -- "$task_output/shadow-project" --shadow >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example animation_fixture -- "$task_output/frame-project" --frame >> "$task_output/fixtures.log" 2>&1
cargo run -p rvn_ui --example imagemap_fixture -- "$task_output/map-project" >> "$task_output/fixtures.log" 2>&1
run_native() {
    local task_name="$1" task_project="$2" task_proof="$3"
    shift 3
    mkdir "$task_output/$task_name"
    env RUST_LOG=error RVN_QA_OUTPUT="$task_output/$task_name" "$@" \
        timeout 45s "$task_repo/target/release/rvn_bevy" "$task_output/$task_project" \
        > "$task_output/$task_name/run.log" 2>&1
    test -s "$task_output/$task_name/$task_proof"
    printf 'Validé : %s\n' "$task_name"
}
run_native pointer pointer-project pointer.txt RVN_QA_POINTER=1
run_native auto-height auto-height-project auto-height.txt RVN_QA_AUTO_HEIGHT=1
run_native local-controls local-controls-project local-controls.txt RVN_QA_LOCAL_CONTROLS=1
run_native resume-thumbnails resume-thumbnails-project resume-thumbnails.txt RVN_QA_RESUME_THUMBNAILS=1
run_native text-states text-states-project text-states.txt RVN_QA_TEXT_STATES=1
run_native image-states image-states-project text-states.txt RVN_QA_TEXT_STATES=1 RVN_QA_IMAGE_STATES=1
run_native languages pointer-project dropdown.txt RVN_QA_LONG_DROPDOWN=1
run_native languages-controller pointer-project dropdown.txt RVN_QA_LONG_DROPDOWN=1 RVN_QA_GAMEPAD=1
run_native controller-focus controller-project controller.txt RVN_QA_CONTROLLER_FOCUS=1 RVN_QA_GAMEPAD=1
run_native controller-choices controller-choices-project controller-choices.txt RVN_QA_CONTROLLER_CHOICES=1 RVN_QA_GAMEPAD=1
run_native disabled-choices controller-choices-project disabled-choices.txt RVN_QA_DISABLED_CHOICES=1
run_native disabled-control disabled-control-project disabled-control.txt RVN_QA_DISABLED_CONTROL=1
run_native conditional-cards cards-project cards.txt RVN_QA_CONDITIONAL_CARDS=1
run_native pagination pagination-project pagination.txt RVN_QA_PAGINATION=1
run_native column-pagination column-pagination-project pagination.txt RVN_QA_PAGINATION=1 RVN_QA_COLUMN_ORDER=1
run_native navigation-loop navigation-loop-project navigation.txt RVN_QA_NAVIGATION_LOOP=1
run_native confirmation confirm-project general-confirmations.txt RVN_QA_GENERAL_CONFIRM=1
run_native confirmation-graph confirm-graph-project general-confirmations.txt RVN_QA_GENERAL_CONFIRM=1 RVN_QA_CONFIRM_GRAPH=1
run_native gameplay-scroll scroll-project scroll-drag.txt RVN_QA_SCROLL=1 RVN_QA_SCROLL_GAMEPLAY=1
run_native long-dialogue dialogue-project dialogue.txt RVN_QA_LONG_DIALOGUE=1
run_native styled-scroll styled-scroll-project dialogue.txt RVN_QA_LONG_DIALOGUE=1
run_native animated-shadow shadow-project animation.txt RVN_QA_ANIMATION=1
run_native nine-slice frame-project animation.txt RVN_QA_ANIMATION=1
for task_zone in 0 1 2; do
    run_native "imagemap-$task_zone" map-project finished.txt RVN_QA_MAP_TARGET="$task_zone" RVN_QA_CUSTOM_CHOICES=1
    rg -q "Destination $task_zone atteinte" "$task_output/imagemap-$task_zone/dialogue-trace.txt"
done
printf 'Parcours natifs terminés. Captures et journaux : %s\n' "$task_output"
