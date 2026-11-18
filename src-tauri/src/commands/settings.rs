use crate::{
    credentials::{CredentialStatus, ProviderProfileWrite, ProviderProfilesDto},
    db::Database,
    error::AppError,
    models::*,
    providers,
};
use tauri::State;

#[tauri::command]
pub(crate) fn save_api_key(db: State<'_, Database>, key: String) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_api_key(db.inner(), key)
}

#[tauri::command]
pub(crate) fn get_api_key(db: State<'_, Database>) -> Result<CredentialStatus, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_api_key(db.inner())
}

#[tauri::command]
pub(crate) fn save_base_url(db: State<'_, Database>, url: String) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_base_url(db.inner(), url)
}

#[tauri::command]
pub(crate) fn get_base_url(db: State<'_, Database>) -> Result<String, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_base_url(db.inner())
}

#[tauri::command]
pub(crate) fn get_endpoint_settings(db: State<'_, Database>) -> Result<EndpointSettings, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_endpoint_settings(db.inner())
}

#[tauri::command]
pub(crate) fn save_endpoint_settings(
    db: State<'_, Database>,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_endpoint_settings(db.inner(), mode, base_url, generation_url, edit_url)
}

#[tauri::command]
pub(crate) fn get_model_api_key(
    db: State<'_, Database>,
    model: String,
) -> Result<CredentialStatus, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_model_api_key(db.inner(), model)
}

#[tauri::command]
pub(crate) fn save_model_api_key(
    db: State<'_, Database>,
    model: String,
    key: String,
) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_model_api_key(db.inner(), model, key)
}

#[tauri::command]
pub(crate) fn get_model_endpoint_settings(
    db: State<'_, Database>,
    model: String,
) -> Result<EndpointSettings, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_model_endpoint_settings(db.inner(), model)
}

#[tauri::command]
pub(crate) fn save_model_endpoint_settings(
    db: State<'_, Database>,
    model: String,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_model_endpoint_settings(
        db.inner(),
        model,
        mode,
        base_url,
        generation_url,
        edit_url,
    )
}

#[tauri::command]
pub(crate) fn get_model_provider_profiles(
    db: State<'_, Database>,
    model: String,
) -> Result<ProviderProfilesDto, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_model_provider_profiles(db.inner(), model)
}

#[tauri::command]
pub(crate) fn save_model_provider_profiles(
    db: State<'_, Database>,
    model: String,
    active_provider_id: String,
    profiles: Vec<ProviderProfileWrite>,
) -> Result<ProviderProfilesDto, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_model_provider_profiles(db.inner(), model, active_provider_id, profiles)
}

#[tauri::command]
pub(crate) fn create_model_provider_profile(
    db: State<'_, Database>,
    model: String,
    name: String,
) -> Result<ProviderProfilesDto, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::create_model_provider_profile(db.inner(), model, name)
}

#[tauri::command]
pub(crate) fn delete_model_provider_profile(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::delete_model_provider_profile(db.inner(), model, provider_id)
}

#[tauri::command]
pub(crate) fn set_active_model_provider(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::set_active_model_provider(db.inner(), model, provider_id)
}

#[tauri::command]
pub(crate) fn get_font_size(db: State<'_, Database>) -> Result<String, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    Ok(crate::normalize_font_size(
        db.get_setting(SETTING_FONT_SIZE)?
            .as_deref()
            .unwrap_or(DEFAULT_FONT_SIZE),
    )
    .to_string())
}

#[tauri::command]
pub(crate) fn save_font_size(db: State<'_, Database>, font_size: String) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    db.set_setting(SETTING_FONT_SIZE, crate::normalize_font_size(&font_size))
}

#[tauri::command]
pub(crate) fn get_image_model(db: State<'_, Database>) -> Result<String, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::get_image_model(db.inner())
}

#[tauri::command]
pub(crate) fn save_image_model(db: State<'_, Database>, model: String) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    providers::save_image_model(db.inner(), model)
}
