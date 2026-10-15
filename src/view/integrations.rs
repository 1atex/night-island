use crate::view::*;

#[derive(Default)]
pub struct IntegrationsUi {
    pub id: TextEdit,
    pub secret: TextEdit,
    pub spotify_id: TextEdit,
}

pub fn discord_fields_ok(ui: &UiState) -> bool {
    crate::services::discord::credentials_look_valid(&ui.int.id.text(), &ui.int.secret.text())
}
