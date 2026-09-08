#!/usr/bin/env bash
# Real native UI input, isolated projects and recent-project list. No JSON edits.
set -euo pipefail
task_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
task_editor="$task_repo/../makepad"
task_output="$(mktemp -d "${TMPDIR:-/tmp}/rvn-menu-editor.XXXXXX")"
task_port=""
printf 'Projets et rapport : %s\n' "$task_output"
for task_command in curl jq cargo; do command -v "$task_command" >/dev/null; done
cleanup() {
    if [[ -n "$task_port" ]]; then
        curl --max-time 12 -s "http://127.0.0.1:$task_port/gq" > "$task_output/final-grab.json" || true
    fi
}
trap cleanup EXIT
trap 'printf "Échec à la ligne %s\n" "$LINENO" >&2; if [[ -n "$task_port" ]]; then curl --max-time 8 -s "http://127.0.0.1:$task_port/snap" > "$task_output/failure-widgets.json"; curl --max-time 8 -s "http://127.0.0.1:$task_port/g" > "$task_output/failure-grab.json"; fi' ERR
cd "$task_editor"
cargo build --release -p makepad-blueprint-demo > "$task_output/build.log" 2>&1
RVN_EDITOR_RECENTS="$task_output/recents.json" ./target/release/makepad-blueprint-demo --remote > "$task_output/editor.log" 2>&1 &
task_pid=$!
for ((task_attempt=0; task_attempt<60; task_attempt++)); do
    task_port="$(sed -n 's/.*listening on 127\.0\.0\.1:\([0-9]*\).*/\1/p' "$task_output/editor.log" | head -1)"
    [[ -n "$task_port" ]] && break
    kill -0 "$task_pid" 2>/dev/null || { printf 'L’éditeur a quitté au démarrage.\n'; exit 1; }
    sleep 0.2
done
[[ -n "$task_port" ]] || { printf 'Contrôle local indisponible.\n'; exit 1; }
request() {
    local task_reply
    task_reply="$(curl --max-time 8 -fsS "http://127.0.0.1:$task_port/$1")"
    jq -e 'has("err") | not' <<< "$task_reply" >/dev/null
    printf '%s\n' "$task_reply"
}
widget() { request "snap?q=$1" | jq -ce --arg id "$1" '.s[] | select(.i==$id and .r[2]>0 and .r[3]>0)'; }
click() {
    local task_rect task_x task_y
    task_rect="$(widget "$1")"
    task_x="$(jq '.r[0]+.r[2]/2' <<< "$task_rect")"
    task_y="$(jq '.r[1]+.r[3]/2' <<< "$task_rect")"
    request "click?x=$task_x&y=$task_y&wait=1" >/dev/null
}
type_field() {
    click "$1"
    request 'k?k=press&c=KeyA&ctrl=1&wait=1' >/dev/null
    request "t?t=$(jq -rn --arg text "$2" '$text|@uri')&wait=1" >/dev/null
}
scroll_left() { request "m?k=scroll&x=175&y=470&dy=$1&wait=1" >/dev/null; }
left_click() {
    local task_info task_try
    scroll_left -10000
    for ((task_try=0; task_try<80; task_try++)); do
        if task_info="$(widget "$1")" && jq -e '.r[3]>=20' <<< "$task_info" >/dev/null; then
            click "$1"; return
        fi
        scroll_left 120
    done
    printf 'Commande hors d’atteinte : %s\n' "$1" >&2; return 1
}
right_click() {
    local task_info task_try
    request 'm?k=scroll&x=1150&y=460&dy=-10000&wait=1' >/dev/null
    for ((task_try=0; task_try<50; task_try++)); do
        if task_info="$(widget "$1")" && jq -e '.r[3]>=20' <<< "$task_info" >/dev/null; then
            click "$1"; return
        fi
        request 'm?k=scroll&x=1150&y=460&dy=100&wait=1' >/dev/null
    done
    printf 'Propriété hors d’atteinte : %s\n' "$1" >&2; return 1
}
for ((task_attempt=0; task_attempt<60; task_attempt++)); do
    if request s | jq -e '.w|length>0' >/dev/null && widget new_project >/dev/null; then break; fi
    sleep 0.2
