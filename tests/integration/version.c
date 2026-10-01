#ifdef _WIN32
#define RT_API __declspec(dllexport)
#endif
#include "ratatui_js.h"
#define API RT_API
#ifndef ABI
#define ABI 1
#endif
#ifndef PROTOCOL
#define PROTOCOL 1
#endif
API uint32_t rt_abi_version(void) { return ABI; }
API uint32_t rt_protocol_version(void) { return PROTOCOL; }
/* Version negotiation must fail before any lifecycle operation is reached. */
#include <stdlib.h>
API uint32_t rt_create(uint32_t a, uint32_t p, uint32_t f, RtSession **s, RtBytes *e) { (void)a; (void)p; (void)f; (void)s; (void)e; abort(); }
API uint32_t rt_render(const RtSession *s, const uint8_t *d, size_t n, RtBytes *o, RtBytes *e) { (void)s; (void)d; (void)n; (void)o; (void)e; abort(); }
API uint32_t rt_poll_event(const RtSession *s, uint32_t t, RtBytes *o, RtBytes *e) { (void)s; (void)t; (void)o; (void)e; abort(); }
API uint32_t rt_close(const RtSession *s, RtBytes *e) { (void)s; (void)e; abort(); }
API uint32_t rt_destroy(RtSession *s, RtBytes *e) { (void)s; (void)e; abort(); }
API void rt_bytes_free(uint8_t *d, size_t n) { (void)d; (void)n; abort(); }
