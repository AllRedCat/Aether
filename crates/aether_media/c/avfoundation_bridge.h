#ifndef AVFOUNDATION_BRIDGE_H
#define AVFOUNDATION_BRIDGE_H

#include <stdint.h>
#include <stddef.h>

typedef struct {
    uint32_t width;
    uint32_t height;
    double duration_seconds;
    int64_t duration_value;
    int32_t duration_timescale;
    double fps;
} AvfMediaInfo;

typedef struct {
    uint32_t width;
    uint32_t height;
    uint32_t bytes_per_row;
    int64_t pts_value;
    int32_t pts_timescale;
    int64_t duration_value;
    int32_t duration_timescale;
    int32_t is_keyframe;
    uint8_t *data;
    size_t data_len;
} AvfRawFrame;

void* avf_decoder_create(const char* file_path, AvfMediaInfo* out_info, char* err_buf, size_t err_len);
int avf_decoder_next_frame(void* handle, AvfRawFrame* out_frame);
void avf_decoder_free_frame(AvfRawFrame* frame);
int avf_decoder_seek_pts(void* handle, int64_t target_value, int32_t target_timescale);
void avf_decoder_destroy(void* handle);

typedef struct {
    uint32_t sample_rate;
    uint16_t channels;
    double duration_seconds;
    int64_t duration_value;
    int32_t duration_timescale;
} AvfAudioInfo;

typedef struct {
    uint32_t sample_rate;
    uint16_t channels;
    uint32_t frame_count;
    int64_t pts_value;
    int32_t pts_timescale;
    float *samples;
    size_t sample_count;
} AvfRawAudioBuffer;

void* avf_audio_decoder_create(const char* file_path, AvfAudioInfo* out_info, char* err_buf, size_t err_len);
int avf_audio_decoder_next_buffer(void* handle, AvfRawAudioBuffer* out_buf);
void avf_audio_decoder_free_buffer(AvfRawAudioBuffer* buf);
int avf_audio_decoder_seek_pts(void* handle, int64_t target_value, int32_t target_timescale);
void avf_audio_decoder_destroy(void* handle);

#endif /* AVFOUNDATION_BRIDGE_H */
