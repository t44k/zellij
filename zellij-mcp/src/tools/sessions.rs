use crate::types::text_content;
use anyhow::{Context, Result};
use serde_json::Value;

pub fn attach_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    Ok(text_content(format!("Run: zellij attach {}", session_name)))
}

pub fn new_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    let layout = args.get("layout").and_then(|l| l.as_str());

    let cmd = if let Some(layout) = layout {
        format!("zellij --session {} --layout {}", session_name, layout)
    } else {
        format!("zellij --session {}", session_name)
    };

    Ok(text_content(format!("Run: {}", cmd)))
}

pub fn kill_session(args: Value) -> Result<Value> {
    let session_name = args
        .get("session_name")
        .and_then(|s| s.as_str())
        .context("Missing session_name")?;

    Ok(text_content(format!(
        "Would kill session: {}",
        session_name
    )))
}

pub fn get_session_info(session: &str) -> Result<Value> {
    use std::process::Command;

    // Get session info via list-sessions
    let output = Command::new("zellij")
        .args(&["list-sessions"])
        .output()
        .context("Failed to execute zellij list-sessions")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Zellij list-sessions failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the output to find our session
    for line in stdout.lines() {
        // Remove ANSI color codes
        let clean_line = strip_ansi_codes(&line);

        if clean_line.starts_with(session) {
            // Extract information from the line
            let info = parse_session_line(&clean_line);
            return Ok(text_content(format!("Session: {}\n{}", session, info)));
        }
    }

    Ok(text_content(format!("Session '{}' not found", session)))
}

fn strip_ansi_codes(s: &str) -> String {
    // Simple ANSI code stripper
    let mut result = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }

    result
}

fn parse_session_line(line: &str) -> String {
    // Example line: "session-name [Created 1h 2m 3s ago] (RUNNING)"
    let parts: Vec<&str> = line.split('[').collect();
    if parts.len() > 1 {
        let info_parts: Vec<&str> = parts[1].split(']').collect();
        if info_parts.len() > 1 {
            let created = info_parts[0].trim();
            let status = info_parts[1].trim().trim_matches(|c| c == '(' || c == ')');
            return format!("Created: {}\nStatus: {}", created, status);
        }
    }
    "Status: UNKNOWN".to_string()
}
