use std::{str::FromStr, sync::Arc};

use librespot::playback::player::PlayerEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;
use spotify::{SessionError, SpotifyPlayer};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use thiserror::Error;

use crate::{settings::Settings, spotify::SpotifySession};
mod oauth;
mod player_window;
mod playlist_window;
mod settings;
mod sink;
mod skins;
pub mod spotify;
mod visualizer;

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
enum StartError {
    #[error("Failed to create {window_name} window ({e:?}")]
    WindowCreationFailed {
        window_name: String,
        e: tauri::Error,
    },

    #[error("Failed to login ({e:?}")]
    LoginFailed { e: SessionError },
}

#[derive(Clone, Serialize)]
enum SpotiampPlayerEvent {
    Stopped { uri: String },
    Paused { uri: String, position_ms: u32 },
    EndOfTrack { uri: String },
    PositionCorrection { uri: String, position_ms: u32 },
    Seeked { uri: String, position_ms: u32 },
    TrackChanged { uri: String },
    Playing { uri: String, position_ms: u32 },
}

#[derive(Clone, Deserialize, Serialize)]
enum PlayerWindowEvent {
    CloseRequested,
    PlayPausePressed,
    StopPressed,
    NextPressed,
    PreviousPressed,
}

#[derive(Clone, Deserialize)]
enum PlaylistWindowEvent {
    Ready,
}

async fn start_app(app_handle: &AppHandle) -> Result<(), StartError> {
    let session = SpotifySession::default();
    session
        .login(app_handle)
        .await
        .map_err(|e| StartError::LoginFailed { e })?;

    let player_window =
        player_window::build_window(app_handle).map_err(|e| StartError::WindowCreationFailed {
            window_name: "Player".to_string(),
            e,
        })?;
    let player = Arc::new(tokio::sync::Mutex::new(SpotifyPlayer::new(session)));

    app_handle.manage(player.clone());
    tauri::async_runtime::spawn(async move {
        let mut channel = player.lock().await.get_player_event_channel();

        while let Some(player_event) = channel.recv().await {
            if let Some(player_event) = match player_event {
                PlayerEvent::Playing {
                    track_id,
                    position_ms,
                    ..
                } => Some(SpotiampPlayerEvent::Playing {
                    uri: track_id.to_uri().expect("a valid uri"),
                    position_ms,
                }),
                PlayerEvent::Stopped { track_id, .. } => Some(SpotiampPlayerEvent::Stopped {
                    uri: track_id.to_uri().expect("a valid uri"),
                }),
                PlayerEvent::Paused {
                    track_id,
                    position_ms,
                    ..
                } => Some(SpotiampPlayerEvent::Paused {
                    uri: track_id.to_uri().expect("a valid uri"),
                    position_ms,
                }),
                PlayerEvent::EndOfTrack { track_id, .. } => Some(SpotiampPlayerEvent::EndOfTrack {
                    uri: track_id.to_uri().expect("a valid uri"),
                }),
                PlayerEvent::PositionCorrection {
                    track_id,
                    position_ms,
                    ..
                } => Some(SpotiampPlayerEvent::PositionCorrection {
                    uri: track_id.to_uri().expect("a valid uri"),
                    position_ms,
                }),
                PlayerEvent::Seeked {
                    track_id,
                    position_ms,
                    ..
                } => Some(SpotiampPlayerEvent::Seeked {
                    uri: track_id.to_uri().expect("a valid uri"),
                    position_ms,
                }),
                PlayerEvent::TrackChanged { audio_item } => {
                    Some(SpotiampPlayerEvent::TrackChanged {
                        uri: audio_item.track_id.to_uri().expect("a valid uri"),
                    })
                }
                _ => None,
            } {
                let _ = player_window.emit("player", player_event);
            }
        }
    });

    Ok(())
}

fn register_media_shortcuts(app_handle: &AppHandle) {
    let shortcut_settings = Settings::current().shortcuts.clone();
    let shortcuts = [
        (
            "play_pause",
            shortcut_settings.play_pause,
            json!({ "PlayPausePressed": null }),
        ),
        (
            "stop",
            shortcut_settings.stop,
            json!({ "StopPressed": null }),
        ),
        (
            "next",
            shortcut_settings.next,
            json!({ "NextPressed": null }),
        ),
        (
            "previous",
            shortcut_settings.previous,
            json!({ "PreviousPressed": null }),
        ),
    ];

    for (name, shortcut, player_event) in shortcuts {
        let Some(shortcut) = shortcut else {
            log::debug!("Media shortcut '{name}' is disabled");
            continue;
        };

        let Ok(shortcut) = Shortcut::from_str(&shortcut) else {
            log::warn!("Could not parse media shortcut '{name}': '{shortcut}'");
            continue;
        };

        let event = player_event.clone();
        if let Err(e) = app_handle.global_shortcut().on_shortcut(
            shortcut,
            move |app, _shortcut, shortcut_event| {
                if shortcut_event.state == ShortcutState::Pressed {
                    let _ = app.emit("playerWindow", event.clone());
                }
            },
        ) {
            log::warn!("Could not register media shortcut '{name}': {:?}", e);
        };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            player_window::get_track_metadata,
            player_window::load_track,
            player_window::get_track_ids,
            player_window::play,
            player_window::pause,
            player_window::stop,
            player_window::get_player_settings,
            player_window::set_volume,
            player_window::set_double_size,
            player_window::take_latest_spectrum,
            player_window::seek,
            player_window::set_playlist_window_visible,
            playlist_window::get_playlist_settings,
            playlist_window::add_uri,
            playlist_window::set_playlist_inner_size,
            skins::get_skin_library,
            skins::set_current_skin,
            skins::import_skin_folder,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            app_handle.listen("playerWindow", move |event| {
                match serde_json::from_str::<PlayerWindowEvent>(event.payload()) {
                    Ok(e) => match e {
                        PlayerWindowEvent::CloseRequested => {
                            std::process::exit(0);
                        }
                        PlayerWindowEvent::PlayPausePressed
                        | PlayerWindowEvent::StopPressed
                        | PlayerWindowEvent::NextPressed
                        | PlayerWindowEvent::PreviousPressed => {}
                    },
                    Err(e) => log::debug!(
                        "Could not deserialize playlistWindow event: '{:?}' ({e:?}) - ignoring",
                        event.payload()
                    ),
                }
            });
            register_media_shortcuts(&app_handle);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_app(&app_handle).await {
                    log::error!("Failed to start ({e:?})");
                    app_handle.exit(1);
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
