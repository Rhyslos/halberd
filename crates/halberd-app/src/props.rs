//! Feeding models to the prop browser, the Place tool and the renderer:
//! models are read in the background (`halberd_assets::ModelLoader`), and
//! each one that arrives becomes a browser picture, what the Place tool
//! needs to know, a box for picking props, and (if the map shows it) a
//! model the renderer draws.

use crate::gpu::GpuViewport;
use halberd_assets::{GameContent, LoadedModel, ModelLoader};
use halberd_geom::Aabb;
use halberd_render::PropModel;
use halberd_ui::Workbench;
use std::collections::VecDeque;
use std::sync::Arc;

/// Most models handed over in one frame: each one's picture is drawn and
/// read back while the frame waits, so a burst is spread over frames.
const MODELS_PER_FRAME: usize = 6;

/// Reads models for the prop browser and the views.
pub(crate) struct PropFeed {
    loader: ModelLoader,
    /// Models read but not handed over yet.
    waiting: VecDeque<LoadedModel>,
}

impl PropFeed {
    /// Starts reading from `content`; `wake` redraws the window when a
    /// model is ready.
    pub(crate) fn new(content: Arc<GameContent>, wake: impl Fn() + Send + 'static) -> Self {
        Self {
            loader: ModelLoader::new(content, wake),
            waiting: VecDeque::new(),
        }
    }

    /// Hands over models read since the last frame, a few at a time.
    /// Returns true if more are waiting (the window should draw another
    /// frame soon).
    pub(crate) fn deliver(
        &mut self,
        workbench: &mut Workbench,
        gpu: Option<&mut GpuViewport>,
    ) -> bool {
        let mut gpu = gpu;
        self.waiting.extend(self.loader.poll());
        for _ in 0..MODELS_PER_FRAME {
            let Some(loaded) = self.waiting.pop_front() else {
                break;
            };
            deliver_one(loaded, workbench, gpu.as_deref_mut());
        }
        !self.waiting.is_empty()
    }

    /// Asks for the models the browser wants pictures of and the models
    /// the map's props show (after the frame is drawn, which is when both
    /// are known).
    pub(crate) fn ask(&mut self, workbench: &mut Workbench, gpu: Option<&mut GpuViewport>) {
        for name in workbench.props_mut().take_wanted() {
            self.loader.request(&name);
        }
        if let Some(gpu) = gpu {
            for name in gpu.wanted_models() {
                self.loader.request(&name);
            }
        }
    }
}

fn deliver_one(loaded: LoadedModel, workbench: &mut Workbench, gpu: Option<&mut GpuViewport>) {
    let LoadedModel {
        name,
        model,
        materials,
    } = loaded;
    let model = match model {
        Ok(model) => model,
        Err(why) => {
            workbench.prop_failed(&name, &why);
            workbench.props_mut().add_picture(&name, Err(why));
            if let Some(gpu) = gpu {
                gpu.model_arrived(&name, None);
            }
            return;
        }
    };
    let (min, max) = model.bounds;
    let local = Aabb::from_corners(min.into(), max.into());
    workbench.add_model_info(&name, local, model.static_prop);
    let prop = PropModel::new(&model, &materials);
    let picture = match gpu {
        Some(gpu) => {
            let picture = gpu
                .thumbnail(&prop)
                .ok_or_else(|| "it has nothing to draw".to_string());
            gpu.model_arrived(&name, Some(prop));
            picture
        }
        None => Err("pictures need a graphics device".to_string()),
    };
    workbench.props_mut().add_picture(&name, picture);
}
