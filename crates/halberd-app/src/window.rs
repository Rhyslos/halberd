//! The editor window: opens it with eframe (winit + wgpu) and hands each
//! frame to the workbench from `halberd-ui`.

use crate::compile::{self, CompileSession};
use crate::content::{ContentLoader, Mounted, Mounting};
use crate::files;
use crate::gpu::GpuViewport;
use crate::logging::LogBook;
use crate::preferences::PreferenceSaver;
use crate::props::PropFeed;
use eframe::egui;
use halberd_assets::{GameContent, GmodInstall, MaterialLoader};
use halberd_config::{Settings, SettingsStore};
use halberd_render::MATERIAL_TEXTURE_SIZE;
use halberd_ui::{
    AppInfo, ContentStatus, FileIntent, LogEntry, NoRenderer, THUMBNAIL_SIZE, ViewSetup,
    ViewportOptions, ViewportPanel, ViewportRenderer, Workbench, WorkbenchAction,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Name eframe uses for the window and its own storage folder
/// (window size, position and the panel layout).
const APP_NAME: &str = "Halberd";
/// Key under which the panel layout is remembered between sessions.
const LAYOUT_KEY: &str = "halberd_panel_layout";
/// Key under which the viewport camera is remembered between sessions.
const CAMERA_KEY: &str = "halberd_viewport_camera";
/// Key under which the viewport's views (layout, what each shows, the side
/// views' cameras) are remembered between sessions.
const VIEWS_KEY: &str = "halberd_viewport_views";
/// Window size on first launch, in logical pixels.
const FIRST_SIZE: [f32; 2] = [1600.0, 900.0];
/// The smallest the window may get while staying usable.
const MIN_SIZE: [f32; 2] = [960.0, 600.0];

/// The running editor.
struct HalberdApp {
    workbench: Workbench,
    /// The GPU renderer, or `None` (then `no_renderer` explains why).
    gpu: Option<GpuViewport>,
    no_renderer: NoRenderer,
    preferences: PreferenceSaver,
    /// The window may close: the user confirmed (or had nothing unsaved).
    allow_close: bool,
    /// The title last given to the window.
    title: String,
    /// Messages from the logger, for the Console.
    log: Arc<LogBook>,
    /// The game's content, while it is being mounted.
    loading: Option<ContentLoader>,
    /// The game's content, once mounted.
    content: Option<Arc<GameContent>>,
    /// Reads material pictures for the browser, once content is mounted.
    materials: Option<MaterialLoader>,
    /// Reads models for the prop browser and the views.
    props: Option<PropFeed>,
    /// Garry's Mod, if found (for compiling and starting the game).
    install: Option<GmodInstall>,
    /// The compile running, if any.
    compile: Option<CompileSession>,
}

impl eframe::App for HalberdApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        for entry in self.log.take() {
            self.workbench.console_mut().push_logged(entry);
        }
        match self.loading.as_ref().map(ContentLoader::poll) {
            Some(Mounting::Ready(Mounted {
                content,
                materials,
                models,
            })) => {
                self.loading = None;
                self.workbench
                    .set_sky_names(halberd_ui::sky_names(&materials));
                self.workbench.materials_mut().set_names(materials);
                self.workbench.props_mut().set_names(models);
                self.workbench.push_console(content.summary());
                for problem in content.problems() {
                    self.workbench.push_warning(problem.clone());
                }
                self.workbench
                    .set_content_status(ContentStatus::Mounted(content.summary()));
                let content = Arc::new(content);
                let wake = ctx.clone();
                self.materials = Some(MaterialLoader::new(Arc::clone(&content), move || {
                    wake.request_repaint();
                }));
                let wake = ctx.clone();
                self.props = Some(PropFeed::new(Arc::clone(&content), move || {
                    wake.request_repaint();
                }));
                self.content = Some(content);
            }
            Some(Mounting::Failed) => {
                self.loading = None;
                let why = "Garry's Mod's content could not be read (the reading stopped \
                           unexpectedly; the log file may say more).";
                self.workbench.push_warning(why);
                self.workbench
                    .set_content_status(ContentStatus::Mounted(why.into()));
            }
            Some(Mounting::Busy) | None => {}
        }
        if let Some(loader) = &mut self.materials {
            for picture in loader.poll() {
                if picture.max_size == MATERIAL_TEXTURE_SIZE {
                    if let Some(gpu) = &mut self.gpu {
                        gpu.texture_arrived(&picture);
                    }
                    continue;
                }
                let result = match picture.image {
                    Some(image) => Ok((image.width, image.height, image.pixels)),
                    None => Err(picture.problem.unwrap_or_else(|| "no picture".into())),
                };
                self.workbench
                    .materials_mut()
                    .add_picture(&picture.name, result);
            }
        }
        if let Some(feed) = &mut self.props
            && feed.deliver(&mut self.workbench, self.gpu.as_mut())
        {
            ctx.request_repaint();
        }
        // The window's close button: with unsaved changes, ask first.
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && !self.allow_close && self.workbench.is_modified() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if let Some(action) = self.workbench.request(FileIntent::Quit) {
                files::handle(action, &mut self.workbench, &ctx, &mut self.allow_close);
            }
        }
        let renderer: &mut dyn ViewportRenderer = match &mut self.gpu {
            Some(gpu) => gpu,
            None => &mut self.no_renderer,
        };
        for action in self.workbench.show(ui, renderer) {
            match action {
                WorkbenchAction::Compile => {
                    if self.compile.is_none() {
                        self.compile =
                            compile::start(&mut self.workbench, self.install.as_ref(), &ctx);
                    }
                }
                WorkbenchAction::CancelCompile => {
                    if let Some(session) = &self.compile {
                        session.cancel();
                    }
                }
                other => files::handle(other, &mut self.workbench, &ctx, &mut self.allow_close),
            }
        }
        if let Some(session) = &mut self.compile
            && !session.poll(&mut self.workbench)
        {
            self.compile = None;
        }
        // Drawing the browser and the views is what says which pictures and
        // textures are wanted, so they are asked for after it.
        if let Some(loader) = &mut self.materials {
            for name in self.workbench.materials_mut().take_wanted() {
                loader.request(&name, THUMBNAIL_SIZE);
            }
            if let Some(gpu) = &mut self.gpu {
                for name in gpu.wanted_textures() {
                    loader.request(&name, MATERIAL_TEXTURE_SIZE);
                }
            }
        }
        if let Some(feed) = &mut self.props {
            feed.ask(&mut self.workbench, self.gpu.as_mut());
        }
        let title = self.workbench.window_title();
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        let (unit, player) = (self.workbench.length_unit(), self.workbench.show_player());
        if let Some(note) = self.preferences.update(unit, player) {
            self.workbench.push_warning(note);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, LAYOUT_KEY, self.workbench.layout());
        eframe::set_value(
            storage,
            CAMERA_KEY,
            &self.workbench.viewport().camera_state(),
        );
        eframe::set_value(storage, VIEWS_KEY, &self.workbench.viewport().view_setup());
    }
}

