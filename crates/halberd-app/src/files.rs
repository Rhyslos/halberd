//! Carries out the workbench's file requests: the Open and Save windows,
//! and reading and writing maps with `halberd-io`.

use eframe::egui;
use halberd_doc::Object;
use halberd_ui::{FileIntent, Workbench, WorkbenchAction};
use std::path::{Path, PathBuf};

/// Filter shown in the Open and Save windows.
const VMF_FILTER: (&str, &[&str]) = ("Hammer map (*.vmf)", &["vmf"]);

/// Carries out one action from the workbench.
pub(crate) fn handle(
    action: WorkbenchAction,
    workbench: &mut Workbench,
    ctx: &egui::Context,
    allow_close: &mut bool,
) {
    match action {
        WorkbenchAction::Quit => {
            *allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        WorkbenchAction::Open => {
            let picked = rfd::FileDialog::new()
                .set_title("Open map")
                .add_filter(VMF_FILTER.0, VMF_FILTER.1)
                .pick_file();
            if let Some(path) = picked {
                open_path(workbench, &path);
            }
        }
        WorkbenchAction::Save { then } => match workbench.file_path().map(Path::to_path_buf) {
            Some(path) => {
                if save_to(workbench, &path) {
                    carry_on(then, workbench, ctx, allow_close);
                }
            }
            None => save_as(then, workbench, ctx, allow_close),
        },
        WorkbenchAction::SaveAs { then } => save_as(then, workbench, ctx, allow_close),
    }
}

/// After a successful save, does what the user was doing when asked to
/// save first.
fn carry_on(
    then: Option<FileIntent>,
    workbench: &mut Workbench,
    ctx: &egui::Context,
    allow_close: &mut bool,
) {
    if let Some(action) = then.and_then(|intent| workbench.carry_out(intent)) {
        handle(action, workbench, ctx, allow_close);
    }
}

fn save_as(
    then: Option<FileIntent>,
    workbench: &mut Workbench,
    ctx: &egui::Context,
    allow_close: &mut bool,
) {
    let name = workbench.file_path().and_then(Path::file_name).map_or_else(
        || "untitled.vmf".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let picked = rfd::FileDialog::new()
        .set_title("Save map as")
        .add_filter(VMF_FILTER.0, VMF_FILTER.1)
        .set_file_name(name)
        .save_file();
    // Cancelling the Save window cancels what was waiting on it too.
    if let Some(path) = picked
        && save_to(workbench, &with_vmf_extension(path))
    {
        carry_on(then, workbench, ctx, allow_close);
    }
}

/// Adds `.vmf` to a file name chosen without it.
pub(crate) fn with_vmf_extension(path: PathBuf) -> PathBuf {
    let has_vmf = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("vmf"));
    if has_vmf {
        path
    } else {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".vmf");
        path.with_file_name(name)
    }
}

/// Opens the map at `path` into the workbench, or explains why not.
pub(crate) fn open_path(workbench: &mut Workbench, path: &Path) {
    match halberd_io::open_map(path) {
        Ok(opened) => {
            let doc = &opened.document;
            let brushes = doc
                .objects()
                .filter(|(_, o)| o.as_brush().is_some())
                .count();
            let entities = doc
                .objects()
                .filter(|(_, o)| matches!(o, Object::Entity(_)))
                .count();
            workbench.set_document(opened.document, Some(path.to_path_buf()));
            workbench.push_console(format!(
                "Opened {}: {brushes} brush{}, {entities} entit{}.",
                path.display(),
                if brushes == 1 { "" } else { "es" },
                if entities == 1 { "y" } else { "ies" },
            ));
            for note in opened.notes {
                workbench.push_console(format!("  Note: {note}"));
            }
        }
        Err(e) => workbench.show_error(e.to_string()),
    }
}

/// Saves the map to `path`. Returns false (and explains) if it failed.
pub(crate) fn save_to(workbench: &mut Workbench, path: &Path) -> bool {
    match halberd_io::save_map(workbench.document_mut(), path) {
        Ok(notes) => {
            workbench.set_file_path(path.to_path_buf());
            workbench.push_console(format!("Saved {}.", path.display()));
            for note in notes {
                workbench.push_console(format!("  Note: {note}"));
            }
            true
        }
        Err(e) => {
            workbench.show_error(e.to_string());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_ui::AppInfo;

    fn workbench() -> Workbench {
        Workbench::new(
            AppInfo {
                name: "Halberd Map Editor".into(),
                version: "0.0.0".into(),
            },
            None,
        )
    }

    const SAMPLE: &str = include_str!("../../halberd-vmf/tests/data/sample.vmf");

    #[test]
    fn opening_and_saving_a_map_keeps_it_exactly() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("sample.vmf");
        std::fs::write(&path, SAMPLE).unwrap();
        let mut wb = workbench();
        open_path(&mut wb, &path);
        assert_eq!(wb.file_path(), Some(path.as_path()));
        assert_eq!(wb.document().len(), 6);
        assert!(
            wb.console_lines()
                .iter()
                .any(|l| l.contains("2 brushes, 4 entities"))
        );
        assert!(wb.window_title().starts_with("sample.vmf — "));
        assert!(save_to(&mut wb, &path));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SAMPLE);
    }

    #[test]
    fn a_file_that_cannot_be_opened_is_explained_in_a_message_box() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("broken.vmf");
        std::fs::write(&path, "versioninfo { }").unwrap();
        let mut wb = workbench();
        open_path(&mut wb, &path);
        assert!(wb.error().unwrap().contains("not a Hammer map"));
        assert_eq!(wb.file_path(), None, "the current map stays");
    }

    #[test]
    fn a_failed_save_is_explained_and_keeps_the_map_unsaved() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("sample.vmf");
        std::fs::write(&path, SAMPLE).unwrap();
        let mut wb = workbench();
        open_path(&mut wb, &path);
        let first = wb.document().objects().next().unwrap().0;
        wb.document_mut()
            .execute(halberd_doc::Command::Remove(vec![first]))
            .unwrap();
        assert!(!save_to(&mut wb, &folder.path().join("missing/x.vmf")));
        assert!(wb.error().unwrap().contains("could not be saved"));
        assert!(wb.is_modified());
        assert!(wb.window_title().starts_with("sample.vmf •"));
    }

    #[test]
    fn saved_names_get_the_vmf_extension() {
        assert_eq!(
            with_vmf_extension("a/map".into()),
            PathBuf::from("a/map.vmf")
        );
        assert_eq!(
            with_vmf_extension("a/map.VMF".into()),
            PathBuf::from("a/map.VMF")
        );
        assert_eq!(
            with_vmf_extension("a/map.v2".into()),
            PathBuf::from("a/map.v2.vmf")
        );
    }
}
