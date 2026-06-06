use crate::core::error::{Result, TingError};
use crate::db::manager::DatabaseManager;
use crate::db::models::UserSettings;
use rusqlite::OptionalExtension;
use std::sync::Arc;

/// Repository for UserSettings entities
pub struct UserSettingsRepository {
    db: Arc<DatabaseManager>,
}

impl UserSettingsRepository {
    /// Create a new UserSettingsRepository
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }

    /// Get settings for a user
    pub async fn get_by_user(&self, user_id: &str) -> Result<Option<UserSettings>> {
        let user_id = user_id.to_string();
        self.db.execute(move |conn| {
            conn.query_row(
                "SELECT user_id, playback_speed, theme, auto_play, skip_intro, skip_outro, settings_json, updated_at \
                 FROM user_settings WHERE user_id = ?",
                [&user_id],
                |row| {
                    Ok(UserSettings {
                        user_id: row.get(0)?,
                        playback_speed: row.get(1)?,
                        theme: row.get(2)?,
                        auto_play: row.get(3)?,
                        skip_intro: row.get(4)?,
                        skip_outro: row.get(5)?,
                        settings_json: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                }
            ).optional()
            .map_err(TingError::DatabaseError)
        }).await
    }

    /// Upsert user settings
    pub async fn upsert(&self, settings: &UserSettings) -> Result<()> {
        let settings = settings.clone();
        self.db.execute(move |conn| {
            conn.execute(
                "INSERT INTO user_settings (user_id, playback_speed, theme, auto_play, skip_intro, skip_outro, settings_json, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                 ON CONFLICT(user_id) DO UPDATE SET \
                 playback_speed = excluded.playback_speed, \
                 theme = excluded.theme, \
                 auto_play = excluded.auto_play, \
                 skip_intro = excluded.skip_intro, \
                 skip_outro = excluded.skip_outro, \
                 settings_json = excluded.settings_json, \
                 updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')",
                rusqlite::params![
                    &settings.user_id,
                    settings.playback_speed,
                    &settings.theme,
                    settings.auto_play,
                    settings.skip_intro,
                    settings.skip_outro,
                    &settings.settings_json,
                ],
            ).map_err(TingError::DatabaseError)?;
            Ok(())
        }).await
    }
}