/// Opens the editor window and runs until it is closed.
///
/// `report` is the startup report (already in the log file); it becomes the
/// first lines of the Console panel. Messages the Console gets later are
/// written to the log file through `log`, which also brings in messages
/// from the logger. `settings` configure the viewport and display; display choices
/// made in the editor are saved back through `store`. Returns a plain-language reason
/// if the window could not be opened (for example, no graphics driver that
/// supports Vulkan, DirectX 12 or Metal).
pub(crate) fn run(
    info: AppInfo,
    report: Vec<LogEntry>,
    settings: Settings,
    store: Option<SettingsStore>,
    map: Option<PathBuf>,
    install: Option<GmodInstall>,
    log: Arc<LogBook>,
) -> Result<(), String> {
    let viewport_options = ViewportOptions {
        // Not asked yet (first launch) counts as on, until Settings can ask.
        wasd_enabled: settings.editor.wasd_movement.unwrap_or(true),
        grid_size: settings.editor.grid_size as f32,
    };
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
                let saved_camera = cc.storage.and_then(|s| eframe::get_value(s, CAMERA_KEY));
                let saved_views: Option<ViewSetup> =
                    cc.storage.and_then(|s| eframe::get_value(s, VIEWS_KEY));
                let viewport = ViewportPanel::new(saved_camera, viewport_options)
                    .with_view_setup(saved_views.unwrap_or_default());
                let editor = &settings.editor;
                let mut workbench = Workbench::new(info, saved_layout)
                    .with_viewport(viewport)
                    .with_display(editor.length_unit, editor.show_player_scale);
                // Messages the Console has so far are sent to the file now;
                // the report is already there (main.rs wrote it).
                let file = Arc::clone(&log);
                workbench
                    .console_mut()
                    .set_mirror(Box::new(move |entry| file.write_to_file(entry)));
                for entry in report {
                    workbench.console_mut().push_logged(entry);
                }
                let ctx = cc.egui_ctx.clone();
                log.start_waker(move || ctx.request_repaint());
                let kept_install = install.clone();
                let loading = install.map(|install| {
                    workbench.set_content_status(ContentStatus::Mounting);
                    let ctx = cc.egui_ctx.clone();
                    ContentLoader::start(install, move || ctx.request_repaint())
                });
                if let Some(path) = &map {
                    files::open_path(&mut workbench, path);
                }
                let gpu = cc.wgpu_render_state.as_ref().map(|state| {
                    let gpu = GpuViewport::new(state);
                    workbench.push_console(gpu.describe());
                    gpu
                });
                let no_renderer = NoRenderer {
                    reason: "the window has no GPU device".to_string(),
                };
                Ok(Box::new(HalberdApp {
                    workbench,
                    gpu,
                    no_renderer,
                    preferences: PreferenceSaver::new(settings, store),
                    allow_close: false,
                    title: String::new(),
                    log,
                    loading,
                    content: None,
                    materials: None,
                    props: None,
                    install: kept_install,
                    compile: None,
                }))
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