done
request s > "$task_output/window.json"
jq -e '.w[0].sz[0]>=1200 and .w[0].sz[1]>=640' "$task_output/window.json" >/dev/null
click new_project
click story
type_field project_name 'Validation visuelle'
request 'm?k=scroll&x=1100&y=470&dy=260&wait=1' >/dev/null
type_field parent_path "$task_output"
click create
click edit_menus
task_project="$task_output/Validation visuelle"
for task_index in 0 1 2; do
    task_names=(Sobre Illustré Science-fiction)
    task_name="${task_names[$task_index]}"
    click pages_tab
    scroll_left -2000
    scroll_left 420
    task_selected="$(widget layout_template | jq -r '.t')"
    case "$task_selected" in Sobre) task_previous=0;; Illustré) task_previous=1;; Science-fiction) task_previous=2;; *) exit 1;; esac
    click layout_template
    task_y=$((534+22*(task_index-task_previous)))
    request "m?k=move&x=110&y=$task_y&wait=1" >/dev/null
    request "click?x=110&y=$task_y&wait=1" >/dev/null
    widget layout_template | jq -e --arg name "$task_name" '.t==$name' >/dev/null
    click prepare_template
    scroll_left 230
    click confirm_template
    click save_menus
    widget menu_status | jq -e '.t=="Menus enregistrés"' >/dev/null
    jq -e '.pages|length==11' "$task_project/menus.rvnui" >/dev/null
    cp "$task_project/menus.rvnui" "$task_output/$task_name.rvnui"
    request g > "$task_output/$task_name-grab.json"
    click components_tab
    scroll_left 800
    type_field package_path "$task_output/$task_name.rvnuitheme"
    click export_design
    widget menu_status | jq -e '.t|startswith("Design exporté")' >/dev/null
    jq -e '.document.pages|length==11' "$task_output/$task_name.rvnuitheme" >/dev/null
    printf 'Validé par l’interface : %s\n' "$task_name"
done
# Import twice into the same project: conflict review and independent copies.
left_click inspect_design
widget package_review > "$task_output/import-review.json"
left_click confirm_design
click save_menus
jq -e '.pages|length==22' "$task_project/menus.rvnui" >/dev/null
left_click inspect_design
left_click confirm_design
click save_menus
jq -e '.pages|length==33' "$task_project/menus.rvnui" >/dev/null
click undo_menu
click save_menus
jq -e '.pages|length==22' "$task_project/menus.rvnui" >/dev/null
click redo_menu
click save_menus
jq -e '.pages|length==33' "$task_project/menus.rvnui" >/dev/null
printf 'Validé : import répété sans écrasement, annuler/rétablir, sauvegarde.\n'
# Undo/redo restores the imported title page, not the last available page.
request 'click?x=430&y=324&wait=1' >/dev/null
widget selected_name | jq -e '.t!="Aucune sélection"' >/dev/null
right_click shared_style_name
type_field shared_style_name 'Titres personnalisés'
right_click create_shared_style
click save_menus
jq -e '.styles["Titres personnalisés"]!=null' "$task_project/menus.rvnui" >/dev/null
click undo_menu
click save_menus
jq -e '.styles["Titres personnalisés"]==null' "$task_project/menus.rvnui" >/dev/null
click redo_menu
request 'click?x=430&y=324&wait=1' >/dev/null
right_click shared_style_name
type_field shared_style_name 'Titres lumineux'
right_click rename_shared_style
click save_menus
jq -e '.styles["Titres lumineux"]!=null and .styles["Titres personnalisés"]==null' "$task_project/menus.rvnui" >/dev/null
printf 'Validé : style nommé, renommage des références et annulation.\n'
task_font_before="$(jq '.styles["Titres lumineux"].font_size' "$task_project/menus.rvnui")"
right_click paint_color
type_field hex '#235577ff'
request 'k?k=press&c=ReturnKey&wait=1' >/dev/null
request 'click?x=950&y=620&wait=1' >/dev/null
click share_paint
click save_menus
jq -e --argjson font "$task_font_before" '.styles["Titres lumineux"] | .font_size==$font and .normal[0]>0.137 and .normal[0]<0.138 and .normal[1]>0.333 and .normal[1]<0.334' "$task_project/menus.rvnui" >/dev/null
click undo_menu
click undo_menu
click redo_menu
click redo_menu
click save_menus
jq -e --argjson font "$task_font_before" '.styles["Titres lumineux"] | .font_size==$font and .normal[0]>0.137 and .normal[0]<0.138' "$task_project/menus.rvnui" >/dev/null
printf 'Validé : couleur saisie dans le popup, partage isolé et annulation.\n'
cp "$task_project/menus.rvnui" "$task_output/before-reopen.rvnui"
cleanup
wait "$task_pid"
task_port=""
RVN_EDITOR_RECENTS="$task_output/recents.json" ./target/release/makepad-blueprint-demo --remote --graph "$task_project/graphs/start.rvngraph" > "$task_output/reopen.log" 2>&1 &
task_pid=$!
for ((task_attempt=0; task_attempt<60; task_attempt++)); do
    task_port="$(sed -n 's/.*listening on 127\.0\.0\.1:\([0-9]*\).*/\1/p' "$task_output/reopen.log" | head -1)"
    if [[ -n "$task_port" ]] && widget edit_menus >/dev/null; then break; fi
    sleep 0.2
done
click edit_menus
click save_menus
widget menu_status | jq -e '.t=="Menus enregistrés"' >/dev/null
cmp "$task_project/menus.rvnui" "$task_output/before-reopen.rvnui"
printf 'Validé : réouverture et enregistrement sans perte.\n'
printf 'Rapport : %s\n' "$task_output"
