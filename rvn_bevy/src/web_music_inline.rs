// PRIVATE generated from the exact tested factory + browser exports; regenerate, do not edit JS here.
use wasm_bindgen::prelude::*;
#[wasm_bindgen(inline_js = r#"
// PRIVATE: independent browser projection, no Core clock and no saved playhead.
// This factory is shared by browser glue and deterministic MockAudio tests.
export function createMusicBridge({ makeAudio, assetUrl = path => `assets/${path}`, maxEntries = 4097 } = {}) {
    if (typeof makeAudio !== "function" || !Number.isSafeInteger(maxEntries) || maxEntries < 1 || maxEntries > 4097) {
        throw new Error("Invalid music bridge factory/capacity");
    }
    const players = new Map();
    let epoch = null, serial = null, visible = true, suspended = false, nonce = 0n, reportSequence = 0n;
    const u64 = value => typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value) && BigInt(value) <= 18446744073709551615n;
    const text = (value, limit) => typeof value === "string" && value.length > 0 && value.length <= limit;
    function target(value, role) {
        if (!value || !["current", "retired"].includes(role)
            || value.kind !== (role === "current" ? "music" : "presentation_retired")
            || !text(value.alias, 4096) || !u64(value.incarnation) || value.incarnation === "0") {
            throw new Error("Invalid exact music target");
        }
        return { kind: value.kind, alias: value.alias, incarnation: value.incarnation };
    }
    function keyFor(item) { return JSON.stringify([item.epoch, item.role, item.target.kind, item.target.alias, item.target.incarnation]); }
    function identityKey(item) { return JSON.stringify([item.epoch, item.target.kind, item.target.alias, item.target.incarnation]); }
    function validateView(value, planEpoch) {
        if (!value || value.epoch !== planEpoch || !text(value.file, 16384) || value.file.includes("\0")
            || !text(value.authority, 65536) || !Number.isFinite(value.gain) || value.gain < 0 || value.gain > 1
            || typeof value.parked !== "boolean" || typeof value.enabled !== "boolean") {
            throw new Error("Invalid music projection descriptor");
        }
        const result = { epoch: value.epoch, role: value.role, target: target(value.target, value.role),
            file: value.file, authority: value.authority, gain: value.gain, parked: value.parked, enabled: value.enabled,
            handoff: null };
        if (value.handoff !== null && value.handoff !== undefined) {
            const source = value.handoff;
            if (result.role !== "retired" || !source || source.role !== "current" || source.epoch !== planEpoch
                || source.file !== result.file || !text(source.authority, 65536)) {
                throw new Error("Invalid exact Current handoff witness");
            }
            result.handoff = { epoch: source.epoch, role: "current", target: target(source.target, "current"),
                file: source.file, authority: source.authority };
        }
        result.key = keyFor(result);
        return result;
    }
    function validatePlan(plan) {
        if (!plan || !u64(plan.epoch) || !u64(plan.serial) || typeof plan.modern !== "boolean" || !Array.isArray(plan.views)
            || plan.views.length > maxEntries) { throw new Error("Invalid music projection plan"); }
        if (!plan.modern && plan.views.length !== 0) { throw new Error("Legacy cannot claim a Narrative music identity"); }
        if (epoch !== null && BigInt(plan.epoch) < BigInt(epoch)) { throw new Error("Stale music epoch"); }
        if (plan.epoch === epoch && serial !== null && BigInt(plan.serial) <= BigInt(serial)) {
            throw new Error("Stale/duplicate music plan serial");
        }
        const views = plan.views.map(view => validateView(view, plan.epoch));
        const keys = new Set(), identities = new Set(); let currentCount = 0;
        for (const view of views) {
            if (keys.has(view.key) || identities.has(identityKey(view))) { throw new Error("Duplicate exact music target"); }
            keys.add(view.key); identities.add(identityKey(view));
            if (view.role === "current" && ++currentCount > 1) { throw new Error("Two Current music targets"); }
            const previous = players.get(view.key);
            if (previous && previous.file !== view.file) { throw new Error("File changed without a fresh target incarnation"); }
        }
        return { epoch: plan.epoch, serial: plan.serial, views, keys };
    }
    function current(player, capturedNonce) { return players.get(player.key) === player && player.nonce === capturedNonce; }
    function runnable(player) { return !suspended && player.enabled && !player.parked && !player.failed; }
    function detach(player) {
        if (player.errorListener) { player.audio.removeEventListener("error", player.errorListener); player.errorListener = null; }
    }
    function dispose(player) {
        detach(player); player.nonce = (++nonce).toString(); player.starting = false; player.notice = null;
        player.audio.pause();
        player.audio.removeAttribute("src");
        player.audio.load();
    }
    function notice(player, kind, message) {
        // One bounded notice per reader. An actual decode error replaces a policy block.
        if (player.notice?.kind === "error" || (player.notice && kind === "blocked")) { return; }
        player.notice = { report_id: (++reportSequence).toString(), kind, message: String(message).slice(0, 4000),
            key: player.key, epoch: player.epoch, role: player.role, target: { ...player.target },
            authority: player.authority, nonce: player.nonce };
    }
    function bind(player) {
        detach(player); const capturedNonce = player.nonce;
        player.errorListener = () => {
            if (!current(player, capturedNonce)) { return; }
            player.failed = true; player.pendingUnlock = false; player.starting = false; player.audio.pause();
            player.mediaError = "Browser music resource/codec could not be loaded";
            notice(player, "error", player.mediaError);
        };
        player.audio.addEventListener("error", player.errorListener);
        if (player.audio.error) { player.errorListener(); }
    }
    function begin(player, fromGesture = false) {
        if (!runnable(player) || player.starting || (player.pendingUnlock && !fromGesture) || !player.audio.paused) { return; }
        const capturedNonce = player.nonce;
        player.starting = true;
        let promise;
        try { promise = player.audio.play(); } catch (error) { settleFailure(player, capturedNonce, error); return; }
        Promise.resolve(promise).then(() => {
            if (!current(player, capturedNonce)) { return; }
            player.starting = false; player.pendingUnlock = false;
            if (player.notice?.kind === "blocked") { player.notice = null; }
            if (!runnable(player)) { player.audio.pause(); }
        }, error => settleFailure(player, capturedNonce, error));
    }
    function settleFailure(player, capturedNonce, error) {
        if (!current(player, capturedNonce)) { return; }
        player.starting = false;
        if (!runnable(player) || error?.name === "AbortError") { return; }
        player.audio.pause();
        if (error?.name === "NotAllowedError") {
            player.pendingUnlock = true; notice(player, "blocked", "The browser requires a user gesture for music");
        } else {
            player.failed = true; player.pendingUnlock = false;
            player.mediaError = String(error?.message ?? error).slice(0, 4000); notice(player, "error", player.mediaError);
        }
    }
    function configure(player, view, transferred) {
        const changed = transferred || player.authority !== view.authority || player.parked !== view.parked || player.enabled !== view.enabled;
        if (changed) {
            player.nonce = (++nonce).toString(); player.starting = false;
            // Media failures belong to this physical reader/source. Operation renewal,
            // Park/Return and exact handoff must not hide an undrained decode error.
        }
        Object.assign(player, { key: view.key, epoch: view.epoch, role: view.role, target: { ...view.target },
            file: view.file, authority: view.authority, gain: view.gain, parked: view.parked, enabled: view.enabled });
        if (changed) { bind(player); }
        if (player.notice && changed) { player.notice = { ...player.notice, key: player.key, epoch: player.epoch,
            role: player.role, target: { ...player.target }, authority: player.authority, nonce: player.nonce }; }
        if (changed && player.mediaError && !player.notice) { notice(player, "error", player.mediaError); }
        player.audio.volume = view.gain;
        if (!runnable(player)) { player.audio.pause(); }
    }
    function apply(plan) {
        const wanted = validatePlan(plan); // complete validation BEFORE any Audio/mutation.
        const transfers = new Map(), claims = new Set(), additions = new Map();
        for (const view of wanted.views) {
            if (players.has(view.key)) { continue; }
            const source = view.handoff && players.get(keyFor(view.handoff));
            if (source && source.file === view.file && source.authority === view.handoff.authority
                && source.epoch === view.epoch && !wanted.keys.has(source.key) && !claims.has(source.key)) {
                transfers.set(view.key, source); claims.add(source.key);
            }
        }
        // Stage all independent Audio objects before changing existing ownership.
        // Normal synchronous invalid gain/source failures therefore preserve the old map.
        try {
            for (const view of wanted.views) {
                if (players.has(view.key) || transfers.has(view.key)) { continue; }
                const audio = makeAudio(assetUrl(view.file));
                const player = { audio, key: view.key, file: view.file, authority: view.authority,
                    epoch: view.epoch, role: view.role, target: { ...view.target }, gain: view.gain,
                    parked: view.parked, enabled: view.enabled, nonce: (++nonce).toString(),
                    failed: false, starting: false, pendingUnlock: false, mediaError: null, notice: null, errorListener: null };
                additions.set(view.key, player);
                audio.loop = true; audio.preload = "auto"; audio.crossOrigin = "anonymous"; audio.volume = view.gain;
            }
        } catch (error) { for (const player of additions.values()) { dispose(player); } throw error; }
        for (const source of transfers.values()) { detach(source); players.delete(source.key); }
        for (const [key, player] of [...players]) {
            if (!wanted.keys.has(key)) { dispose(player); players.delete(key); }
        }
        for (const view of wanted.views) {
            const transfer = transfers.get(view.key);
            let player = players.get(view.key) || transfer || additions.get(view.key);
            players.set(view.key, player);
            configure(player, view, Boolean(transfer));
            if (additions.has(view.key)) { bind(player); }
        }
        epoch = wanted.epoch; serial = wanted.serial;
        for (const player of players.values()) { begin(player); }
    }
    function unlock() {
        // Called only from a real user gesture. An acknowledgement is not a play request.
        for (const player of players.values()) {
            if (player.pendingUnlock && runnable(player)) { begin(player, true); }
        }
    }
    function errorsForPlan(plan) {
        // Pure pre-publication probe. A failed reader is matched by exact identity
        // or valid handoff witness, then reported under the candidate authority.
        const wanted = validatePlan(plan), claims = new Set(), errors = [];
        for (const view of wanted.views) {
            let player = players.get(view.key);
            if (!player && view.handoff) {
                const source = players.get(keyFor(view.handoff));
                if (source && source.file === view.file && source.authority === view.handoff.authority
                    && !wanted.keys.has(source.key) && !claims.has(source.key)) {
                    player = source; claims.add(source.key);
                }
            }
            if (player?.mediaError) {
                errors.push({ ...(player.notice || { report_id: "0", kind: "error", message: player.mediaError,
                    nonce: player.nonce }), key: view.key, epoch: view.epoch, role: view.role,
                    target: { ...view.target }, authority: view.authority });
            }
        }
        return errors;
    }
    function setVisible(value) {
        if (typeof value !== "boolean") { throw new Error("Invalid page visibility"); }
        // Page/menu/focus clock gates do not revoke a live audio reader.
        // Keep its promise/listener nonce, playhead and playback unchanged.
        visible = value;
    }
    function setSuspended(value) {
        if (typeof value !== "boolean") { throw new Error("Invalid consumer failure suspension"); }
        if (suspended === value) { return; } suspended = value;
        for (const player of players.values()) {
            player.nonce = (++nonce).toString(); player.starting = false;
            if (player.notice) { player.notice = { ...player.notice, nonce: player.nonce }; }
            bind(player);
            if (!runnable(player)) { player.audio.pause(); } else { begin(player); }
        }
    }
    function reports() { return [...players.values()].flatMap(player => player.notice ? [{ ...player.notice, target: { ...player.notice.target } }] : []); }
    function ack(report) {
        const player = players.get(report?.key);
        if (!player?.notice || player.notice.report_id !== report.report_id || player.nonce !== report.nonce
            || player.epoch !== report.epoch || player.authority !== report.authority) { return false; }
        player.notice = null; return true;
    }
    function removeExact(witness) {
        const key = keyFor(witness), player = players.get(key);
        if (!player || player.file !== witness.file || player.authority !== witness.authority) { return false; }
        dispose(player); players.delete(key); return true;
    }
    function reset() {
        // Explicit VnEngine-instance replacement. Core epoch 0 is valid and is
        // not globally unique across independent Engine constructors.
        for (const player of players.values()) { dispose(player); }
        players.clear(); epoch = null; serial = null; suspended = false;
        // Keep nonce/report counters monotonic so no old asynchronous receipt is reused.
    }
    function inspect() { return [...players.values()].map(player => ({ key: player.key, epoch: player.epoch, role: player.role,
        target: { ...player.target }, file: player.file, authority: player.authority, gain: player.gain, parked: player.parked,
        enabled: player.enabled, nonce: player.nonce, pendingUnlock: player.pendingUnlock, failed: player.failed,
        starting: player.starting, audio: player.audio })); }
    return { apply, errorsForPlan, unlock, setVisible, setSuspended, reports, ack, removeExact, reset, inspect };
}

