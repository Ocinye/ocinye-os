//! Displays, from the kernel's DRM connectors (`/sys/class/drm`).
//!
//! Each connector is a directory `cardN-NAME` with `status` (`connected`),
//! `enabled` (`enabled`/`disabled`) and `modes`, one mode per line, the
//! preferred one first. That is everything this service reports: refresh rate
//! and scale belong to whoever sets modes — the compositor — and
//! `display.configure` is not implemented here for the same reason.

use std::path::Path;

use ocinye_system_contracts::state::DisplayOutput;

/// Connector types that are the machine's own panel.
const INTERNAL: &[&str] = &["eDP", "LVDS", "DSI"];

/// `1920x1080` (sometimes followed by `i` for interlaced) as width and
/// height.
#[must_use]
pub fn mode(line: &str) -> Option<(u32, u32)> {
    let (w, h) = line.trim().split_once('x')?;
    let h: String = h.chars().take_while(char::is_ascii_digit).collect();
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    (w > 0 && h > 0 && w <= 16384 && h <= 16384).then_some((w, h))
}

/// The connector's name without its card (`card0-eDP-1` → `eDP-1`).
fn connector(dir: &str) -> Option<&str> {
    let rest = dir.strip_prefix("card")?;
    let (card, name) = rest.split_once('-')?;
    (!card.is_empty() && card.bytes().all(|b| b.is_ascii_digit()) && !name.is_empty())
        .then_some(name)
}

/// The connected displays under `drm` (`/sys/class/drm`), by connector name.
#[must_use]
pub fn outputs(drm: &Path) -> Vec<DisplayOutput> {
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let mut names: Vec<String> = std::fs::read_dir(drm)
        .map(|d| {
            d.filter_map(Result::ok)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
        .iter()
        .filter_map(|dir| {
            let name = connector(dir)?;
            let d = drm.join(dir);
            if read(&d.join("status"))?.trim() != "connected" {
                return None;
            }
            let (width, height) = read(&d.join("modes"))
                .and_then(|m| m.lines().next().and_then(mode))
                .unwrap_or((0, 0));
            Some(DisplayOutput {
                name: name.to_owned(),
                internal: INTERNAL.iter().any(|k| {
                    name.strip_prefix(k)
                        .is_some_and(|r| r.is_empty() || r.starts_with('-'))
                }),
                enabled: read(&d.join("enabled")).is_some_and(|e| e.trim() == "enabled"),
                width,
                height,
                refresh_mhz: None,
                scale_percent: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_modos_e_os_conectores() {
        assert_eq!(mode("1920x1080"), Some((1920, 1080)));
        assert_eq!(mode("1920x1080i\n"), Some((1920, 1080)));
        assert_eq!(mode("x1080"), None);
        assert_eq!(mode("0x0"), None);
        assert_eq!(connector("card0-eDP-1"), Some("eDP-1"));
        assert_eq!(connector("card12-HDMI-A-2"), Some("HDMI-A-2"));
        assert_eq!(connector("card0"), None);
        assert_eq!(connector("renderD128"), None);
        assert_eq!(connector("cardX-DP-1"), None);
    }

    #[test]
    fn so_os_ligados_e_o_painel_interno_e_o_interno() {
        let d = std::env::temp_dir().join(format!("ocinye-drm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let put = |rel: &str, body: &str| {
            let p = d.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        put("card0-eDP-1/status", "connected\n");
        put("card0-eDP-1/enabled", "enabled\n");
        put("card0-eDP-1/modes", "2560x1600\n1920x1200\n");
        put("card0-HDMI-A-1/status", "disconnected\n");
        put("card0-DP-2/status", "connected\n");
        put("card0-DP-2/enabled", "disabled\n");
        put("card0-DP-2/modes", "");
        put("card0/dev", "226:0\n");
        put("version", "drm 1.1.0\n");
        let o = outputs(&d);
        assert_eq!(
            o.iter()
                .map(|x| (x.name.as_str(), x.internal, x.enabled, x.width, x.height))
                .collect::<Vec<_>>(),
            [
                ("DP-2", false, false, 0, 0),
                ("eDP-1", true, true, 2560, 1600)
            ]
        );
        assert!(o
            .iter()
            .all(|x| x.refresh_mhz.is_none() && x.scale_percent.is_none()));
        assert!(outputs(&d.join("nothing")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }
}
