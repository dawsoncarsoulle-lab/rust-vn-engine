//! Prepared only. Intended child module of native_methodcall_video_0691.rs.
//! Reuses that target's genuine Graph/source/Renderer helpers; no second owner.
use super::*;

fn ready(name: &str, raw: &str) -> (Engine<Host>, ParsedProgram) {
    let p = fixture(name, raw, true);
    let mut e = Engine::from_program(p.clone(), Host::default(), 16).unwrap();
    assert!(matches!(e.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "before"));
    e.advance_dialogue().unwrap();
    (e, p)
}
fn refusing(e: &mut Engine<Host>) {
    let before = state(e);
    let epoch = e.runtime_epoch();
    let n = count(e);
    assert!(e.preview_step_statement().is_err());
    assert_eq!(state(e), before);
    assert_eq!(e.runtime_epoch(), epoch);
    assert_eq!(count(e), n);
}

#[test]
fn method0818_skip_callback_rng_renderer_refusal_then_retry_is_once() {
    let raw = format!("handler finished(event){{set roll=random(1,999999) set ends=ends+1}}\ninit{{set ends=0}}\nscene label start\n{SETUP}\n\"before\"\nintro.skip()\n\"after\"\n");
    let (mut e, _) = ready("rng-callback", &raw);
    let mut expected = rvn_core::random::RandomState::seeded(818);
    e.state.random = expected;
    let draw = expected.integer(1, 999999).unwrap();
    let session = e.debug_begin(DebugOperation::Statement, vec![]).unwrap();
    e.renderer.reject.store(true, Ordering::SeqCst);
    refusing(&mut e);
    assert!(e.accepts_debug_session(&session));
    e.renderer.reject.store(false, Ordering::SeqCst);
    let n = count(&e);
    e.preview_step_statement().unwrap();
    assert_eq!(count(&e), n + 1);
    assert_eq!(e.state.vars["ends"], Value::Int(1));
    assert_eq!(e.state.vars["roll"], Value::Int(draw));
    assert_eq!(e.state.random, expected);
    assert_eq!(e.renderer.views.lock().unwrap()[0].track.playback, Playback::Ended);
}

#[test]
fn method0818_second_skip_cannot_duplicate_the_actual_terminal_callback() {
    let (mut e, _) = scene("repeat-skip", "", "intro.skip() intro.skip()");
    e.preview_step_statement().unwrap();
    assert_eq!(e.state.vars["ends"], Value::Int(1));
    let rng = e.state.random;
    e.preview_step_statement().unwrap();
    assert_eq!(e.state.vars["ends"], Value::Int(1));
    assert_eq!(e.state.random, rng);
    assert_eq!(e.state.videos.tracks["intro"].playback, Playback::Ended);
}

#[test]
fn method0818_second_stop_refuses_the_really_retired_receiver_atomically() {
    let (mut e, _) = scene("repeat-stop", "", "intro.stop() intro.stop()");
    let old = e.video_views().unwrap().remove(0);
    e.preview_step_statement().unwrap();
    assert!(!e.state.videos.tracks.contains_key("intro"));
    assert!(!e.resume_video_from_view(&old).unwrap());
    refusing(&mut e);
}

#[test]
fn method0818_actual_autoplay_blocked_feedback_then_resume_clears_message() {
    let (mut e, _) = scene("blocked", "", "intro.resume()");
    let view = e.video_views().unwrap().remove(0);
    assert!(e.video_feedback_from_view(&view, rvn_core::video::Feedback::AutoplayBlocked("real host refused autoplay".into())).unwrap());
    assert_eq!(e.state.videos.tracks["intro"].playback, Playback::Blocked);
    assert!(e.state.videos.tracks["intro"].message.is_some());
    e.preview_step_statement().unwrap();
    assert_eq!(e.state.videos.tracks["intro"].playback, Playback::Playing);
    assert_eq!(e.state.videos.tracks["intro"].message, None);
}

#[test]
fn method0818_unskippable_clip_refuses_skip_before_callback_or_publication() {
    let raw = "handler finished(event){set ends=ends+1}\ninit{set ends=0}\nscene label start\nvideo.play(\"intro\",video_clip(\"clip.webm\",{\"skippable\":false,\"on_end\":\"finished\"}))\n\"before\"\nintro.skip()\n\"after\"\n";
    let (mut e, _) = ready("unskippable", raw);
    refusing(&mut e);
    assert_eq!(e.state.vars["ends"], Value::Int(0));
    assert_eq!(e.state.videos.tracks["intro"].playback, Playback::Playing);
}

