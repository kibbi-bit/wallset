use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub current_wallpaper: Option<PathBuf>,
    pub available: bool,
}

#[derive(Default)]
pub struct MonitorRegistry {
    connected: Vec<Monitor>,
    connected_ids: HashSet<String>,
}

pub struct Reconciliation {
    pub topology_changed: bool,
    pub newly_connected: Vec<String>,
}

impl MonitorRegistry {
    pub fn reconcile(&mut self, monitors: Vec<Monitor>) -> Reconciliation {
        let next_ids: HashSet<_> = monitors.iter().map(|monitor| monitor.id.clone()).collect();
        let newly_connected = next_ids.difference(&self.connected_ids).cloned().collect();
        let topology_changed = next_ids != self.connected_ids;
        self.connected = monitors;
        self.connected_ids = next_ids;
        Reconciliation {
            topology_changed,
            newly_connected,
        }
    }

    pub fn is_connected(&self, monitor_id: &str) -> bool {
        self.connected_ids.contains(monitor_id)
    }

    pub fn current_wallpaper(&self, monitor_id: &str) -> Option<&PathBuf> {
        self.connected
            .iter()
            .find(|monitor| monitor.id == monitor_id)
            .and_then(|monitor| monitor.current_wallpaper.as_ref())
    }

    pub fn view<'a>(&self, saved_ids: impl IntoIterator<Item = &'a String>) -> Vec<Monitor> {
        let mut monitors = self.connected.clone();
        for id in saved_ids {
            if !self.connected_ids.contains(id) {
                monitors.push(Monitor {
                    id: id.clone(),
                    name: "Saved display".into(),
                    detail: "Configuration retained".into(),
                    current_wallpaper: None,
                    available: false,
                });
            }
        }
        monitors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(id: &str) -> Monitor {
        Monitor {
            id: id.into(),
            name: id.into(),
            detail: String::new(),
            current_wallpaper: None,
            available: true,
        }
    }

    #[test]
    fn registry_reports_topology_and_new_connections() {
        let mut registry = MonitorRegistry::default();
        let first = registry.reconcile(vec![monitor("a")]);
        assert!(first.topology_changed);
        assert_eq!(first.newly_connected, ["a"]);
        let unchanged = registry.reconcile(vec![monitor("a")]);
        assert!(!unchanged.topology_changed);
        assert!(unchanged.newly_connected.is_empty());
    }

    #[test]
    fn registry_keeps_saved_disconnected_monitors_in_its_view() {
        let mut registry = MonitorRegistry::default();
        registry.reconcile(vec![monitor("connected")]);
        let saved = ["connected".to_string(), "disconnected".to_string()];
        let view = registry.view(saved.iter());
        assert_eq!(view.len(), 2);
        assert!(
            !view
                .iter()
                .find(|monitor| monitor.id == "disconnected")
                .unwrap()
                .available
        );
    }
}
