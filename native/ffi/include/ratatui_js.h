#ifndef RATATUI_JS_H
#define RATATUI_JS_H
#include <stdint.h>
#include <stddef.h>
/* DLL implementations can supply __declspec(dllexport) before inclusion. */
#ifndef RT_API
#define RT_API
#endif
#ifdef __cplusplus
extern "C" {
#endif
#define RT_ABI_VERSION 1u
#define RT_PROTOCOL_VERSION 1u
#define RT_OK 0u
#define RT_TIMEOUT 1u
#define RT_CLOSED 2u
#define RT_ERROR 3u
#define RT_ALTERNATE_SCREEN 1u
#define RT_MOUSE_CAPTURE 2u
#define RT_BRACKETED_PASTE 4u
#define RT_FOCUS_REPORTING 8u
/* Probe support; unsupported terminals fail creation instead of downgrading. */
#define RT_ENHANCED_KEYBOARD 16u
#define RT_STREAM_STANDARD (-1)
#define RT_STREAM_TTY (-2)
typedef struct RtSession RtSession;
typedef struct { uint8_t *data; size_t len; } RtBytes;
/* All output arguments are required, aligned, writable, and disjoint. They
 * must initially hold no unreleased buffers. Null arguments are rejected.
 * Other invalid/forged pointers cannot be detected and are undefined behavior.
 * Input bytes are borrowed for the duration of the call.
 * Outputs/errors are owned; free each exact pair once using rt_bytes_free.
 * Error payloads are UTF-8 JSON; no last-error global exists.
 * Render/poll/close may overlap. Only one poll may be outstanding.
 * Close wakes polls and is idempotent, including cleanup errors.
 * Destroy requires all outstanding calls to have returned. It consumes the
 * handle even on error; never reuse it. Call close explicitly to observe errors.
 * Never unload while handles, calls, or buffers exist. */
RT_API uint32_t rt_abi_version(void);
RT_API uint32_t rt_protocol_version(void);
RT_API uint32_t rt_create(uint32_t abi, uint32_t protocol, uint32_t flags, RtSession **session, RtBytes *error);
/* Additive ABI 1 entrypoint. Unix only for nonstandard selections; Windows
 * supports STANDARD/ STANDARD. Selected handles must be blocking terminals with
 * read/write access appropriate to their direction. Caller retains originals,
 * keeps them valid until return, and must not read or change modes concurrently.
 * The session owns close-on-exec duplicates until destroy; close restores modes
 * before those handles are released. One session per library still applies. */
RT_API uint32_t rt_create_with_streams(uint32_t abi, uint32_t protocol, uint32_t flags, int32_t input, int32_t output, RtSession **session, RtBytes *error);
RT_API uint32_t rt_render(const RtSession *session, const uint8_t *data, size_t len, RtBytes *output, RtBytes *error);
RT_API uint32_t rt_poll_event(const RtSession *session, uint32_t timeout_ms, RtBytes *output, RtBytes *error);
RT_API uint32_t rt_close(const RtSession *session, RtBytes *error);
RT_API uint32_t rt_destroy(RtSession *session, RtBytes *error);
RT_API void rt_bytes_free(uint8_t *data, size_t len);
#ifdef __cplusplus
}
#endif
#endif
