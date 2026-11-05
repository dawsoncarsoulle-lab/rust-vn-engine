//! Typed literal-reference admission before Init. Dynamic argument evaluation
//! stays in the existing evaluator; this visitor never executes authored code.
use super::*;
use rvn_parser::{Expr,Statement,TextSegment,InterpolatedText};
enum Work<'a>{Statement(&'a Statement),Expression(&'a Expr),Text(&'a InterpolatedText)}
pub(super) fn validate(script:&[Statement],resources:&crate::enum_runtime_resources1025::EnumAssets1025)->EvalResult<()>{
    let mut pending=script.iter().rev().map(Work::Statement).collect::<Vec<_>>();
    while let Some(work)=pending.pop(){match work{
        Work::Text(text)=>for part in text.0.iter().rev(){if let TextSegment::Interp(expr)=part{pending.push(Work::Expression(expr));}},
        Work::Expression(expr)=>{
            if let Expr::Call{name,args}=expr{if name==CONSTRUCTOR{
                if args.len()!=2{return Err(EvalError::InvalidFunction("Literal Enum constructor requires two explicit argument slots before Init".into()));}
                if let [Expr::Str(asset),Expr::Str(member)]=args.as_slice(){
                    let asset=EnumAssetId::parse(asset).map_err(EvalError::InvalidFunction)?;let member=EnumMemberId::parse(member).map_err(EvalError::InvalidFunction)?;
                    resources.value(asset,member).map_err(|message|EvalError::InvalidFunction(format!("Literal Enum reference refused before Init: {message}")))?;
                }
            }}
            match expr{
                Expr::LibraryCall{call}=>pending.extend(call.arguments().iter().rev().map(|(_,value)|Work::Expression(value))),
                Expr::Call{args,..}|Expr::ListLit(args)=>pending.extend(args.iter().rev().map(Work::Expression)),
                Expr::BinOp{left,right,..}|Expr::And(left,right)|Expr::Or(left,right)=>{pending.push(Work::Expression(right));pending.push(Work::Expression(left));},
                Expr::Index{target,index}=>{pending.push(Work::Expression(index));pending.push(Work::Expression(target));},
                Expr::Neg(value)|Expr::Not(value)=>pending.push(Work::Expression(value)),
                // Site endpoints are nominal metadata, without expression operands.
                Expr::StandardMacroData{..}|Expr::Int(_)|Expr::Float(_)|Expr::Bool(_)|Expr::Str(_)|Expr::Var(_)=>{},
            }
        },
        Work::Statement(statement)=>match statement{
            Statement::FunctionLibrary{scope}=>pending.extend(scope.members().iter().rev().map(|member|Work::Statement(member.declaration()))),
            Statement::LibraryUnitCall{call}=>pending.extend(call.arguments().iter().rev().map(|(_,value)|Work::Expression(value))),
            Statement::Init{body}|Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}=>pending.extend(body.iter().rev().map(Work::Statement)),
            Statement::ScheduledConsumer{steps,body,..}=>{
                pending.extend(body.iter().rev().map(Work::Statement));for step in steps.iter().rev(){match step{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>pending.push(Work::Expression(value))}}
            },
            Statement::StandardMacroDispatch{dispatch}=>{
                for branch in dispatch.branches.iter().rev(){pending.extend(branch.body.iter().rev().map(Work::Statement));}
                pending.extend(dispatch.operand_expressions().into_iter().rev().map(Work::Expression));
            },
            Statement::SetVar{value,..}|Statement::LocalVar{value,..}|Statement::FunctionReturn{value}=>pending.push(Work::Expression(value)),
            Statement::UnitCall{arguments,..}=>pending.extend(arguments.iter().rev().map(Work::Expression)),
            Statement::If{condition,then_branch,else_branch}=>{pending.extend(else_branch.iter().rev().map(Work::Statement));pending.extend(then_branch.iter().rev().map(Work::Statement));pending.push(Work::Expression(condition));},
            Statement::Sequence{branches}=>for branch in branches.iter().rev(){pending.extend(branch.iter().rev().map(Work::Statement));},
            Statement::While{condition,body}=>{pending.extend(body.iter().rev().map(Work::Statement));pending.push(Work::Expression(condition));},
            Statement::ForEach{collection,body,..}=>{pending.extend(body.iter().rev().map(Work::Statement));pending.push(Work::Expression(collection));},
            Statement::ForLoopWithBreak{first,last,body,..}=>{pending.extend(body.iter().rev().map(Work::Statement));pending.push(Work::Expression(last));pending.push(Work::Expression(first));},
            Statement::UiOpen{name,arguments,modal,layer,..}=>for expr in [layer,modal,arguments,name]{pending.push(Work::Expression(expr));},
            Statement::UiSetState{name,element,state}=>for expr in [state,element,name]{pending.push(Work::Expression(expr));},
            Statement::UiFocus{name,element}=>for expr in [element,name]{pending.push(Work::Expression(expr));},
            Statement::WidgetAddToViewport{object,z_order}=>for expr in [z_order,object]{pending.push(Work::Expression(expr));},
            Statement::WidgetRemoveFromParent{object}=>pending.push(Work::Expression(object)),
            Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}|Statement::VideoWait{name}=>pending.push(Work::Expression(name)),
            Statement::MenuExecute{request}=>pending.push(Work::Expression(request)),
            Statement::MotionPlay{target,definition}=>for expr in [definition,target]{pending.push(Work::Expression(expr));},
            Statement::MotionStop{target}|Statement::MotionWait{target}=>pending.push(Work::Expression(target)),
            Statement::CharacterCompose{character,definition}=>for expr in [definition,character]{pending.push(Work::Expression(expr));},
            Statement::CharacterAttributes{character,attributes}=>for expr in [attributes,character]{pending.push(Work::Expression(expr));},
            Statement::VideoPlay{name,definition}=>for expr in [definition,name]{pending.push(Work::Expression(expr));},
            Statement::VideoSeek{name,seconds}=>for expr in [seconds,name]{pending.push(Work::Expression(expr));},
            Statement::VideoVolume{name,volume}=>for expr in [volume,name]{pending.push(Work::Expression(expr));},
            Statement::AccessibilityConfigure{settings}=>pending.push(Work::Expression(settings)),
            Statement::AccessibilitySpeak{text}=>pending.push(Work::Expression(text)),
            Statement::MusicVolumeValue{level}=>pending.push(Work::Expression(level)),
            Statement::TypewriterSetValue{enabled}=>pending.push(Work::Expression(enabled)),
            Statement::TypewriterSpeedValue{chars_per_sec}=>pending.push(Work::Expression(chars_per_sec)),
            Statement::Dialogue{text,..}=>pending.push(Work::Text(text)),
            Statement::Choice{options}=>for option in options.iter().rev(){pending.extend(option.body.iter().rev().map(Work::Statement));if let Some(expr)=&option.condition{pending.push(Work::Expression(expr));}pending.push(Work::Text(&option.label));},
            Statement::Imagemap{hotspots,..}=>for hotspot in hotspots.iter().rev(){pending.extend(hotspot.body.iter().rev().map(Work::Statement));},
            Statement::Use{..}|Statement::UnitReturn|Statement::LoopBreak|Statement::AccessibilityStop|Statement::Config{..}|Statement::CharacterCreate{..}|Statement::Label{..}|Statement::SceneLabel{..}|Statement::Jump{..}|Statement::Call{..}|Statement::Return|Statement::Scene{..}|Statement::CinematicShow{..}|Statement::CinematicHide{..}|Statement::UnlockEnding{..}|Statement::ShowSprite{..}|Statement::HideSprite{..}|Statement::MoveSprite{..}|Statement::SpriteAnimate{..}|Statement::SpriteStopAnimation{..}|Statement::SpriteEffect{..}|Statement::Timer{..}|Statement::TimerCancel|Statement::MethodCall{..}|Statement::MusicPlay{..}|Statement::MusicStop{..}|Statement::MusicVolume{..}|Statement::SfxPlay{..}|Statement::SfxStop{..}|Statement::VoicePlay{..}|Statement::VoiceStop|Statement::TypewriterSet{..}|Statement::TypewriterSpeed{..}=>{},
        },
    }}Ok(())
}
