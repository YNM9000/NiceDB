use std::sync::Arc;

use nd_core::Core;
use nd_store::{GenProfiles, GenSaved, Marks, NoteScope, Preferences, SavedNote, Script, Snippet};
use tauri::State;

use crate::error::CommandError;

type Ctx<'a> = State<'a, Arc<Core>>;
type Res<T> = Result<T, CommandError>;

#[tauri::command]
#[specta::specta]
pub fn scripts_list(core: Ctx<'_>) -> Res<Vec<Script>> {
    core.store(nd_store::Store::scripts)
}

#[tauri::command]
#[specta::specta]
pub fn script_save(core: Ctx<'_>, script: Script) -> Res<Script> {
    core.store(|s| s.save_script(&script))
}

#[tauri::command]
#[specta::specta]
pub fn script_delete(core: Ctx<'_>, id: String) -> Res<()> {
    core.store(|s| s.delete_script(&id))
}

#[tauri::command]
#[specta::specta]
pub fn snippets_list(core: Ctx<'_>) -> Res<Vec<Snippet>> {
    core.store(nd_store::Store::snippets)
}

#[tauri::command]
#[specta::specta]
pub fn snippet_save(core: Ctx<'_>, snippet: Snippet) -> Res<()> {
    core.store(|s| s.save_snippet(&snippet))
}

#[tauri::command]
#[specta::specta]
pub fn snippet_delete(core: Ctx<'_>, id: String) -> Res<()> {
    core.store(|s| s.delete_snippet(&id))
}

#[tauri::command]
#[specta::specta]
pub fn notes_list(core: Ctx<'_>) -> Res<Vec<SavedNote>> {
    core.store(nd_store::Store::notes)
}

#[tauri::command]
#[specta::specta]
pub fn note_set(
    core: Ctx<'_>,
    scope: NoteScope,
    scope_id: String,
    body: String,
) -> Res<Option<SavedNote>> {
    core.store(|s| s.set_note(scope, &scope_id, &body))
}

#[tauri::command]
#[specta::specta]
pub fn preferences_get(core: Ctx<'_>) -> Res<Preferences> {
    core.store(nd_store::Store::preferences)
}

#[tauri::command]
#[specta::specta]
pub fn preferences_set(
    core: Ctx<'_>,
    value: Preferences,
    confirm: Option<String>,
) -> Res<Preferences> {
    core.set_preferences(&value, confirm.as_deref())
}

#[tauri::command]
#[specta::specta]
pub fn marks_get(core: Ctx<'_>) -> Res<Marks> {
    core.store(nd_store::Store::marks)
}

#[tauri::command]
#[specta::specta]
pub fn marks_set(core: Ctx<'_>, value: Marks) -> Res<Marks> {
    core.store(|s| s.set_marks(&value))
}

#[tauri::command]
#[specta::specta]
pub fn gen_profiles_get(core: Ctx<'_>) -> Res<GenProfiles> {
    core.store(nd_store::Store::gen_profiles)
}

#[tauri::command]
#[specta::specta]
pub fn gen_profile_save(core: Ctx<'_>, profile: GenSaved) -> Res<GenProfiles> {
    core.store(|s| s.save_gen_profile(&profile))
}

#[tauri::command]
#[specta::specta]
pub fn gen_profile_delete(core: Ctx<'_>, id: String) -> Res<GenProfiles> {
    core.store(|s| s.delete_gen_profile(&id))
}
