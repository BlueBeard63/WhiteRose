//! Workspaces: the open windows, their tabs and split panes, saved under a
//! name (☰ › Workspaces, and the start page) and opened again later, in
//! $XDG_STATE_HOME/whiterose/files/workspaces.json. The views of the
//! folders in it (grid or list, sorting, grouping...) are saved with it and
//! put back when it opens.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    session::{self, Session, Tab},
    views,
};

/// Workspaces kept at most.
const KEEP: usize = 30;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub name: String,
    /// unix s
    pub saved: i64,
    pub session: Session,
    /// The folders' own views when it was saved.
    pub views: BTreeMap<String, views::Folder>,
}

/// What the menus show.
#[derive(Serialize, Debug, PartialEq)]
pub struct Summary {
    pub name: String,
    pub saved: i64,
    pub windows: usize,
    pub tabs: usize,
    /// The folders its tabs show (first ones), for a hint.
    pub folders: Vec<String>,
}

pub fn file() -> PathBuf {
    session::dir().join("workspaces.json")
}

pub fn load(file: &Path) -> Vec<Workspace> {
    fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn store(file: &Path, all: &[Workspace]) -> Result<(), String> {
    if let Some(d) = file.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string(all).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Every folder a tab shows (both sides of a split).
fn paths(s: &Session) -> Vec<String> {
    fn tab(t: &Tab, out: &mut Vec<String>) {
        out.push(t.path.clone());
        if let Some(b) = &t.second {
            tab(b, out);
        }
    }
    let mut out = vec![];
    for w in &s.windows {
        for t in &w.tabs {
            tab(t, &mut out);
        }
    }
    out
}

fn check_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Give it a name".into());
    }
    if n.chars().count() > 60 {
        return Err("That name is too long".into());
    }
    Ok(n.to_string())
}

/// Saves `s` as `name` (replacing one called that), with its folders' views.
pub fn save(
    file: &Path,
    all_views: &views::Views,
    name: &str,
    s: Session,
    now: i64,
) -> Result<(), String> {
    let name = check_name(name)?;
    if s.windows.is_empty() {
        return Err("Nothing is open".into());
    }
    let views = paths(&s)
        .into_iter()
        .filter_map(|p| all_views.folders.get(&p).map(|f| (p, f.clone())))
        .collect();
    let mut all = load(file);
    all.retain(|w| w.name != name);
    all.insert(
        0,
        Workspace {
            name,
            saved: now,
            session: s,
            views,
        },
    );
    all.truncate(KEEP);
    store(file, &all)
}

pub fn list(file: &Path) -> Vec<Summary> {
    load(file)
        .into_iter()
        .map(|w| {
            let ps = paths(&w.session);
            Summary {
                windows: w.session.windows.len(),
                tabs: w.session.windows.iter().map(|x| x.tabs.len()).sum(),
                folders: ps.into_iter().take(4).collect(),
                name: w.name,
                saved: w.saved,
            }
        })
        .collect()
}

/// `name`'s windows (cleaned up: folders that have gone open the nearest
/// one still there); its folders' views go back first.
pub fn open(
    file: &Path,
    name: &str,
    home: &Path,
    put_views: &mut dyn FnMut(&str, &views::Folder),
) -> Result<Session, String> {
    let w = load(file)
        .into_iter()
        .find(|w| w.name == name)
        .ok_or_else(|| format!("There's no workspace called “{name}”"))?;
    for (p, v) in &w.views {
        put_views(p, v);
    }
    Ok(session::clean(w.session, home))
}

pub fn delete(file: &Path, name: &str) -> Result<(), String> {
    let mut all = load(file);
    all.retain(|w| w.name != name);
    store(file, &all)
}

pub fn rename(file: &Path, from: &str, to: &str) -> Result<(), String> {
    let to = check_name(to)?;
    let mut all = load(file);
    if from != to && all.iter().any(|w| w.name == to) {
        return Err(format!("There's already a workspace called “{to}”"));
    }
    let w = all.iter_mut().find(|w| w.name == from).ok_or("It's gone")?;
    w.name = to;
    store(file, &all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use crate::session::Window;

    fn tab(p: &str) -> Tab {
        Tab {
            path: p.into(),
            ..Tab::default()
        }
    }

    #[test]
    fn save_list_open_rename_delete() {
        let d = Scratch::new("ws");
        fs::create_dir_all(d.join("code/site")).unwrap();
        fs::create_dir_all(d.join("docs")).unwrap();
        let f = d.join("workspaces.json");
        let site = d.join("code/site").to_string_lossy().into_owned();
        let docs = d.join("docs").to_string_lossy().into_owned();

        let mut all_views = views::Views::default();
        let grid = views::View {
            mode: "list".into(),
            ..views::View::default()
        };
        all_views.set(&site, grid.clone());
        let s = Session {
            windows: vec![
                Window {
                    tabs: vec![
                        Tab {
                            second: Some(Box::new(tab(&docs))),
                            ..tab(&site)
                        },
                        tab("recent:"),
                    ],
                    current: 1,
                },
                Window {
                    tabs: vec![tab(&format!("{docs}/gone"))],
                    current: 0,
                },
            ],
        };
        assert!(save(&f, &all_views, "  ", s.clone(), 1).is_err());
        save(&f, &all_views, " Website ", s.clone(), 100).unwrap();
        save(
            &f,
            &all_views,
            "Writing",
            Session {
                windows: vec![Window {
                    tabs: vec![tab(&docs)],
                    current: 0,
                }],
            },
            200,
        )
        .unwrap();

        let l = list(&f);
        assert_eq!(
            l.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            vec!["Writing", "Website"]
        );
        assert_eq!((l[1].windows, l[1].tabs), (2, 3));
        assert_eq!(l[1].folders[..2], [site.clone(), docs.clone()]);

        // Opening: views go back, gone folders open the nearest one, Recent stays.
        let mut put = vec![];
        let back = open(&f, "Website", &d, &mut |p, v| {
            put.push((p.to_string(), v.view.mode.clone()))
        })
        .unwrap();
        assert_eq!(put, vec![(site.clone(), "list".to_string())]);
        assert_eq!(back.windows[0].tabs[1].path, "recent:");
        assert_eq!(back.windows[0].tabs[0].second.as_ref().unwrap().path, docs);
        assert_eq!(back.windows[1].tabs[0].path, docs);
        assert!(open(&f, "Nope", &d, &mut |_, _| {}).is_err());

        // Saving again under a name replaces it.
        save(
            &f,
            &all_views,
            "Website",
            Session {
                windows: vec![Window {
                    tabs: vec![tab(&docs)],
                    current: 0,
                }],
            },
            300,
        )
        .unwrap();
        assert_eq!(list(&f).len(), 2);

        assert!(rename(&f, "Website", "Writing").is_err());
        rename(&f, "Website", "Site").unwrap();
        delete(&f, "Writing").unwrap();
        assert_eq!(
            list(&f).iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
            vec!["Site"]
        );
    }
}