// Browser-only outer adapter; pure factory above is tested directly under Node.
const indexedMusic=createMusicBridge({makeAudio: url=>new Audio(url),assetUrl:path=>{
    const relative=path.startsWith('assets/')?path.slice(7):path;
    return new URL('assets/'+relative.split('/').map(encodeURIComponent).join('/'),document.baseURI).href;
}});
indexedMusic.setVisible(!document.hidden);
document.addEventListener('visibilitychange',()=>indexedMusic.setVisible(!document.hidden));
export function rvn_indexed_music_apply(description){
    const snapshot=JSON.parse(description);indexedMusic.apply(snapshot);
    if(snapshot.modern){
        const legacy=globalThis.__rvnMusicAudio;
        if(legacy){legacy.pause();legacy.removeAttribute('src');legacy.load();delete globalThis.__rvnMusicAudio;}
    }
}
export function rvn_indexed_music_errors(description){if(typeof Audio!=='function')throw new Error('HTMLAudio is unavailable');return JSON.stringify(indexedMusic.errorsForPlan(JSON.parse(description)));}
export function rvn_indexed_music_reports(){return JSON.stringify(indexedMusic.reports());}
export function rvn_indexed_music_ack(description){return indexedMusic.ack(JSON.parse(description));}
export function rvn_indexed_music_unlock(){indexedMusic.unlock();}
export function rvn_indexed_music_suspend(value){indexedMusic.setSuspended(value);}
export function rvn_indexed_music_reset(){indexedMusic.reset();}

