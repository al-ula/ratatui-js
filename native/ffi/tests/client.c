#include "ratatui_js.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <time.h>
static const uint8_t frame[] = "{\"protocolVersion\":1,\"root\":{\"type\":\"paragraph\",\"lines\":[[{\"text\":\"PTY frame\"}]]}}";
static void release(RtBytes *bytes) {
    rt_bytes_free(bytes->data, bytes->len);
    bytes->data = NULL; bytes->len = 0;
}
static int contains(RtBytes bytes, const char *text) {
    char *copy = malloc(bytes.len + 1); assert(copy);
    memcpy(copy, bytes.data, bytes.len); copy[bytes.len] = 0;
    int found = strstr(copy, text) != NULL; free(copy); return found;
}
static void *poller(void *session) {
    RtBytes output = {0}, error = {0};
    assert(rt_poll_event(session, 60000, &output, &error) == RT_CLOSED);
    release(&output); release(&error); return NULL;
}
int main(int argc, char **argv) {
    const char *scenario = argc > 1 ? argv[1] : "keyboard";
    RtSession *session = NULL; RtBytes output = {0}, error = {0};
    assert(rt_abi_version() == RT_ABI_VERSION);
    assert(rt_protocol_version() == RT_PROTOCOL_VERSION);
    assert(rt_create(99, 1, 1, &session, &error) == RT_ERROR);
    assert(!session && contains(error, "unsupportedAbi")); release(&error);
    assert(rt_create(1, 99, 1, &session, &error) == RT_ERROR);
    assert(!session && contains(error, "unsupportedProtocol")); release(&error);
    assert(rt_create(1, 1, 99, &session, &error) == RT_ERROR);
    assert(!session && contains(error, "invalidArgument")); release(&error);
    assert(rt_render(NULL, frame, sizeof(frame)-1, &output, &error) == RT_ERROR);
    assert(contains(error, "invalidArgument")); release(&error);
    assert(rt_create(1, 1, strcmp(scenario, "no-alternate") ? 1 : 0, &session, &error) == RT_OK);
    RtSession *second = NULL;
    assert(rt_create(1, 1, 1, &second, &error) == RT_ERROR);
    assert(contains(error, "terminalBusy")); release(&error);
    assert(rt_render(session, NULL, 0, &output, &error) == RT_ERROR); release(&error);
    assert(rt_render(session, (const uint8_t *)"{", 1, &output, &error) == RT_ERROR);
    assert(contains(error, "invalidJson")); release(&error);
    assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_OK);
    assert(contains(output, "\"width\":80")); release(&output);
    assert(rt_poll_event(session, 0, &output, &error) == RT_TIMEOUT);
    if (!strcmp(scenario, "close-wait")) {
        pthread_t thread; assert(!pthread_create(&thread, NULL, poller, session));
        int waiting = 0;
        for (int i = 0; i < 5000; i++) {
            uint32_t status = rt_poll_event(session, 0, &output, &error);
            if (status == RT_ERROR) { assert(contains(error, "concurrentEventWait")); waiting = 1; release(&error); break; }
            assert(status == RT_TIMEOUT);
            struct timespec delay = {0, 1000000}; nanosleep(&delay, NULL);
        }
        assert(waiting);
        assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_OK); release(&output);
        assert(rt_close(session, &error) == RT_OK);
        assert(!pthread_join(thread, NULL));
    } else {
        puts("PTY_READY\r"); fflush(stdout);
        for (;;) {
            uint32_t status = rt_poll_event(session, 100, &output, &error);
            if (status == RT_TIMEOUT) continue;
            assert(status == RT_OK);
            if (contains(output, "\"type\":\"resize\"")) {
                assert(contains(output, "\"width\":90")); release(&output);
                assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_OK);
                assert(contains(output, "\"height\":30")); release(&output);
                puts("PTY_RESIZED\r"); fflush(stdout);
            } else {
                assert(contains(output, "\"value\":\"q\"")); release(&output); break;
            }
        }
    }
    assert(rt_close(session, &error) == RT_OK);
    assert(rt_close(session, &error) == RT_OK);
    assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_ERROR);
    assert(contains(error, "closed")); release(&error);
    assert(rt_poll_event(session, 0, &output, &error) == RT_CLOSED);
    assert(rt_destroy(session, &error) == RT_OK); release(&error);
    rt_bytes_free(NULL, 0);
    return 0;
}
