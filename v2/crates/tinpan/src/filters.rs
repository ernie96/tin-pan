//! The Library filters (Alt+F), as Tin Pan 1's: workspace, disliked songs, every filter Suno's own
//! library has, and the order. Suno applies them itself, so paging and search follow them. Kept in the
//! settings, which Tin Pan 1 shares.

use tinpan_core::api::{ANY, FEED_FILTERS, HIDE, ONLY, Workspace};
use tinpan_core::config::LibraryConfig;
use tinpan_core::i18n::{t, tf};
use wxdragon::prelude::*;

use crate::form::{self, Field};

const DISLIKED: [(&str, &str); 3] = [("False", "Hide songs I disliked"), ("Any", "Show songs I disliked too"), ("True", "Only songs I disliked")];
const SORTS: [(&str, &str); 4] = [("newest", "Newest first"), ("oldest", "Oldest first"), ("most-liked", "Most liked first"), ("least-liked", "Least liked first")];

/// The form label for each of Suno's filters, with its access key.
fn label(field: &str) -> &'static str {
    match field {
        "trashed" => "Songs in the _trash",
        "stem" => "_Stems",
        "fromStudioProject" => "Suno Stud_io songs",
        "liked" => "_Liked songs",
        "public" => "Pu_blic or private",
        "upload" => "_Uploaded songs",
        "cover" => "_Covers",
        "extend" => "_Extended songs",
        "remaster" => "_Remasters",
        "concat" => "W_hole songs (joined parts)",
        "persona" => "Songs with a _persona",
        "playlist" => "Songs in a pla_ylist",
        "unlocked" => "Downloaded be_fore",
        _ => "",
    }
}

/// A filter's three choices, in Suno's values: show both, only these, hide these.
fn choices(field: &str) -> [(&'static str, &'static str); 3] {
    if field == "public" {
        [(ANY, "Both"), (ONLY, "Only public"), (HIDE, "Only private")]
    } else {
        [(ANY, "Show"), (ONLY, "Only these"), (HIDE, "Hide")]
    }
}

fn value_of(library: &LibraryConfig, field: &str, default: &'static str) -> &'static str {
    let saved = library.show.get(field).map(String::as_str);
    [ANY, ONLY, HIDE].into_iter().find(|v| Some(*v) == saved).unwrap_or(default)
}

/// Shows the form; true when the filters changed and the library should load again.
pub fn edit(parent: &impl WxWidget, library: &mut LibraryConfig, workspaces: &[Workspace]) -> bool {
    let all = Workspace { id: String::new(), name: t("All workspaces"), songs: None };
    let mut spaces = vec![all];
    spaces.extend(workspaces.iter().cloned());
    let workspace_now = spaces.iter().position(|w| w.id == library.workspace_id).unwrap_or(0);

    let mut fields = vec![
        Field::choice("_Workspace", spaces.iter().map(ToString::to_string).collect(), workspace_now),
        Field::choice(
            "_Disliked songs",
            DISLIKED.iter().map(|(_, l)| t(l)).collect(),
            DISLIKED.iter().position(|(v, _)| *v == library.disliked).unwrap_or(0),
        ),
    ];
    for (field, _, default) in FEED_FILTERS {
        let now = value_of(library, field, default);
        let options = choices(field);
        fields.push(Field::choice(label(field), options.iter().map(|(_, l)| t(l)).collect(), options.iter().position(|(v, _)| *v == now).unwrap_or(0)));
    }
    fields.push(Field::choice("_Order", SORTS.iter().map(|(_, l)| t(l)).collect(), SORTS.iter().position(|(id, _)| Some(*id) == library.sort.as_deref()).unwrap_or(0)));

    let intro = t("Suno applies these filters itself, so paging and search follow them. Each one shows those songs, shows only them, or hides them.");
    if !form::show(parent, &t("Library filters"), &intro, &t("Apply"), &mut fields) {
        return false;
    }

    let chosen = &spaces[fields[0].selected()];
    library.workspace_id = chosen.id.clone();
    library.workspace_name = if chosen.id.is_empty() { String::new() } else { chosen.name.clone() };
    library.disliked = DISLIKED[fields[1].selected()].0.to_string();
    for (i, (field, _, _)) in FEED_FILTERS.iter().enumerate() {
        let value = choices(field)[fields[2 + i].selected()].0;
        library.show.insert((*field).to_string(), value.to_string());
    }
    library.sort = Some(SORTS[fields[2 + FEED_FILTERS.len()].selected()].0.to_string());
    true
}

/// A short spoken description of the filters, for after they change: only what differs from Suno's defaults.
pub fn summary(library: &LibraryConfig) -> String {
    let mut parts = vec![if library.workspace_id.is_empty() { t("All workspaces") } else { library.workspace_name.clone() }];
    parts.push(match library.disliked.as_str() {
        "Any" => t("disliked shown"),
        "True" => t("only disliked"),
        _ => t("disliked hidden"),
    });
    for (field, _, default) in FEED_FILTERS {
        let value = value_of(library, field, default);
        if value == default {
            continue;
        }
        let shown = choices(field).iter().find(|(v, _)| *v == value).map(|(_, l)| t(l)).unwrap_or_default();
        parts.push(tf("{0}: {1}", &[&t(label(field)).replace('_', ""), &shown]));
    }
    parts.push(t(SORTS.iter().find(|(id, _)| Some(*id) == library.sort.as_deref()).map_or(SORTS[0].1, |(_, l)| l)));
    parts.join(", ") + "."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_names_only_what_differs_from_sunos_defaults() {
        let mut library = LibraryConfig::default();
        assert_eq!(summary(&library), "All workspaces, disliked hidden, Newest first.");
        library.show.insert("stem".into(), "Any".into());
        library.show.insert("public".into(), "False".into());
        library.disliked = "True".into();
        library.sort = Some("oldest".into());
        assert_eq!(summary(&library), "All workspaces, only disliked, Stems: Show, Public or private: Only private, Oldest first.");
    }
}
