use super::*;
use crate::components::{
    CharacterNameText, ChoiceButton, ChoiceContainer, DialogueBox, DialogueText,
};
use bevy::ecs::schedule::ExecutorKind;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct CapturedWarnings(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedWarnings {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn two_choices_to_dialogue_removes_each_button_once() {
    let warnings = CapturedWarnings::default();
    let writer = warnings.clone();
    let subscriber = bevy::log::tracing_subscriber::fmt()
        .with_max_level(bevy::log::Level::WARN)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();

    // Keep tracing scoped to this test and Commands on this thread. No render,
    // audio, window, font loading, project files or global logger are needed.
    bevy::utils::tracing::subscriber::with_default(subscriber, || {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_event::<VnCommand>();
        app.init_resource::<VnRenderState>();
        app.init_resource::<Theme>();
        app.init_resource::<ChoiceFocus>();
        app.init_resource::<menu_documents::Menus>();
        app.init_resource::<CharacterRegistry>();
        app.init_resource::<DialogueHistory>();
        app.init_resource::<TypewriterState>();
        app.insert_resource(TypewriterConfig {
            enabled: false,
            chars_per_sec: 0.0,
        });
        app.world_mut()
            .resource_mut::<CharacterRegistry>()
            .0
            .insert("keeper".into(), "Gardienne".into());

        let container = app.world_mut().spawn(ChoiceContainer).id();
        let dialogue_box = app.world_mut().spawn((DialogueBox, Style::default())).id();
        let name = app
            .world_mut()
            .spawn((
                CharacterNameText,
                Text::from_section("", TextStyle::default()),
            ))
            .id();
        let dialogue = app
            .world_mut()
            .spawn((DialogueText, Text::from_section("", TextStyle::default())))
            .id();
        app.add_systems(
            Update,
            (
                dialogue_system,
                choice_system,
                choice_button_update_systems(),
            ),
        );
        app.edit_schedule(Update, |schedule| {
            schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        });

        app.world_mut().send_event(VnCommand::ShowChoice {
            options: vec!["Traverser".into(), "Attendre".into()],
        });
        app.update();

        let world = app.world_mut();
        let mut buttons: Vec<_> = world
            .query::<(Entity, &ChoiceButton)>()
            .iter(world)
            .map(|(entity, choice)| (choice.0, entity))
            .collect();
        buttons.sort_by_key(|(index, _)| *index);
        assert_eq!(buttons.len(), 2);
        let button_entities: Vec<_> = buttons.iter().map(|(_, entity)| *entity).collect();
        assert_eq!(world.get::<Children>(container).unwrap().len(), 2);
        let labels: Vec<_> = button_entities
            .iter()
            .flat_map(|entity| world.get::<Children>(*entity).unwrap().iter().copied())
            .collect();
        assert_eq!(labels.len(), 4, "each real choice has its number and label");

        // This is the render-state change made by PlayerInput::Choose, followed
        // by the next story command. Both cleanup systems run in this Update.
        world.resource_mut::<VnRenderState>().choice_options.clear();
        world.resource_mut::<ChoiceFocus>().0 = None;
        world.send_event(VnCommand::ShowDialogue {
            character: Some("keeper".into()),
            text: "Votre voyage continue.".into(),
        });
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&ChoiceButton>().iter(world).count(), 0);
        for entity in button_entities.iter().chain(&labels) {
            assert!(world.get_entity(*entity).is_none(), "recursive cleanup");
        }
        assert!(world.get_entity(container).is_some());
        assert!(world
            .get::<Children>(container)
            .is_none_or(|children| children.is_empty()));
        assert_eq!(
            world.get::<Style>(dialogue_box).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            world.get::<Text>(name).unwrap().sections[0].value,
            "Gardienne"
        );
        let shown: String = world
            .get::<Text>(dialogue)
            .unwrap()
            .sections
            .iter()
            .map(|section| section.value.as_str())
            .collect();
        assert_eq!(shown, "Votre voyage continue.");
        let history = world.resource::<DialogueHistory>();
        assert_eq!(history.lines.len(), 1);
        assert_eq!(history.lines[0].character, "Gardienne");
        assert_eq!(history.lines[0].text, shown);
    });

    let output = String::from_utf8(warnings.0.lock().unwrap().clone()).unwrap();
    assert!(
        !output.contains("B0003"),
        "both cleanup paths must not despawn the same choice: {output}"
    );
}
