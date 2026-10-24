//! Feeding models to the prop browser, the Place tool and the renderer:
//! models are read in the background (`halberd_assets::ModelLoader`), and
//! each one that arrives becomes a browser picture, what the Place tool
//! needs to know, a box for picking props, and (if the map shows it) a
//! model the renderer draws.

use crate::gpu::GpuViewport;
use halberd_assets::{GameContent, LoadedModel, ModelLoader};
use halberd_geom::Aabb;
use halberd_render::PropModel;
use halberd_tools::PropChoice;
use halberd_ui::Workbench;
use std::sync::Arc;

/// Reads models for the prop browser and the views.
pub(crate) struct PropFeed {
    loader: ModelLoader,
}

impl PropFeed {
    /// Starts reading from `content`; `wake` redraws the window when a
    /// model is ready.
    pub(crate) fn new(content: Arc<GameContent>, wake: impl Fn() + Send + 'static) -> Self {
        Self {
            loader: ModelLoader::new(content, wake),
        }
    }

    /// Hands over the models read since the last frame.
    pub(crate) fn deliver(&mut self, workbench: &mut Workbench, gpu: Option<&mut GpuViewport>) {
        let mut gpu = gpu;
        for loaded in self.loader.poll() {
            deliver_one(loaded, workbench, gpu.as_deref_mut());
        }
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
            workbench.props_mut().add_picture(&name, Err(why));
            if let Some(gpu) = gpu {
                gpu.model_arrived(&name, None);
            }
            return;
        }
    };
    let (min, max) = model.bounds;
    let local = Aabb::from_corners(min.into(), max.into());
    workbench.document_mut().set_model_box(&name, local);
    workbench.add_prop_info(PropChoice {
        model: name.clone(),
        local,
        static_prop: model.static_prop,
    });
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