#[test]
fn method0818_looping_clip_cannot_be_ended_by_skip() {
    let raw = "handler finished(event){set ends=ends+1}\ninit{set ends=0}\nscene label start\nvideo.play(\"intro\",video_clip(\"clip.webm\",{\"skippable\":true,\"looping\":true,\"on_end\":\"finished\"}))\n\"before\"\nintro.skip()\n\"after\"\n";
    let (mut e, _) = ready("looping", raw);
    assert!(e.state.videos.tracks["intro"].clip.looping);
    refusing(&mut e);
    assert_eq!(e.state.vars["ends"], Value::Int(0));
}

#[test]
fn method0818_real_end_and_error_feedback_refuse_resume_without_replay() {
    for failed in [false, true] {
        let (mut e, _) = scene(if failed {"failed-resume"} else {"ended-resume"}, "", "intro.resume()");
        let view = e.video_views().unwrap().remove(0);
        let feedback = if failed {rvn_core::video::Feedback::Error("real decoder report".into())} else {rvn_core::video::Feedback::End};
        assert!(e.video_feedback_from_view(&view, feedback).unwrap());
        assert_eq!(e.state.videos.tracks["intro"].playback, if failed {Playback::Failed} else {Playback::Ended});
        refusing(&mut e);
    }
}

#[test]
fn method0818_scene_call_parks_parent_receiver_and_child_cannot_claim_it() {
    let raw = format!("handler finished(event){{set ends=ends+1}}\ninit{{set ends=0}}\nscene label start\n{SETUP}\n\"before\"\ncall child\n\"returned\"\nreturn\nscene label child\n\"child\"\nintro.pause()\nreturn\n");
    let (mut e, _) = ready("parked-parent", &raw);
    let old = e.video_views().unwrap().remove(0);
    assert!(matches!(e.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue{text,..}) if text == "child"));
    assert!(e.state.scene_execution.as_ref().unwrap().frames.iter().any(|frame| frame.parked.is_some()));
    assert!(!e.resume_video_from_view(&old).unwrap());
    e.advance_dialogue().unwrap();
    refusing(&mut e);
}

#[test]
fn method0818_real_video_wait_blocks_story_method_until_real_end_feedback() {
    let (mut e, _) = scene("wait-boundary", "", "video.wait(\"intro\") intro.stop()");
    e.preview_step_statement().unwrap();
    assert_eq!(e.state.videos.waiting.as_deref(), Some("intro"));
    let before = state(&e);
    let n = count(&e);
    assert_eq!(e.preview_step_statement().unwrap(), None);
    assert_eq!(state(&e), before);
    assert_eq!(count(&e), n);
    let view = e.video_views().unwrap().remove(0);
    assert!(e.video_feedback_from_view(&view, rvn_core::video::Feedback::End).unwrap());
    assert_eq!(e.state.videos.waiting, None);
    assert_eq!(e.state.vars["ends"], Value::Int(1));
    e.preview_step_statement().unwrap();
    assert!(!e.state.videos.tracks.contains_key("intro"));
    // This does not claim that a blocked Story executed MethodCall during wait.
}

#[test]
fn method0818_real_legacy_game_video_does_not_forge_narrative_method_owner() {
    let raw = format!("handler finished(event){{set ends=ends+1}}\ninit{{set ends=0}}\nlabel start\n{SETUP}\n\"before\"\nintro.pause()\n\"after\"\n");
    let (mut e, _) = ready("game-owner", &raw);
    assert!(e.state.scene_execution.is_none());
    assert!(e.state.videos.tracks.contains_key("intro"));
    refusing(&mut e);
}

