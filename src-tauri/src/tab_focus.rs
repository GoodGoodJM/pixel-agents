use std::process::Command;

/// Focus a Warp terminal tab by index (0-based).
/// Uses AppleScript to activate Warp and send Cmd+N keystroke where N = tab_index + 1.
/// Only supports tab indices 0-8 (Cmd+1 through Cmd+9).
pub fn focus_warp_tab(tab_index: usize) -> Result<(), String> {
    if tab_index >= 9 {
        return Err(format!(
            "Tab index {} is out of range (max 8, Cmd+1 through Cmd+9)",
            tab_index
        ));
    }

    let tab_number = tab_index + 1;

    // Activate Warp
    let activate_script = r#"tell application "Warp" to activate"#;
    Command::new("osascript")
        .args(["-e", activate_script])
        .output()
        .map_err(|e| format!("Failed to activate Warp: {}", e))?;

    // Send Cmd+N to switch to the tab
    let keystroke_script = format!(
        r#"tell application "System Events" to keystroke "{}" using {{command down}}"#,
        tab_number
    );
    let output = Command::new("osascript")
        .args(["-e", &keystroke_script])
        .output()
        .map_err(|e| format!("Failed to send keystroke: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript error: {}", stderr));
    }

    Ok(())
}
