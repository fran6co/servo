/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::ffi::c_void;

use servo_api::{DeviceIntRect, DeviceIntSize, WebView};

/// Reads the rendering context as it is right now, unlike
/// `servo_webview_take_screenshot` which first waits for the page to reach a stable state.
///
/// # Safety
/// `webview` must be a live handle from `servo_webview_builder_build`, used on its creating thread.
/// `callback` must not unwind. `data` is tightly packed RGBA8, top row first, `NULL` on failure,
/// and only valid for the duration of the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_read_frame(
    webview: *mut WebView,
    callback: Option<
        unsafe extern "C" fn(data: *const u8, width: u32, height: u32, user_data: *mut c_void),
    >,
    user_data: *mut c_void,
) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    let Some(callback) = callback else {
        return;
    };

    let rendering_context = webview.rendering_context();
    let size = rendering_context.size();
    let rect = DeviceIntRect::from_size(DeviceIntSize::new(size.width as i32, size.height as i32));

    match rendering_context.read_to_image(rect) {
        Some(image) => {
            let (width, height) = image.dimensions();
            let data = image.into_raw();
            unsafe {
                callback(data.as_ptr(), width, height, user_data);
            }
        },
        None => unsafe {
            callback(std::ptr::null(), 0, 0, user_data);
        },
    }
}

/// # Safety
/// See [`servo_webview_read_frame`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_resize(webview: *mut WebView, width: u32, height: u32) {
    assert!(!webview.is_null(), "webview pointer must not be null");

    if width == 0 || height == 0 {
        log::error!("servo_webview_resize: refusing to resize to {width}x{height}");
        return;
    }

    let webview = unsafe { &*webview };

    // The rendering context is not resized here: `WebView::resize` does that itself, and only
    // treats the resize as new when the context still has the old size.
    webview.resize(dpi::PhysicalSize::new(width, height));
}

/// Swaps the back buffer of the `WebView`'s rendering context, making the frame just painted the
/// front one.
///
/// # Safety
/// See [`servo_webview_read_frame`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_present(webview: *mut WebView) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    webview.rendering_context().present();
}