#[test]
fn method0818_actual_skip_callback_breakpoint_roundtrips_stopped_save15() {
    let raw = format!("function callback_draw(){{local value=random(1,999999) return value}}\nhandler finished(event){{set roll=callback_draw() set ends=ends+1}}\ninit{{set ends=0}}\nscene label start\n{SETUP}\n\"before\"\nintro.skip()\n\"after\"\n");
    let (mut warm, p) = ready("callback-cold", &raw);
    let mut expected = rvn_core::random::RandomState::seeded(818);
    warm.state.random = expected;
    let draw = expected.integer(1, 999999).unwrap();
    // Discover the actual source-backed site. Never manufacture a handler scope,
    // PC, SourceHash, route, or resident Screen permit for this media operation.
    let mut offset = 0;
    let breakpoint = loop {
        let page = warm.debug_site_page(offset).unwrap();
        if let Some(entry) = page.entries.iter().find(|entry| entry.breakpoint.site.as_ref().is_some_and(|site| site.function.as_deref() == Some("callback_draw") && site.statement)) {
            break entry.breakpoint.clone();
        }
        offset += page.entries.len();
        assert!(offset < page.total, "real callback function site was not exposed");
    };
    let before = state(&warm);
    let old = warm.video_views().unwrap().remove(0);
    let mut session = warm.debug_begin(DebugOperation::Statement, vec![breakpoint.clone()]).unwrap();
    let mut stopped = false;
    for _ in 0..128 {
        let progress = warm.debug_step(&mut session, StepKind::Continue).unwrap().0;
        if progress == rvn_core::engine::DebugProgress::Stopped {
            let actual = session.snapshot().site.expect("actual callback stop site");
            let authored = breakpoint.site.as_ref().unwrap();
            assert_eq!(actual.function, authored.function);
            assert_eq!(actual.route, authored.route);
            assert_eq!(actual.statement, authored.statement);
            assert!(actual.depth > 0, "callback must have a real callee frame");
            stopped = true;
            break;
        }
        assert_ne!(progress, rvn_core::engine::DebugProgress::Complete, "method callback passed its actual breakpoint");
    }
    assert!(stopped);
    assert_eq!(state(&warm), before);
    let stopped_site = session.snapshot().site;
    let capture_source0883=warm.debug_state_source().unwrap();
    let save = warm.capture_save15(&p, Some(&session), 1, "actual method callback stop".into()).unwrap();
    assert!(warm.accepts_debug_state_source(&capture_source0883),"capture changed the exact opaque source/anchor");
    assert!(warm.accepts_debug_session(&session),"capture invalidated its retained real session");
    assert!(warm.accepts_debug_session(&session));
    let bytes = save.encode().unwrap();
    let mut cold = Engine::from_program(p.clone(), Host::default(), 16).unwrap();
    let mut restored = cold.restore_save15(&p, rvn_core::save::consumer_checkpoint::Save15::decode(&bytes).unwrap()).unwrap().expect("stopped method callback continuation");
    assert_eq!(restored.snapshot().site, stopped_site, "cold continuation preserves the actual stopped callee depth and route");
    assert!(!cold.resume_video_from_view(&old).unwrap());
    for (engine, session) in [(&mut warm, &mut session), (&mut cold, &mut restored)] {
        let mut complete = false;
        for _ in 0..128 {
            if engine.debug_step(session, StepKind::Continue).unwrap().0 == rvn_core::engine::DebugProgress::Complete { complete = true; break; }
        }
        assert!(complete);
        assert_eq!(engine.state.vars["ends"], Value::Int(1));
        assert_eq!(engine.state.vars["roll"], Value::Int(draw));
        assert_eq!(engine.state.random, expected);
        assert_eq!(engine.state.videos.tracks["intro"].playback, Playback::Ended);
    }
    assert_eq!(cold.state.vars, warm.state.vars);
    assert_eq!(cold.state.pc, warm.state.pc);
}


// Genuine public fixture: the same retained Statement reaches the actual
// handler/callee breakpoint. No scope, owner, frame, site or Program is injected.
fn stopped_statement_media0883(name:&str)->(Engine<Host>,ParsedProgram,rvn_core::engine::DebugSession,rvn_core::random::RandomState,i64){
    let raw=format!("function callback_draw(){{local value=random(1,999999) return value}}\nhandler finished(event){{set roll=callback_draw() set ends=ends+1}}\ninit{{set ends=0}}\nscene label start\n{SETUP}\n\"before\"\nintro.skip()\n\"after\"\n");
    let(mut warm,p)=ready(name,&raw);
    warm.state.random=rvn_core::random::RandomState::seeded(883);
    let mut expected=warm.state.random;let draw=expected.integer(1,999999).unwrap();
    let mut breakpoint=None;let mut offset=0;
    loop{
        let page=warm.debug_site_page(offset).unwrap();
        let count=page.entries.len();
        for row in page.entries{if row.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("callback_draw")&&site.statement){breakpoint=Some(row.breakpoint);break;}}
        if breakpoint.is_some(){break;}offset+=count;assert!(offset<page.total,"actual callback breakpoint missing");
    }
    let breakpoint=breakpoint.unwrap();
    let before=state(&warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![breakpoint.clone()]).unwrap();
    let mut stopped=false;
    for _ in 0..128{
        let progress=warm.debug_step(&mut session,StepKind::Continue).unwrap().0;
        if progress==rvn_core::engine::DebugProgress::Stopped{
            let actual=session.snapshot().site.expect("actual callback stop");let authored=breakpoint.site.as_ref().unwrap();
            assert_eq!(actual.function,authored.function);assert_eq!(actual.route,authored.route);assert_eq!(actual.statement,authored.statement);assert!(actual.depth>0);
            stopped=true;break;
        }
        assert_ne!(progress,rvn_core::engine::DebugProgress::Complete,"callback stop missing");
    }
    assert!(stopped);assert_eq!(state(&warm),before);(warm,p,session,expected,draw)
}

