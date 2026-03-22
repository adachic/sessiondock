mod session;
mod terminal;

use session::{Session, SessionManager, SessionStatus};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

struct PendingNotify {
    started_at: std::time::Instant,
    sent_3s: bool,
    sent_10s: bool,
}

struct AppState {
    session_manager: Mutex<SessionManager>,
    previous_sessions: Mutex<Vec<Session>>,
    pending_notify: Mutex<HashMap<String, PendingNotify>>,
}

#[tauri::command]
fn get_sessions(state: tauri::State<AppState>) -> Vec<Session> {
    let mut manager = state.session_manager.lock().unwrap();
    manager.get_sessions()
}

#[tauri::command]
fn focus_terminal(pid: u32, cwd: String) -> Result<String, String> {
    terminal::activate_terminal_for_pid(pid, &cwd)
}

fn update_tray_title(app: &AppHandle, sessions: &[Session]) {
    let active_count = sessions
        .iter()
        .filter(|s| matches!(s.status, SessionStatus::Running))
        .count();
    let waiting_count = sessions
        .iter()
        .filter(|s| matches!(s.status, SessionStatus::Waiting))
        .count();

    let title = if active_count > 0 {
        format!("SD:▶{}", active_count)
    } else if waiting_count > 0 {
        format!("SD:⏸{}", waiting_count)
    } else {
        "SD".to_string()
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_title(Some(&title));
    }
}

fn check_status_changes(
    app: &AppHandle,
    current: &[Session],
    previous: &[Session],
    pending: &mut HashMap<String, PendingNotify>,
) {
    let now = std::time::Instant::now();

    for curr in current {
        let prev_status = previous
            .iter()
            .find(|s| s.session_id == curr.session_id)
            .map(|s| &s.status);

        match (&curr.status, prev_status) {
            // Running → Waiting: タイマー開始
            (SessionStatus::Waiting, Some(SessionStatus::Running)) => {
                pending.insert(
                    curr.session_id.clone(),
                    PendingNotify {
                        started_at: now,
                        sent_3s: false,
                        sent_10s: false,
                    },
                );
            }
            // Waiting → Running: タイマーキャンセル
            (SessionStatus::Running, Some(SessionStatus::Waiting)) => {
                pending.remove(&curr.session_id);
            }
            // → Done: 即通知
            (SessionStatus::Done, Some(SessionStatus::Running))
            | (SessionStatus::Done, Some(SessionStatus::Waiting)) => {
                pending.remove(&curr.session_id);
                let body = format!(
                    "{} completed ({}min, ${:.2})",
                    curr.project_name,
                    curr.elapsed_seconds / 60,
                    curr.estimated_cost
                );
                send_notification(app, &body);
                play_sound_soft();
            }
            _ => {}
        }
    }

    // 3秒後: 軽い通知音（Glass）
    // 10秒後: はっきりした通知音（Ping）+ macOS通知
    let ids: Vec<String> = pending.keys().cloned().collect();
    for id in ids {
        let entry = pending.get_mut(&id).unwrap();
        let elapsed = now.duration_since(entry.started_at).as_secs();

        // まだWaitingかどうか確認
        let still_waiting = current
            .iter()
            .find(|s| s.session_id == id)
            .map(|s| matches!(s.status, SessionStatus::Waiting))
            .unwrap_or(false);

        if !still_waiting {
            pending.remove(&id);
            continue;
        }

        if elapsed >= 10 && !entry.sent_10s {
            entry.sent_10s = true;
            play_sound_soft();
        }
    }
}

fn send_notification(app: &AppHandle, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("SessionDock")
        .body(body)
        .show();
}

// 3秒後: 軽い音 (Glass)
fn play_sound_soft() {
    std::thread::spawn(|| {
        let _ = std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    });
}

// 10秒後: はっきりした音 (Ping)
fn play_sound_strong() {
    std::thread::spawn(|| {
        let _ = std::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Ping.aiff")
            .spawn();
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState {
            session_manager: Mutex::new(SessionManager::new()),
            previous_sessions: Mutex::new(Vec::new()),
            pending_notify: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![get_sessions, focus_terminal])
        .setup(|app| {
            let quit = MenuItemBuilder::with_id("quit", "Quit SessionDock").build(app)?;
            let show = MenuItemBuilder::with_id("show", "Show Dashboard").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .title("SD")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            // Polling: 1秒間隔（3秒タイマーの精度のため）
            let app_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));

                let state = app_handle.state::<AppState>();
                let sessions = {
                    let mut manager = state.session_manager.lock().unwrap();
                    manager.get_sessions()
                };

                {
                    let previous = state.previous_sessions.lock().unwrap();
                    let mut pending = state.pending_notify.lock().unwrap();
                    check_status_changes(&app_handle, &sessions, &previous, &mut pending);
                }

                update_tray_title(&app_handle, &sessions);
                let _ = app_handle.emit("sessions-updated", &sessions);

                {
                    let mut previous = state.previous_sessions.lock().unwrap();
                    *previous = sessions;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SessionDock");
}