"#)]
extern "C" {
    #[wasm_bindgen(catch)] fn rvn_indexed_music_apply(description:&str)->Result<(),JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_errors(description:&str)->Result<String,JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_reports()->Result<String,JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_ack(description:&str)->Result<bool,JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_unlock()->Result<(),JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_suspend(value:bool)->Result<(),JsValue>;
    #[wasm_bindgen(catch)] fn rvn_indexed_music_reset()->Result<(),JsValue>;
}
fn problem(error:JsValue)->String {format!("Browser music bridge: {error:?}")}
pub(super) fn apply(description:&str)->Result<(),String>{rvn_indexed_music_apply(description).map_err(problem)}
pub(super) fn errors_for_plan(description:&str)->Result<String,String>{rvn_indexed_music_errors(description).map_err(problem)}
pub(super) fn reports()->Result<String,String>{rvn_indexed_music_reports().map_err(problem)}
pub(super) fn ack(description:&str)->Result<(),String>{rvn_indexed_music_ack(description).map(|_|()).map_err(problem)}
pub(super) fn unlock()->Result<(),String>{rvn_indexed_music_unlock().map_err(problem)}
pub(super) fn suspend(value:bool)->Result<(),String>{rvn_indexed_music_suspend(value).map_err(problem)}
pub(super) fn reset()->Result<(),String>{rvn_indexed_music_reset().map_err(problem)}