#[test]
fn method0883_statement_callback_recaptures_after_cold_restore_without_rng_or_callback_replay(){
    let(warm,p,session,expected,draw)=stopped_statement_media0883("media-recapture");
    let capture_source0883=warm.debug_state_source().unwrap();
    let save=warm.capture_save15(&p,Some(&session),883,"genuine method callback".into()).unwrap();
    assert!(warm.accepts_debug_state_source(&capture_source0883),"capture changed the exact opaque source/anchor");
    assert!(warm.accepts_debug_session(&session),"capture invalidated its retained real session");
    let mut cold=Engine::from_program(p.clone(),Host::default(),16).unwrap();
    let restored=cold.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&save.encode().unwrap()).unwrap()).unwrap().unwrap();
    assert_eq!(restored.snapshot().site,session.snapshot().site);assert!(cold.accepts_debug_session(&restored));
    let recapture_source0883=cold.debug_state_source().unwrap();
    let saved_again=cold.capture_save15(&p,Some(&restored),884,"actual rebound operation".into()).unwrap();
    assert!(cold.accepts_debug_state_source(&recapture_source0883),"capture changed the exact opaque source/anchor");
    assert!(cold.accepts_debug_session(&restored),"capture invalidated its retained real session");
    let mut second=Engine::from_program(p.clone(),Host::default(),16).unwrap();
    let mut resumed=second.restore_save15(&p,rvn_core::save::consumer_checkpoint::Save15::decode(&saved_again.encode().unwrap()).unwrap()).unwrap().unwrap();
    assert_eq!(resumed.snapshot().site,session.snapshot().site);
    let mut complete=false;
    for _ in 0..128{if second.debug_step(&mut resumed,StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{complete=true;break;}}
    assert!(complete);assert_eq!(second.state.vars["ends"],Value::Int(1));assert_eq!(second.state.vars["roll"],Value::Int(draw));assert_eq!(second.state.random,expected);
    assert_eq!(second.state.videos.tracks["intro"].playback,Playback::Ended);
}

#[test]
fn method0883_statement_media_permission_does_not_survive_real_terminal_publication(){
    let(mut warm,p,session,expected,draw)=stopped_statement_media0883("media-retired");
    let old_view=warm.video_views().unwrap().remove(0);
    // A real public operation publishes the terminal callback on this Engine.
    assert!(warm.skip_video_from_view(&old_view).unwrap());
    assert!(!warm.accepts_debug_session(&session));
    assert_eq!(warm.state.vars["ends"],Value::Int(1));assert_eq!(warm.state.vars["roll"],Value::Int(draw));assert_eq!(warm.state.random,expected);
    let before=state(&warm);let batches=count(&warm);let epoch=warm.runtime_epoch();
    let refused_source0883=warm.debug_state_source().unwrap();
    assert!(warm.capture_save15(&p,Some(&session),885,"retired stopped operation".into()).is_err());
    assert!(warm.accepts_debug_state_source(&refused_source0883));
    assert_eq!(state(&warm),before);assert_eq!(count(&warm),batches);assert_eq!(warm.runtime_epoch(),epoch);
    // Skip ends playback but keeps the actual video incarnation. The stopped
    // debugger session is stale; this live resource instead rejects Resume
    // with its typed terminal-state error, without publishing or replaying.
    assert!(!warm.skip_video_from_view(&old_view).unwrap());
    let error=warm.resume_video_from_view(&old_view).unwrap_err();
    assert_eq!(error.kind,rvn_core::error::RuntimeErrorKind::EvalError(
        rvn_core::error::EvalErrorKind::InvalidFunction(
            "A finished video needs video.play or video.seek before resume".into())));
    assert_eq!(error.pc,warm.state.pc);
    assert_eq!(error.stmt_debug,"Video resume");
    assert_eq!(warm.state.videos.tracks["intro"].playback,Playback::Ended);
    assert_eq!(state(&warm),before);assert_eq!(count(&warm),batches);assert_eq!(warm.runtime_epoch(),epoch);
    assert!(warm.accepts_debug_state_source(&refused_source0883));
    assert!(!warm.accepts_debug_session(&session));
}
