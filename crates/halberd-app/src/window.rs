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
    .map_err(|err| err.to_string())
}
