use crate::view::*;

#[derive(Default)]
pub struct IntegrationsUi {
    pub id: TextEdit,
    pub secret: TextEdit,
    pub spotify_id: TextEdit,
    pub ai_key: TextEdit,
    pub ai_model: TextEdit,
    pub ai_base: TextEdit,
    pub ai_key_saved: bool,
    pub voice_key: TextEdit,
    pub voice_model: TextEdit,
    pub voice_base: TextEdit,
    pub voice_key_saved: bool,
}

pub fn discord_fields_ok(ui: &UiState) -> bool {
    crate::services::discord::credentials_look_valid(&ui.int.id.text(), &ui.int.secret.text())
}
