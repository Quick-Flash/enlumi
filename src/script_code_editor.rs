use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_editor::{Document, EditorEvent, EditorHandle, EditorProps};

fn primary_shortcut(
    event: &zgui::vocab::KeyEvent,
    modifiers: zgui::vocab::Modifiers,
    macos: bool,
) -> Option<zgui_editor::Command> {
    use zgui::vocab::{Key, Modifiers};
    if !macos || !modifiers.meta() || modifiers.control() || modifiers.alt() {
        return None;
    }
    if !matches!(&event.key_without_modifiers, Key::Character(key) if matches!(key.as_str(), "a" | "c" | "v" | "x" | "z" | "y"))
    {
        return None;
    }
    let modifiers = modifiers
        .with(Modifiers::META, false)
        .with(Modifiers::CONTROL, true);
    zgui_editor::input::keymap::default_keymap(event, modifiers)
}

#[component]
pub fn ScriptCodeEditor(
    source: RwSignal<String, LocalStorage>,
    read_only: Signal<bool, LocalStorage>,
    #[prop(optional)] on_ready: Option<UnsyncCallback<EditorHandle>>,
) -> impl IntoView {
    let document = Document::new(&source.get_untracked());
    let text = StoredValue::new_local(document.clone());
    let handle = RwSignal::new_local(None::<EditorHandle>);
    let sync = RenderEffect::new(move |_| {
        let source = source.get();
        let read_only = read_only.get();
        if let Some(editor) = handle.get() {
            if text.with_value(Document::text) != source {
                editor.set_text(&source);
            }
            let mut options = editor.edit_options();
            if options.read_only != read_only {
                options.read_only = read_only;
                editor.set_edit_options(options);
            }
        }
    });
    on_cleanup_local(move || drop(sync));
    let ready: Box<dyn Fn(EditorHandle)> = Box::new(move |editor| {
        handle.set(Some(editor.clone()));
        if let Some(ready) = on_ready {
            ready.run(editor);
        }
    });
    let edited: Box<dyn Fn(EditorEvent)> = Box::new(move |event| {
        if matches!(event, EditorEvent::Edited { .. }) {
            let value = text.with_value(Document::text);
            if source.get_untracked() != value {
                source.set(value);
            }
        }
    });
    let key: zgui_editor::KeyFilter = Box::new(|event, modifiers, editor| {
        if let Some(command) = primary_shortcut(event, modifiers, cfg!(target_os = "macos")) {
            editor.command(command);
            true
        } else {
            false
        }
    });
    view! {
        column(class = "script-code-frame") {
        Editor(document = document, language = "rust", autofocus = false, class = "script-source", a11y:label = "Rust filter source", a11y:role = Role::MultilineTextInput,
            on_ready = ready, on_event = edited, on_key = key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zgui::vocab::{Key, KeyEvent, Modifiers, PhysicalKey};
    #[test]
    fn mac_command_shortcuts_use_editor_commands_without_changing_other_keys() {
        let event = KeyEvent {
            key: Key::character("z"),
            key_without_modifiers: Key::character("z"),
            physical: PhysicalKey::Unidentified(0),
            location: Default::default(),
            repeat: false,
        };
        assert!(matches!(
            primary_shortcut(&event, Modifiers::META, true),
            Some(zgui_editor::Command::Undo)
        ));
        assert!(matches!(
            primary_shortcut(&event, Modifiers::META | Modifiers::SHIFT, true),
            Some(zgui_editor::Command::Redo)
        ));
        assert!(primary_shortcut(&event, Modifiers::META, false).is_none());
        assert!(primary_shortcut(&event, Modifiers::NONE, true).is_none());
        assert!(primary_shortcut(&event, Modifiers::META | Modifiers::ALT, true).is_none());
    }
}
