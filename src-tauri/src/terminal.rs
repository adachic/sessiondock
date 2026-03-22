use std::process::Command;

/// PIDからプロセスツリーを辿り、ターミナルアプリを特定してアクティブにする
pub fn activate_terminal_for_pid(pid: u32) -> Result<String, String> {
    // 1. TTYを取得
    let tty = get_tty(pid);

    // 2. プロセスツリーを辿ってターミナルアプリを特定
    let terminal = find_terminal_app(pid)?;

    // 3. ターミナルの種類に応じてアクティブ化
    match terminal.as_str() {
        "Terminal" => activate_terminal_app(&tty),
        "iTerm2" => activate_iterm2(&tty),
        "ghostty" => activate_ghostty(&tty),
        "Code" | "Visual Studio Code" | "Cursor" => activate_electron_app(&terminal),
        _ => activate_generic(&terminal),
    }
}

/// PIDからTTYデバイスを取得
fn get_tty(pid: u32) -> String {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok();

    if let Some(out) = output {
        let tty = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !tty.is_empty() && tty != "??" {
            return format!("/dev/{}", tty);
        }
    }
    String::new()
}

/// プロセスツリーを辿ってターミナルアプリ名を特定
fn find_terminal_app(pid: u32) -> Result<String, String> {
    let known_terminals = [
        "Terminal",
        "iTerm2",
        "ghostty",
        "Code",
        "Cursor",
        "Visual Studio Code",
        "Warp",
        "Hyper",
        "Alacritty",
        "kitty",
        "WezTerm",
    ];

    let mut current_pid = pid;
    let mut depth = 0;

    loop {
        if depth > 20 || current_pid <= 1 {
            break;
        }

        let output = Command::new("ps")
            .args(["-o", "ppid=,comm=", "-p", &current_pid.to_string()])
            .output()
            .map_err(|e| format!("ps failed: {}", e))?;

        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if line.is_empty() {
            break;
        }

        // ppidとcommを分離（ppidは先頭の数字）
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            break;
        }

        let ppid: u32 = parts[0].trim().parse().unwrap_or(0);
        let comm = parts[1].trim();

        // commからアプリ名を抽出（パスの最後の部分）
        let app_name = comm
            .rsplit('/')
            .next()
            .unwrap_or(comm)
            .trim_end_matches(".app");

        for &term in &known_terminals {
            if app_name.contains(term) {
                return Ok(term.to_string());
            }
        }

        // "Code Helper" → VS Code
        if app_name.contains("Code Helper") || app_name.contains("Electron") {
            // もう1段上がればVS Codeが見つかるかも
            current_pid = ppid;
            depth += 1;
            continue;
        }

        current_pid = ppid;
        depth += 1;
    }

    // 見つからなかった場合、直近の親プロセスのアプリ名を返す
    Err("Terminal app not found in process tree".to_string())
}

/// Terminal.app: TTYでタブを特定してアクティブ化
fn activate_terminal_app(tty: &str) -> Result<String, String> {
    if tty.is_empty() {
        return activate_generic("Terminal");
    }

    let script = format!(
        r#"
        tell application "Terminal"
            repeat with w in every window
                repeat with t in every tab of w
                    if tty of t is "{}" then
                        set index of w to 1
                        set selected tab of w to t
                        activate
                        return "ok"
                    end if
                end repeat
            end repeat
            activate
        end tell
        "#,
        tty
    );
    run_osascript(&script)
}

/// iTerm2: TTYでセッションを特定してアクティブ化
fn activate_iterm2(tty: &str) -> Result<String, String> {
    if tty.is_empty() {
        return activate_generic("iTerm2");
    }

    let script = format!(
        r#"
        tell application "iTerm2"
            repeat with w in every window
                repeat with t in every tab of w
                    repeat with s in every session of t
                        if (tty of s) is "{}" then
                            select w
                            tell t to select
                            activate
                            return "ok"
                        end if
                    end repeat
                end repeat
            end repeat
            activate
        end tell
        "#,
        tty
    );
    run_osascript(&script)
}

/// Ghostty: TTYでウィンドウを特定してアクティブ化
fn activate_ghostty(tty: &str) -> Result<String, String> {
    if tty.is_empty() {
        return activate_generic("Ghostty");
    }

    // Ghostty v1.3.0+ はAppleScript対応だが、TTY照合のAPIは要確認
    // まずはアプリレベルのアクティブ化
    activate_generic("Ghostty")
}

/// VS Code / Cursor: アプリ全体をアクティブ化
fn activate_electron_app(app_name: &str) -> Result<String, String> {
    let actual_name = match app_name {
        "Code" => "Visual Studio Code",
        _ => app_name,
    };
    activate_generic(actual_name)
}

/// 汎用: アプリ名でアクティブ化
fn activate_generic(app_name: &str) -> Result<String, String> {
    let script = format!(
        r#"tell application "{}" to activate"#,
        app_name
    );
    run_osascript(&script)
}

fn run_osascript(script: &str) -> Result<String, String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|e| format!("osascript failed: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // エラーでもアプリは起動できている場合がある
        if err.contains("not running") {
            Err(format!("App not running: {}", err))
        } else {
            Ok(format!("activated (with warning: {})", err))
        }
    }
}
