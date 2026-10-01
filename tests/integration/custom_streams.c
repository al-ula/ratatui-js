#include "ratatui_js.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static void release(RtBytes *bytes) { rt_bytes_free(bytes->data, bytes->len); bytes->data = NULL; bytes->len = 0; }
static int contains(RtBytes bytes, const char *text) {
    size_t len = strlen(text);
    for (size_t i = 0; i + len <= bytes.len; i++) if (!memcmp(bytes.data + i, text, len)) return 1;
    return 0;
}
int main(void) {
    const char *scenario = getenv("RATATUI_JS_CUSTOM_SCENARIO");
    int fd = atoi(getenv("RATATUI_JS_CUSTOM_FD"));
    int output_fd = atoi(getenv("RATATUI_JS_CUSTOM_OUTPUT_FD"));
    int marker = dup(output_fd); assert(marker >= 0);
    int cleanup = !strcmp(scenario, "cleanup");
    int selected = !strcmp(scenario, "tty") ? RT_STREAM_TTY : fd;
    RtSession *session = NULL;
    RtBytes error = {0}, output = {0};
    if (!strcmp(scenario, "invalid")) {
        int closed = dup(fd); assert(closed >= 0); assert(close(closed) == 0);
        assert(rt_create_with_streams(1, 1, 1, closed, selected, &session, &error) == RT_ERROR);
        assert(!session && contains(error, "\"code\":\"io\"")); release(&error);
        assert(rt_create_with_streams(1, 1, 1, -3, selected, &session, &error) == RT_ERROR);
        assert(!session && contains(error, "invalidArgument")); release(&error);
    }
    if (!strcmp(scenario, "rollback")) {
        assert(rt_create_with_streams(1, 1, 17, selected, selected, &session, &error) == RT_ERROR);
        assert(!session && contains(error, "unsupportedCapability")); release(&error);
    }
    if (!strcmp(scenario, "render-failure")) {
        assert(rt_create_with_streams(1, 1, 1, selected, selected, &session, &error) == RT_ERROR);
        assert(!session && contains(error, "create renderer")); release(&error);
        close(marker); return 0;
    }
    int extended = !strcmp(scenario, "extended");
    assert(rt_create_with_streams(1, 1, extended ? 31 : 1, selected, cleanup ? output_fd : selected, &session, &error) == RT_OK);
    if (!strcmp(scenario, "invalid") || !strcmp(scenario, "rollback")) {
        assert(rt_destroy(session, &error) == RT_OK);
        close(marker); return 0;
    }
    assert(close(fd) == 0);
    const char *frame = "{\"protocolVersion\":1,\"root\":{\"type\":\"paragraph\",\"lines\":[[{\"text\":\"CUSTOM_READY\"}]]}}";
    assert(rt_render(session, (const uint8_t *)frame, strlen(frame), &output, &error) == RT_OK);
    assert(contains(output, "\"width\":80")); release(&output);
    if (cleanup) {
        assert(write(marker, "CUSTOM_CLEANUP", 14) == 14);
        (void)rt_poll_event(session, 5000, &output, &error); release(&output); release(&error);
        assert(rt_close(session, &error) == RT_ERROR);
        assert(contains(error, "shutdown")); release(&error);
        assert(rt_destroy(session, &error) == RT_ERROR); release(&error);
        assert(rt_create_with_streams(1, 1, 1, marker, marker, &session, &error) == RT_ERROR);
        assert(contains(error, "terminalPoisoned")); release(&error);
        close(marker); return 0;
    }
    assert(rt_poll_event(session, 5000, &output, &error) == RT_OK);
    assert(contains(output, "\"type\":\"resize\"") && contains(output, "\"width\":90")); release(&output);
    assert(rt_render(session, (const uint8_t *)frame, strlen(frame), &output, &error) == RT_OK);
    assert(contains(output, "\"width\":90")); release(&output);
    assert(write(marker, "CUSTOM_RESIZED", 14) == 14);
    if (extended) {
        const char *types[] = {"focus", "focus", "paste", "paste", "mouse", "mouse", "mouse", "mouse", "mouse", "mouse", "mouse", "mouse", "key", "key"};
        for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
            assert(rt_poll_event(session, 5000, &output, &error) == RT_OK);
            assert(contains(output, types[i])); release(&output);
        }
    } else {
        assert(rt_poll_event(session, 5000, &output, &error) == RT_OK);
        assert(contains(output, "\"value\":\"q\"")); release(&output);
    }
    if (strcmp(scenario, "drop")) {
        assert(rt_close(session, &error) == RT_OK);
        assert(rt_close(session, &error) == RT_OK);
    }
    assert(rt_destroy(session, &error) == RT_OK);
    assert(rt_create_with_streams(1, 1, 1, marker, marker, &session, &error) == RT_OK);
    assert(rt_destroy(session, &error) == RT_OK);
    close(marker); return 0;
}
