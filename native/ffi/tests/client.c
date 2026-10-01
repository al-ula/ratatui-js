#include "ratatui_js.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
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
#ifdef _WIN32
static DWORD WINAPI windows_poller(LPVOID session) { poller(session); return 0; }
#endif
int main(int argc, char **argv) {
    const char *scenario = argc > 1 ? argv[1] : "keyboard";
#ifdef _WIN32
    DWORD input_mode, output_mode;
    assert(GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &input_mode));
    assert(GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &output_mode));
#endif
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
    /* Oversized lengths are rejected before dereferencing input. Frame codes
       and paths are preserved even at this boundary. */
    assert(rt_render(session, frame, 1048577, &output, &error) == RT_ERROR);
    assert(contains(error, "invalidFrame") && contains(error, "\"path\":\"$\"")); release(&error);
    assert(rt_render(session, (const uint8_t *)"{", 1, &output, &error) == RT_ERROR);
    assert(contains(error, "invalidJson")); release(&error);
    assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_OK);
    assert(contains(output, "\"width\":80")); release(&output);
    assert(rt_poll_event(session, 0, &output, &error) == RT_TIMEOUT);
    if (!strcmp(scenario, "close-wait")) {
#ifdef _WIN32
        HANDLE thread = CreateThread(NULL, 0, windows_poller, session, 0, NULL);
        assert(thread);
#else
        pthread_t thread; assert(!pthread_create(&thread, NULL, poller, session));
#endif
        int waiting = 0;
        for (int i = 0; i < 5000; i++) {
            uint32_t status = rt_poll_event(session, 0, &output, &error);
            if (status == RT_ERROR) { assert(contains(error, "concurrentEventWait")); waiting = 1; release(&error); break; }
            assert(status == RT_TIMEOUT);
#ifdef _WIN32
            Sleep(1);
#else
            struct timespec delay = {0, 1000000}; nanosleep(&delay, NULL);
#endif
        }
        assert(waiting);
        assert(rt_render(session, frame, sizeof(frame)-1, &output, &error) == RT_OK); release(&output);
        assert(rt_close(session, &error) == RT_OK);
#ifdef _WIN32
        assert(WaitForSingleObject(thread, 5000) == WAIT_OBJECT_0); CloseHandle(thread);
#else
        assert(!pthread_join(thread, NULL));
#endif
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
#ifdef _WIN32
    DWORD restored;
    assert(GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &restored) && restored == input_mode);
    assert(GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &restored) && restored == output_mode);
    puts("MODES_RESTORED"); fflush(stdout);
#endif
    return 0;
}
