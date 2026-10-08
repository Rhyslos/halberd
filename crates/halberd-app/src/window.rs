//! The editor window: opens it with eframe (winit + wgpu) and hands each
//! frame to the workbench from `halberd-ui`.

use eframe::egui;
use halberd_ui::{AppInfo, Workbench, WorkbenchAction};

/// Name eframe uses for the window and its own storage folder
/// (window size, position and the panel layout).
const APP_NAME: &str = "Halberd";
/// Key under which the panel layout is remembered between sessions.
const LAYOUT_KEY: &str = "halberd_panel_layout";
/// Window size on first launch, in logical pixels.
const FIRST_SIZE: [f32; 2] = [1600.0, 900.0];
/// The smallest the window may get while staying usable.
const MIN_SIZE: [f32; 2] = [960.0, 600.0];

/// The running editor.
struct HalberdApp {
    workbench: Workbench,
}

impl eframe::App for HalberdApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for action in self.workbench.show(ui) {
            match action {
                WorkbenchAction::Quit => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, LAYOUT_KEY, self.workbench.layout());
    }
}

/// Opens the editor window and runs until it is closed.
///
/// `report` is the startup report; it becomes the first lines of the Console
/// panel. Returns a plain-language reason if the window could not be opened
/// (for example, no graphics driver that supports Vulkan, DirectX 12 or Metal).
pub(crate) fn run(info: AppInfo, report: Vec<String>) -> Result<(), String> {
    let title = format!("{} {}", info.name, info.version);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(FIRST_SIZE)
            .with_min_inner_size(MIN_SIZE),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    // Some windowing libraries stop with a panic instead of an error when a
    // system library is missing (for example libxkbcommon-x11 on Linux).
    // Catch that and turn it into an explanation, so Halberd never just
    // vanishes.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            APP_NAME,
            options,
            Box::new(move |cc| {
                let saved_layout = cc.storage.and_then(|s| eframe::get_value(s, LAYOUT_KEY));
                let mut workbench = Workbench::new(info, saved_layout);
                for line in report {
                    workbench.push_console(line);
                }
                Ok(Box::new(HalberdApp { workbench }))
            }),
        )
    }));
    match outcome {
        Ok(result) => result.map_err(|err| err.to_string()),
        Err(payload) => Err(explain_panic(payload.as_ref())),
    }
}

/// Turns a caught panic into a plain-language reason, adding an install
/// hint when the cause is a missing Linux system library.
fn explain_panic(payload: &(dyn std::any::Any + Send)) -> String {
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("the windowing system stopped unexpectedly");
    if message.contains(".so") && message.contains("could not be loaded") {
        format!(
            "{message} On Linux, install the missing library with your package manager \
             (for example libxkbcommon-x11-0 and mesa-vulkan-drivers on Ubuntu)."
        )
    } else {
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_linux_library_gets_an_install_hint() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from(
            "Library libxkbcommon-x11.so could not be loaded.",
        ));
        let text = explain_panic(payload.as_ref());
        assert!(text.starts_with("Library libxkbcommon-x11.so could not be loaded."));
        assert!(text.contains("package manager"));
    }

    #[test]
    fn other_panics_are_passed_through() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("surface lost");
        assert_eq!(explain_panic(payload.as_ref()), "surface lost");
    }

    #[test]
    fn unknown_panic_payloads_get_a_generic_reason() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(42_u32);
        assert!(explain_panic(payload.as_ref()).contains("stopped unexpectedly"));
    }

    #[test]
    fn a_real_panic_is_caught_and_explained() {
        let caught = std::panic::catch_unwind(|| {
            std::panic::panic_any(String::from("Library libfoo.so could not be loaded."))
        });
        let text = explain_panic(caught.unwrap_err().as_ref());
        assert!(text.contains("libfoo.so") && text.contains("package manager"));
    }
}
