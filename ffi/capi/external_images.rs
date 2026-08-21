/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use servo_api::{EmbedderTexture, ExternalImageChannel};

use crate::protocol::ServoProtocolResponse;

/// An opaque handle to the registry of textures the embedder wants pages to be able to display.
// cbindgen:opaque
pub struct ExternalImages {
    pub(crate) inner: Arc<ExternalImageChannel>,
}

/// Creates a registry of textures to display in pages. Pass it to `servo_builder_set_external_images`
/// before building `Servo`.
///
/// The ownership of the returned handle is transferred to the caller, who must free it with
/// [`servo_external_images_free`].
#[unsafe(no_mangle)]
pub extern "C" fn servo_external_images_create() -> *mut ExternalImages {
    Box::into_raw(Box::new(ExternalImages {
        inner: ExternalImageChannel::new(),
    }))
}

/// Registers `texture_id`, a texture in the GPU context Servo renders with, and returns the
/// identifier to serve as the content of a resource. Returns 0 before `Servo` has been built.
///
/// # Safety
/// `images` must be a live handle from `servo_external_images_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_external_images_register(
    images: *mut ExternalImages,
    texture_id: u32,
    width: i32,
    height: i32,
) -> u64 {
    assert!(!images.is_null(), "images pointer must not be null");
    let images = unsafe { &*images };

    images
        .inner
        .register(EmbedderTexture {
            texture_id,
            width,
            height,
        })
        .unwrap_or(0)
}

/// Points `id` at a different texture, or at the same texture at a different size, from the next
/// time it is sampled.
///
/// # Safety
/// See [`servo_external_images_register`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_external_images_update(
    images: *mut ExternalImages,
    id: u64,
    texture_id: u32,
    width: i32,
    height: i32,
) {
    assert!(!images.is_null(), "images pointer must not be null");
    let images = unsafe { &*images };

    images.inner.update(
        id,
        EmbedderTexture {
            texture_id,
            width,
            height,
        },
    );
}

/// Forgets `id`. Pages still displaying it will stop being able to.
///
/// # Safety
/// See [`servo_external_images_register`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_external_images_unregister(images: *mut ExternalImages, id: u64) {
    assert!(!images.is_null(), "images pointer must not be null");
    let images = unsafe { &*images };

    images.inner.unregister(id);
}

/// Answers a protocol handler request with the texture registered as `id`, so that the requesting
/// document displays it as the resource's content.
///
/// # Safety
/// `response` must be the response of a call to a `ServoProtocolHandlerCallback`, and the filled in
/// data is only valid until that call returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_external_images_answer_request(
    response: *mut ServoProtocolResponse,
    id: u64,
    width: u32,
    height: u32,
) {
    assert!(!response.is_null(), "response pointer must not be null");

    // The body of an external image resource, as `image_cache` parses it.
    let mut body = [0u8; 16];
    body[0..8].copy_from_slice(&id.to_le_bytes());
    body[8..12].copy_from_slice(&width.to_le_bytes());
    body[12..16].copy_from_slice(&height.to_le_bytes());

    EXTERNAL_IMAGE_BODY.with(|storage| {
        let storage = &mut *storage.borrow_mut();
        *storage = body;

        let response = unsafe { &mut *response };
        response.data = storage.as_ptr();
        response.len = storage.len();
        response.content_type = EXTERNAL_IMAGE_CONTENT_TYPE_C.as_ptr();
    });
}

thread_local! {
    static EXTERNAL_IMAGE_BODY: std::cell::RefCell<[u8; 16]> = const { std::cell::RefCell::new([0u8; 16]) };
}

const EXTERNAL_IMAGE_CONTENT_TYPE_C: &std::ffi::CStr =
    c"application/x-servo-external-image";

/// # Safety
/// `images` must be a live handle from `servo_external_images_create` that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_external_images_free(images: *mut ExternalImages) {
    assert!(!images.is_null(), "images pointer must not be null");

    unsafe {
        let _ = Box::from_raw(images);
    }
}
