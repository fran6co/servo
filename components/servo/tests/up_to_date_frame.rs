/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tests for `WebView::wait_for_up_to_date_frame`.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use euclid::default::Point2D;
use image::Rgba;
use servo::{DeviceIntRect, WebView, WebViewBuilder};
use url::Url;

use crate::common::{
    ServoTest, WebViewDelegateImpl, evaluate_javascript,
    show_webview_and_wait_for_rendering_to_be_ready,
};

/// Waits for an up-to-date frame and spins until the callback is called, returning the color of
/// the top-left pixel of the rendering at the time of the callback.
fn up_to_date_frame_pixel(servo_test: &ServoTest, webview: &WebView) -> Rgba<u8> {
    let pixel = Rc::new(RefCell::new(None));
    let callback_pixel = pixel.clone();
    let rendering_context = webview.rendering_context();
    webview.wait_for_up_to_date_frame(move |result| {
        assert!(
            result.is_ok(),
            "waiting for an up-to-date frame failed: {result:?}"
        );
        let image = rendering_context
            .read_to_image(DeviceIntRect::new(
                Point2D::origin().cast_unit(),
                Point2D::new(1, 1).cast_unit(),
            ))
            .expect("Should be able to read image");
        *callback_pixel.borrow_mut() = image.pixels().next().copied();
    });
    servo_test.spin(|| pixel.borrow().is_none());
    let pixel = pixel.borrow().expect("Should have read a pixel");
    pixel
}

#[test]
fn test_wait_for_up_to_date_frame_reflects_prior_changes() {
    let servo_test = ServoTest::new();
    let delegate = Rc::new(WebViewDelegateImpl::default());
    let webview = WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone())
        .delegate(delegate.clone())
        .url(Url::parse("data:text/html,<body style='margin: 0'></body>").unwrap())
        .build();
    show_webview_and_wait_for_rendering_to_be_ready(&servo_test, &webview, &delegate);

    for (color, expected) in [
        ("rgb(255, 0, 0)", Rgba([255, 0, 0, 255])),
        ("rgb(0, 0, 255)", Rgba([0, 0, 255, 255])),
    ] {
        let _ = evaluate_javascript(
            &servo_test,
            webview.clone(),
            format!("document.body.style.background = '{color}';"),
        );
        assert_eq!(up_to_date_frame_pixel(&servo_test, &webview), expected);
    }
}
