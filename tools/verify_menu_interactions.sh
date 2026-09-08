#!/usr/bin/env bash
# Native regression: only an isolated copy of the shipped example is edited.
set -euo pipefail
task_repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
task_output="$(mktemp -d /tmp/rvn-interactions.XXXXXX)"
task_port=""
trap 'if [[ -n "$task_port" ]]; then curl --max-time 12 -s "http://127.0.0.1:$task_port/gq" > "$task_output/quit.json" || true; fi' EXIT
cp -a "$task_repo/examples/menu-designs/Sobre" "$task_output/project"
cd "$task_repo/../makepad"
cargo build --release -p makepad-blueprint-demo > "$task_output/build.log" 2>&1
RVN_EDITOR_RECENTS="$task_output/recents.json" ./target/release/makepad-blueprint-demo --remote --graph "$task_output/project/graphs/start.rvngraph" > "$task_output/editor.log" 2>&1 &
for ((i=0;i<60;i++)); do
    task_port="$(sed -n 's/.*listening on 127\.0\.0\.1:\([0-9]*\).*/\1/p' "$task_output/editor.log" | head -1)"
    [[ -n "$task_port" ]] && break
    sleep 0.2
done
request(){ curl --max-time 12 -fsS "http://127.0.0.1:$task_port/$1"; }
click(){ local r; r="$(request "snap?q=$1" | jq -ce --arg id "$1" '.s[]|select(.i==$id and .r[3]>=20)')"; request "click?x=$(jq '.r[0]+.r[2]/2' <<< "$r")&y=$(jq '.r[1]+.r[3]/2' <<< "$r")&wait=1" >/dev/null; }
key(){ request "k?k=press&c=$1&wait=1" >/dev/null; }
drag(){ request "m?k=down&x=$1&y=$2&b=$5" >/dev/null; request "m?k=move&x=$3&y=$4" >/dev/null; request "m?k=up&x=$3&y=$4&b=$5&wait=1" >/dev/null; }
for ((i=0;i<60;i++)); do request 'snap?q=edit_menus' | jq -e '.s|length>0' >/dev/null && break; sleep 0.2; done
request s | jq -e '.w[0].sz==[1280,650]' >/dev/null
click edit_menus
click interactions_mode
click interaction_scope
key ArrowDown; key ReturnKey
click interaction_event
key ArrowDown; key ArrowDown; key ArrowDown; key ReturnKey
click new_interaction
click save_menus
task_document="$task_output/project/menus.rvnui"
jq -c '.pages[0].graphs' "$task_document" > "$task_output/before.json"
# Search from an execution output; cancelling must not modify the document.
drag 660 263 740 375 0
request 't?t=son&wait=1' >/dev/null
request g > "$task_output/search.json"
key Escape
click save_menus
jq -c '.pages[0].graphs' "$task_document" | cmp - "$task_output/before.json"
# Add + connect is one undoable operation, at the drop position.
drag 660 263 740 375 0
key ReturnKey
click add_interaction_node
click save_menus
jq -e '.pages[0].graphs[0] | .nodes[0].next==1 and .nodes[1].position==[510,268]' "$task_document" >/dev/null
click undo_menu
click save_menus
jq -c '.pages[0].graphs' "$task_document" | cmp - "$task_output/before.json"
click redo_menu
click save_menus
jq -c '.pages[0].graphs' "$task_document" > "$task_output/added.json"
# Reproduce the trapped animation inspector. Its footer must stay visible.
request 'm?k=scroll&x=1150&y=400&dy=200&wait=1' >/dev/null
click interaction_operation
for ((i=0;i<8;i++)); do key ArrowDown; done
key ReturnKey
request 'snap?q=discard_interaction_node' | jq -e '.s[0].r[1]+.s[0].r[3]<590' >/dev/null
drag 600 420 450 400 1
request 'm?k=scroll&x=600&y=300&dy=120&wait=1' >/dev/null
request g > "$task_output/pending-navigation.json"
key Escape
click save_menus
jq -c '.pages[0].graphs' "$task_document" | cmp - "$task_output/added.json"
# Middle-button navigation and keyboard framing; neither changes graph data.
drag 600 420 750 410 2
key KeyF
click save_menus
jq -c '.pages[0].graphs' "$task_document" | cmp - "$task_output/added.json"
request g > "$task_output/final.json"
# Design selection must never display another control's graph.
click interactions_mode
click fit_menu
request 'click?x=550&y=279&wait=1' >/dev/null
click interactions_mode
click interaction_scope
key ArrowUp; key ReturnKey
request 'm?k=scroll&x=1150&y=400&dy=-10000&wait=1' >/dev/null
click interaction_event
key ArrowUp; key ArrowUp; key ReturnKey
click new_interaction
click interactions_mode
request 'click?x=550&y=310&wait=1' >/dev/null
click interactions_mode
request 'snap?q=interaction_owner' | jq -e '.s[0].t|contains("button_1")' >/dev/null
request 'snap?q=interaction_graph' | jq -e '.s[0].t=="Aucune interaction"' >/dev/null
request g > "$task_output/empty-owner.json"
click interactions_mode
request 'click?x=550&y=279&wait=1' >/dev/null
click interactions_mode
request 'snap?q=interaction_graph' | jq -e '.s[0].t|contains("button_0")' >/dev/null
click save_menus
printf 'Interactions : recherche, annulation, ajout lié, undo/redo, saisie et navigation validés. Rapport : %s\n' "$task_output"
