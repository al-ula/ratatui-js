/* A deliberately malformed library fixture. Destroy aborts if the adapter has
 * leaked any owned buffers; the real-library PTY tests cover terminal behavior. */
#include "ratatui_js.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#define API __declspec(dllexport)
#else
#define API
#endif
static int allocations;
static int closed;
static int token;
static RtBytes owned(const char *text) {
    size_t len = strlen(text);
    uint8_t *bytes = malloc(len); assert(bytes); memcpy(bytes, text, len);
    allocations++; RtBytes result = {bytes, len}; return result;
}
API uint32_t rt_abi_version(void) { return 1; }
API uint32_t rt_protocol_version(void) { return 1; }
API uint32_t rt_create(uint32_t a, uint32_t p, uint32_t f, RtSession **s, RtBytes *e) {
    (void)a; (void)p; (void)f; *s = (RtSession *)&token; e->data = NULL; e->len = 0; closed = 0; return RT_OK;
}
API uint32_t rt_render(const RtSession *s, const uint8_t *d, size_t n, RtBytes *o, RtBytes *e) {
    (void)s; (void)d; (void)n; *o = owned("{"); e->data = NULL; e->len = 0; return RT_OK;
}
API uint32_t rt_poll_event(const RtSession *s, uint32_t t, RtBytes *o, RtBytes *e) {
    (void)s; (void)t; *o = owned("{\"type\":\"key\",\"key\":{\"type\":\"character\",\"value\":\"ab\"},\"kind\":\"press\",\"modifiers\":[]}");
    e->data = NULL; e->len = 0; return RT_OK;
}
API uint32_t rt_close(const RtSession *s, RtBytes *e) {
    (void)s; closed++;
    *e = owned("{\"code\":\"shutdown\",\"message\":\"restore failed\",\"cleanup\":[{\"operation\":\"raw mode\",\"message\":\"failed\"}]}");
    return RT_ERROR;
}
API uint32_t rt_destroy(RtSession *s, RtBytes *e) {
    (void)s; assert(allocations == 0 && closed == 1); e->data = NULL; e->len = 0; return RT_OK;
}
API void rt_bytes_free(uint8_t *d, size_t n) { (void)n; if (d) { free(d); allocations--; } }
