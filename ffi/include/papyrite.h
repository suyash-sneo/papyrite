#ifndef PAPYRITE_H
#define PAPYRITE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PAPYRITE_STATUS_OK 0
#define PAPYRITE_STATUS_INVALID_ARGUMENT 1
#define PAPYRITE_STATUS_ENGINE_ERROR 2
#define PAPYRITE_STATUS_PANIC 3

typedef struct PapyriteBuffer {
    uint8_t *ptr;
    size_t len;
} PapyriteBuffer;

typedef struct PapyriteResult {
    int32_t code;
    PapyriteBuffer data;
    PapyriteBuffer error;
    uint8_t bool_value;
} PapyriteResult;

int32_t papyrite_create_json(
    const uint8_t *path_ptr,
    size_t path_len,
    const uint8_t *json_ptr,
    size_t json_len,
    PapyriteResult *out
);

int32_t papyrite_get_json(
    const uint8_t *path_ptr,
    size_t path_len,
    const uint8_t *json_ptr,
    size_t json_len,
    PapyriteResult *out
);

int32_t papyrite_delete_json(
    const uint8_t *path_ptr,
    size_t path_len,
    const uint8_t *json_ptr,
    size_t json_len,
    PapyriteResult *out
);

int32_t papyrite_update_json(
    const uint8_t *path_ptr,
    size_t path_len,
    const uint8_t *json_ptr,
    size_t json_len,
    PapyriteResult *out
);

int32_t papyrite_find_json(
    const uint8_t *path_ptr,
    size_t path_len,
    const uint8_t *json_ptr,
    size_t json_len,
    PapyriteResult *out
);

int32_t papyrite_dump_json(
    const uint8_t *path_ptr,
    size_t path_len,
    PapyriteResult *out
);

void papyrite_buffer_free(uint8_t *ptr, size_t len);
void papyrite_result_free(PapyriteResult *result);

#ifdef __cplusplus
}
#endif

#endif
