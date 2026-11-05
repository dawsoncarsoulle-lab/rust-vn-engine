//! Grammar/shape tests only. Parser admission confers no Core owner/dispatch.
use rvn_parser::{parse,scheduled_consumer_body_supported,scheduled_consumer_kind,ClassicConsumerKind as C,ScheduledConsumerKind as K,Statement};

fn classic(source:&str,kind:C) {
    let raw=parse(source).expect(source);assert_eq!(raw.len(),1, "{source}");
    assert_eq!(scheduled_consumer_kind(&raw[0]),Some(K::Classic(kind)));
    let wrapped=format!("flow.consume_v2(\"scope\",\"consumer\"){{{source}}}");
    let parsed=parse(&wrapped).expect(&wrapped);
    let [Statement::ScheduledConsumer{version,body,..}]=parsed.as_slice() else {panic!("{wrapped}")};
    assert_eq!(*version,2);assert_eq!(body,&raw);assert!(scheduled_consumer_body_supported(2,body));
    assert!(!scheduled_consumer_body_supported(1,body));
    assert!(parse(&format!("flow.consume_v1(\"scope\",\"consumer\"){{{source}}}")).is_err());
}
macro_rules! case {($name:ident,$kind:ident,$source:expr)=>{#[test]fn $name(){classic($source,C::$kind);}}}
case!(classic_config,Config,"bg: \"garden.png\"");
case!(classic_character_create,CharacterCreate,"character.create(\"hero\",\"Hero\")");
case!(classic_background,Background,"scene \"garden.png\"");
case!(classic_cinematic_show,CinematicShow,"cinematic \"cg.png\"");
case!(classic_cinematic_hide,CinematicHide,"cinematic hide");
case!(classic_unlock_ending,UnlockEnding,"unlock_ending \"ending\"");
case!(classic_sprite_show,SpriteShow,"hero.show(\"hero.png\") at center");
case!(classic_sprite_hide,SpriteHide,"hero.hide()");
case!(classic_sprite_move,SpriteMove,"hero.move() at left");
case!(classic_sprite_animate,SpriteAnimate,"hero.animate(\"bounce\")");
case!(classic_sprite_stop_animation,SpriteStopAnimation,"hero.stop_animation()");
case!(classic_sprite_effect,SpriteEffect,"hero.effect(scale:1.25,flip_x:true)");
case!(classic_timer,Timer,"timer 0.25 => jump start");
case!(classic_timer_cancel,TimerCancel,"timer cancel");
case!(classic_method_call,MethodCall,"hero.custom(\"value\")");
case!(classic_imagemap,Imagemap,"imagemap {background: \"garden.png\" hotspot {name: \"act\" area: (0,0,100,100)} => {set chosen=1}}");
case!(classic_typewriter_set,TypewriterSet,"typewriter=true");
case!(classic_typewriter_speed,TypewriterSpeed,"typewriter.speed(20)");

#[test]
fn branch_end_stays_zero_step_and_multiple_consumers_are_not_grammar() {
    assert!(parse("flow.consume_v2(\"s\",\"c\"){}").is_ok());
    assert!(parse("flow.consume_v2(\"s\",\"c\"){capture \"p\"=1}").is_err());
    assert!(parse("flow.consume_v2(\"s\",\"c\"){typewriter=true typewriter.speed(20)}").is_err());
    assert!(!scheduled_consumer_body_supported(99,&[]));
}
#[test]
fn computation_context_refusal_and_literal_typewriter_input_stay_strict() {
    assert!(parse("function invalid(){flow.consume_v2(\"s\",\"c\"){typewriter.speed(20)}}").is_err());
    let parsed = parse("flow.consume_v2(\"s\",\"c\"){typewriter=enabled}").expect("typed Bool expression belongs to the v2 classic consumer");
    let [Statement::ScheduledConsumer { version, steps, body, .. }] = parsed.as_slice() else { panic!("expected the genuine v2 consumer AST") };
    assert_eq!(*version, 2);
    assert!(steps.is_empty());
    assert!(matches!(body.as_slice(), [Statement::TypewriterSetValue { enabled: rvn_parser::Expr::Var(name) }] if name == "enabled"));
    assert!(scheduled_consumer_body_supported(2, body));
    assert_eq!(scheduled_consumer_kind(&body[0]), Some(K::Classic(C::TypewriterSet)));
    assert!(parse("flow.consume_v1(\"s\",\"c\"){typewriter=enabled}").is_err());
    assert!(parse("flow.consume_v2(\"s\",\"c\"){flow.break()}").is_err());
}
#[test]
fn existing_audio_control_and_value_shapes_keep_their_classification() {
    for (source,kind) in [("music.volume(\"0.25\")",K::Audio),("video.wait(\"clip\")",K::Video),("set x=3",K::Value),("if true {set x=1}",K::Control)] {
        let body=parse(source).unwrap();assert_eq!(scheduled_consumer_kind(&body[0]),Some(kind));
        assert!(parse(&format!("flow.consume_v2(\"s\",\"c\"){{{source}}}")).is_ok());
    }
}
