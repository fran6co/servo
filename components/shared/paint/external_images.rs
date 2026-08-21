/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Letting the embedder put textures it owns into the page.
//!
//! An embedder that renders with the same GPU context as Servo can register a texture here and
//! serve it as the content of a resource, so that a document can lay it out like any other image
//! while WebRender samples the texture directly, with no copy of the pixels.

use std::collections::HashMap;
use std::sync::Arc;

use euclid::default::Size2D as UntypedSize2D;
use parking_lot::Mutex;
use webrender_api::{
    ExternalImageData, ExternalImageId, ExternalImageSource, ExternalImageType, ImageBufferKind,
    ImageDescriptor, ImageDescriptorFlags, ImageFormat, ImageKey,
};

use crate::{CrossProcessPaintApi, SerializableImageData};

use crate::{
    WebRenderExternalImageApi, WebRenderExternalImageIdManager, WebRenderImageHandlerType,
};

/// A texture the embedder owns, and the size WebRender should sample it at.
#[derive(Clone, Copy, Debug)]
pub struct EmbedderTexture {
    /// The name of the texture in the GPU context Servo renders with.
    pub texture_id: u32,
    pub width: i32,
    pub height: i32,
}

/// The embedder's registry of the textures it wants the pages to be able to display. Register a
/// texture to get an [`ExternalImageId`], hand that id out as the content of a resource, and the
/// document can then display it.
pub struct ExternalImageChannel {
    id_manager: Mutex<Option<WebRenderExternalImageIdManager>>,
    textures: Mutex<HashMap<ExternalImageId, EmbedderTexture>>,
    /// The images a document has resolved to one of these textures. WebRender caches what it has
    /// composited, so it has to be told when a texture holds something new.
    bindings: Mutex<HashMap<ExternalImageId, Vec<(ImageKey, CrossProcessPaintApi)>>>,
}

impl Default for ExternalImageChannel {
    fn default() -> Self {
        Self {
            id_manager: Mutex::new(None),
            textures: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
        }
    }
}

impl ExternalImageChannel {
    pub fn new() -> Arc<ExternalImageChannel> {
        Arc::new(ExternalImageChannel::default())
    }

    /// Called by Servo once it knows how to allocate external image identifiers.
    pub fn attach(&self, id_manager: WebRenderExternalImageIdManager) {
        *self.id_manager.lock() = Some(id_manager);
    }

    /// Registers `texture`, returning the identifier to serve as the content of a resource, or
    /// `None` if Servo has not started yet.
    pub fn register(&self, texture: EmbedderTexture) -> Option<u64> {
        let id = self
            .id_manager
            .lock()
            .as_mut()?
            .next_id(WebRenderImageHandlerType::Embedder);
        self.textures.lock().insert(id, texture);
        Some(id.0)
    }

    /// Records that a document resolved a resource to the texture registered as `id`, so that
    /// updating the texture can invalidate what WebRender composited from it.
    pub fn bind(&self, id: u64, image_key: ImageKey, paint_api: CrossProcessPaintApi) {
        self.bindings
            .lock()
            .entry(ExternalImageId(id))
            .or_default()
            .push((image_key, paint_api));
    }

    /// Points `id` at a different texture, or at the same texture at a different size, and tells
    /// WebRender that anything composited from it is out of date.
    pub fn update(&self, id: u64, texture: EmbedderTexture) {
        let id = ExternalImageId(id);
        self.textures.lock().insert(id, texture);

        let bindings = self.bindings.lock();
        let Some(bindings) = bindings.get(&id) else {
            return;
        };

        let descriptor = ImageDescriptor {
            format: ImageFormat::BGRA8,
            size: euclid::Size2D::new(texture.width, texture.height),
            stride: None,
            offset: 0,
            flags: ImageDescriptorFlags::empty(),
        };
        let external_image_data = ExternalImageData {
            id,
            channel_index: 0,
            image_type: ExternalImageType::TextureHandle(ImageBufferKind::Texture2D),
            normalized_uvs: false,
        };

        for (image_key, paint_api) in bindings {
            paint_api.update_image(
                *image_key,
                descriptor,
                SerializableImageData::External(external_image_data),
                None,
            );
            paint_api.generate_frame(vec![(*image_key).into()]);
        }
    }

    /// Forgets that `image_key` displays the texture registered as `id`, once the document that
    /// resolved it is gone.
    pub fn unbind(&self, id: u64, image_key: ImageKey) {
        let mut bindings = self.bindings.lock();
        let Some(keys) = bindings.get_mut(&ExternalImageId(id)) else {
            return;
        };

        keys.retain(|(key, _)| *key != image_key);
        if keys.is_empty() {
            bindings.remove(&ExternalImageId(id));
        }
    }

    pub fn unregister(&self, id: u64) {
        let id = ExternalImageId(id);
        self.textures.lock().remove(&id);
        self.bindings.lock().remove(&id);
        if let Some(id_manager) = self.id_manager.lock().as_mut() {
            id_manager.remove(&id);
        }
    }

    pub fn get(&self, id: u64) -> Option<EmbedderTexture> {
        self.textures.lock().get(&ExternalImageId(id)).copied()
    }
}

/// Answers WebRender's requests for the textures in an [`ExternalImageChannel`].
pub struct ExternalImageChannelHandler(Arc<ExternalImageChannel>);

impl ExternalImageChannelHandler {
    pub fn new(channel: Arc<ExternalImageChannel>) -> Self {
        ExternalImageChannelHandler(channel)
    }
}

impl WebRenderExternalImageApi for ExternalImageChannelHandler {
    fn lock(&mut self, id: u64) -> (ExternalImageSource<'_>, UntypedSize2D<i32>) {
        match self.0.get(id) {
            Some(texture) => (
                ExternalImageSource::NativeTexture(texture.texture_id),
                UntypedSize2D::new(texture.width, texture.height),
            ),
            None => (
                ExternalImageSource::Invalid,
                UntypedSize2D::new(0, 0),
            ),
        }
    }

    fn unlock(&mut self, _id: u64) {}
}
