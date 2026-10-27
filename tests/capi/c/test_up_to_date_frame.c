/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#define _POSIX_C_SOURCE 200809L // required by `nanosleep` in `<time.h>`
#include "servo/servo_capi.h"
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <time.h>
#endif

#define log(fmt, ...)               \
    do {                            \
        printf(fmt, ##__VA_ARGS__); \
        fflush(stdout);             \
    } while (0)

static void sleep_ms(long period)
{
#ifdef _WIN32
    Sleep(period);
#else
    struct timespec delay;
    delay.tv_sec = period / 1000;
    delay.tv_nsec = (period % 1000) * 1000000;
    nanosleep(&delay, NULL);
#endif
}

typedef enum {
    TEST_STEP_STARTING = 0,
    TEST_STEP_HEAD_PARSED,
    TEST_STEP_LOAD_COMPLETE,
    TEST_STEP_FRAME_READY,
    TEST_STEP_FRAME_FAILED,
} test_step_t;

// State passed to the delegate callbacks.
typedef struct {
    // `step` is not atomic since it is modified by the delegate
    // callbacks that are run on the main thread during spin.
    test_step_t step;
} test_state_t;

static atomic_int waker_triggered;

static void wake_callback(void) { atomic_store(&waker_triggered, 1); }

// WebView delegate callbacks
static void on_load_status_changed(struct WebView* webview, int32_t load_status,
    void* user_data)
{
    (void)webview;
    log("  [delegate] load_status_changed -> %d\n", load_status);
    test_state_t* state = (test_state_t*)user_data;
    test_step_t new_step;
    switch (load_status) {
    case SERVO_LOAD_STATUS_STARTED:
        new_step = TEST_STEP_STARTING;
        break;
    case SERVO_LOAD_STATUS_HEAD_PARSED:
        new_step = TEST_STEP_HEAD_PARSED;
        break;
    case SERVO_LOAD_STATUS_COMPLETE:
        new_step = TEST_STEP_LOAD_COMPLETE;
        break;
    default:
        fprintf(stderr, "  FAIL: unexpected load_status %d\n", load_status);
        abort();
    }
    state->step = new_step;
}

static void on_up_to_date_frame(bool ready, int32_t error, void* user_data)
{
    test_state_t* state = (test_state_t*)user_data;
    if (!ready) {
        state->step = TEST_STEP_FRAME_FAILED;
        log("  [frame] failed with error=%d\n", error);
        return;
    }
    state->step = TEST_STEP_FRAME_READY;
    log("  [frame] ready\n");
}

/* Integration test that
 *  1. Creates a Servo instance
 *  2. Opens a WebView and waits for its page to load
 *  3. Waits for an up-to-date frame
 */
int test_webview_wait_for_up_to_date_frame(void)
{
    log("running test_webview_wait_for_up_to_date_frame\n");
    int result = 1; // default to fail
    struct Servo* servo = NULL;
    struct WebView* webview = NULL;
    test_state_t state;
    memset(&state, 0, sizeof(state));

    ServoBuilder* builder = servo_builder_create();

    struct ServoEventLoopWaker waker = { wake_callback };
    servo_builder_set_event_loop_waker(builder, waker);

    ServoOptions* options = servo_options_create();
    servo_options_set_multiprocess(options, false);
    servo_builder_set_options(builder, options);

    servo = servo_builder_build(builder);
    if (!servo) {
        log("  FAIL: servo_builder_build returned NULL\n");
        goto cleanup;
    }

    RenderingContext* context = servo_rendering_context_create_software(100, 100);
    if (!context) {
        log("  FAIL: servo_rendering_context_create_software returned NULL\n");
        goto cleanup;
    }

    ServoWebViewBuilder* webview_builder = servo_webview_builder_create(servo, context);
    if (!webview_builder) {
        log("  FAIL: servo_webview_builder_create returned NULL\n");
        goto cleanup;
    }

    ServoWebViewDelegate delegate = {
        &state,
        on_load_status_changed,
        NULL,
    };
    servo_webview_builder_set_delegate(webview_builder, delegate);

    const char* url = "data:text/html,<html><body style='background:red'></body></html>";
    if (servo_webview_builder_set_url(webview_builder, url) != 0) {
        log("  FAIL: servo_webview_builder_set_url failed for url '%s'\n", url);
        servo_webview_builder_free(webview_builder);
        goto cleanup;
    }

    webview = servo_webview_builder_build(webview_builder);
    if (!webview) {
        log("  FAIL: servo_webview_builder_build returned NULL\n");
        goto cleanup;
    }

    // Spin the event loop until the page loads or we timeout.
    log("  Spinning event loop... (timeout 30s)\n");
    {
        int elapsed_ms = 0;
        int poll_ms = 15;
        while (elapsed_ms < 30000) {
            servo_spin_event_loop(servo);
            if (state.step == TEST_STEP_LOAD_COMPLETE) {
                break;
            }
            sleep_ms(poll_ms);
            elapsed_ms += poll_ms;
        }
    }
    if (state.step != TEST_STEP_LOAD_COMPLETE) {
        log("  FAIL: page never finished loading (step=%d)\n", (int)state.step);
        goto cleanup;
    }

    servo_webview_wait_for_up_to_date_frame(webview, on_up_to_date_frame, &state);

    // Spin the event loop, painting new frames, until the frame is ready or we timeout.
    log("  Spinning event loop... (timeout 30s)\n");
    {
        int elapsed_ms = 0;
        int poll_ms = 15;
        while (elapsed_ms < 30000) {
            servo_spin_event_loop(servo);
            servo_webview_paint(webview);
            if (state.step >= TEST_STEP_FRAME_READY) {
                break;
            }
            sleep_ms(poll_ms);
            elapsed_ms += poll_ms;
        }
    }

    if (state.step == TEST_STEP_FRAME_FAILED) {
        log("  FAIL: waiting for an up-to-date frame failed\n");
        goto cleanup;
    }
    if (state.step != TEST_STEP_FRAME_READY) {
        log("  FAIL: up-to-date frame callback never fired (step=%d)\n", (int)state.step);
        goto cleanup;
    }

    result = 0;
    log("  PASS: the frame became ready\n");

cleanup:
    if (webview) {
        servo_webview_free(webview);
    }
    if (servo) {
        servo_free(servo);
    }
    return result;
}

// Entry point for the up-to-date frame integration test. This is called from the Rust-side test
// runner.
int run_c_up_to_date_frame_tests(void)
{
    int result = test_webview_wait_for_up_to_date_frame();
    log("%s\n", result == 0 ? "PASS" : "FAIL");
    return result;
}
