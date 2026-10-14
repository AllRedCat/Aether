#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#include "avfoundation_bridge.h"

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"

@interface AvfBridgeDecoder : NSObject
@property (nonatomic, strong) AVURLAsset *asset;
@property (nonatomic, strong) AVAssetTrack *videoTrack;
@property (nonatomic, strong) AVAssetReader *reader;
@property (nonatomic, strong) AVAssetReaderTrackOutput *output;
@property (nonatomic, assign) AvfMediaInfo info;
@property (nonatomic, assign) uint32_t targetWidth;
@property (nonatomic, assign) uint32_t targetHeight;
@end

@implementation AvfBridgeDecoder
@end

void* avf_decoder_create_with_max_size(const char* file_path, uint32_t max_width, uint32_t max_height, AvfMediaInfo* out_info, char* err_buf, size_t err_len) {
    @autoreleasepool {
        if (!file_path) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "Null file path");
            return NULL;
        }

        NSString *pathStr = [NSString stringWithUTF8String:file_path];
        if (!pathStr) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "Invalid UTF-8 file path: %s", file_path);
            return NULL;
        }

        if (![[NSFileManager defaultManager] fileExistsAtPath:pathStr]) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "File does not exist: %s", file_path);
            return NULL;
        }

        NSURL *url = [NSURL fileURLWithPath:pathStr];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
        NSArray<AVAssetTrack *> *tracks = [asset tracksWithMediaType:AVMediaTypeVideo];
        if (tracks.count == 0) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "No video track found in media file: %s", file_path);
            return NULL;
        }

        AVAssetTrack *track = tracks.firstObject;
        AvfBridgeDecoder *dec = [[AvfBridgeDecoder alloc] init];
        dec.asset = asset;
        dec.videoTrack = track;

        CGSize naturalSize = track.naturalSize;
        double fps = track.nominalFrameRate > 0.0 ? (double)track.nominalFrameRate : 30.0;
        CMTime duration = asset.duration;
        if (duration.timescale <= 0 || track.naturalTimeScale <= 0) {
            if (err_buf && err_len > 0) {
                snprintf(err_buf, err_len, "Invalid container or track timescale (<= 0)");
            }
            return NULL;
        }
        double durationSeconds = CMTimeGetSeconds(duration);
        if (isnan(durationSeconds) || durationSeconds < 0.0) {
            durationSeconds = 0.0;
        }

        uint32_t orig_w = (uint32_t)naturalSize.width;
        uint32_t orig_h = (uint32_t)naturalSize.height;
        uint32_t target_w = orig_w;
        uint32_t target_h = orig_h;

        if (max_width > 0 && max_height > 0 && (orig_w > max_width || orig_h > max_height)) {
            double scale_w = (double)max_width / (double)orig_w;
            double scale_h = (double)max_height / (double)orig_h;
            double scale = scale_w < scale_h ? scale_w : scale_h;
            target_w = ((uint32_t)round(orig_w * scale)) & ~1;
            target_h = ((uint32_t)round(orig_h * scale)) & ~1;
            if (target_w < 2) target_w = 2;
            if (target_h < 2) target_h = 2;
        }

        dec.targetWidth = target_w;
        dec.targetHeight = target_h;

        dec.info = (AvfMediaInfo){
            .width = target_w,
            .height = target_h,
            .duration_seconds = durationSeconds,
            .duration_value = duration.value,
            .duration_timescale = duration.timescale,
            .fps = fps,
        };

        if (out_info) {
            *out_info = dec.info;
        }

        NSError *error = nil;
        dec.reader = [AVAssetReader assetReaderWithAsset:dec.asset error:&error];
        if (!dec.reader || error) {
            if (err_buf && err_len > 0) {
                snprintf(err_buf, err_len, "AVAssetReader init failed: %s",
                         error.localizedDescription.UTF8String ?: "Unknown error");
            }
            return NULL;
        }

        NSMutableDictionary *settings = [NSMutableDictionary dictionaryWithDictionary:@{
            (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)
        }];
        if (target_w > 0 && target_h > 0) {
            settings[(id)kCVPixelBufferWidthKey] = @(target_w);
            settings[(id)kCVPixelBufferHeightKey] = @(target_h);
        }

        dec.output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:dec.videoTrack
                                                               outputSettings:settings];
        dec.output.alwaysCopiesSampleData = NO;
        [dec.reader addOutput:dec.output];

        if (![dec.reader startReading]) {
            if (err_buf && err_len > 0) {
                snprintf(err_buf, err_len, "AVAssetReader failed to start reading");
            }
            return NULL;
        }

        return (__bridge_retained void*)dec;
    }
}

void* avf_decoder_create(const char* file_path, AvfMediaInfo* out_info, char* err_buf, size_t err_len) {
    return avf_decoder_create_with_max_size(file_path, 0, 0, out_info, err_buf, err_len);
}

int avf_decoder_next_frame(void* handle, AvfRawFrame* out_frame) {
    if (!handle || !out_frame) return -1;
    AvfBridgeDecoder *dec = (__bridge AvfBridgeDecoder*)handle;

    @autoreleasepool {
        CMSampleBufferRef sampleBuffer = [dec.output copyNextSampleBuffer];
        if (!sampleBuffer) {
            if (dec.reader.status == AVAssetReaderStatusFailed) {
                return -1;
            }
            return 0; // End of stream or reading completed
        }

        CMTime pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
        CMTime duration = CMSampleBufferGetDuration(sampleBuffer);

        int32_t isKeyframe = 1;
        CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
        if (attachments != NULL && CFArrayGetCount(attachments) > 0) {
            CFDictionaryRef dict = (CFDictionaryRef)CFArrayGetValueAtIndex(attachments, 0);
            if (dict != NULL) {
                CFBooleanRef notSync = (CFBooleanRef)CFDictionaryGetValue(dict, kCMSampleAttachmentKey_NotSync);
                if (notSync != NULL && CFBooleanGetValue(notSync)) {
                    isKeyframe = 0;
                }
            }
        }

        CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
        if (!imageBuffer) {
            CFRelease(sampleBuffer);
            return -1;
        }

        CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
        uint32_t width = (uint32_t)CVPixelBufferGetWidth(imageBuffer);
        uint32_t height = (uint32_t)CVPixelBufferGetHeight(imageBuffer);
        uint32_t bytesPerRow = (uint32_t)CVPixelBufferGetBytesPerRow(imageBuffer);
        uint8_t *baseAddr = (uint8_t*)CVPixelBufferGetBaseAddress(imageBuffer);

        size_t totalBytes = (size_t)bytesPerRow * height;
        uint8_t *copyData = (uint8_t*)malloc(totalBytes);
        if (!copyData) {
            CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
            CFRelease(sampleBuffer);
            return -1;
        }
        memcpy(copyData, baseAddr, totalBytes);

        CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
        CFRelease(sampleBuffer);

        out_frame->width = width;
        out_frame->height = height;
        out_frame->bytes_per_row = bytesPerRow;
        out_frame->pts_value = pts.value;
        out_frame->pts_timescale = pts.timescale;
        out_frame->duration_value = duration.value;
        out_frame->duration_timescale = duration.timescale;
        out_frame->is_keyframe = isKeyframe;
        out_frame->data = copyData;
        out_frame->data_len = totalBytes;

        return 1; // Frame decoded successfully
    }
}

int avf_decoder_next_frame_at_or_after(void* handle, int64_t min_pts_value, int32_t min_pts_timescale, AvfRawFrame* out_frame) {
    if (!handle || !out_frame) return -1;
    AvfBridgeDecoder *dec = (__bridge AvfBridgeDecoder*)handle;

    @autoreleasepool {
        CMTime minTime = min_pts_timescale > 0 ? CMTimeMake(min_pts_value, min_pts_timescale) : kCMTimeZero;

        while (1) {
            CMSampleBufferRef sampleBuffer = [dec.output copyNextSampleBuffer];
            if (!sampleBuffer) {
                if (dec.reader.status == AVAssetReaderStatusFailed) {
                    return -1;
                }
                return 0; // End of stream or reading completed
            }

            CMTime pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
            if (min_pts_timescale > 0 && CMTimeCompare(pts, minTime) < 0) {
                // PTS is before target: skip pixel lock, malloc, and memcpy!
                CFRelease(sampleBuffer);
                continue;
            }

            CMTime duration = CMSampleBufferGetDuration(sampleBuffer);

            int32_t isKeyframe = 1;
            CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, false);
            if (attachments != NULL && CFArrayGetCount(attachments) > 0) {
                CFDictionaryRef dict = (CFDictionaryRef)CFArrayGetValueAtIndex(attachments, 0);
                if (dict != NULL) {
                    CFBooleanRef notSync = (CFBooleanRef)CFDictionaryGetValue(dict, kCMSampleAttachmentKey_NotSync);
                    if (notSync != NULL && CFBooleanGetValue(notSync)) {
                        isKeyframe = 0;
                    }
                }
            }

            CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
            if (!imageBuffer) {
                CFRelease(sampleBuffer);
                return -1;
            }

            CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
            uint32_t width = (uint32_t)CVPixelBufferGetWidth(imageBuffer);
            uint32_t height = (uint32_t)CVPixelBufferGetHeight(imageBuffer);
            uint32_t bytesPerRow = (uint32_t)CVPixelBufferGetBytesPerRow(imageBuffer);
            uint8_t *baseAddr = (uint8_t*)CVPixelBufferGetBaseAddress(imageBuffer);

            size_t totalBytes = (size_t)bytesPerRow * height;
            uint8_t *copyData = (uint8_t*)malloc(totalBytes);
            if (!copyData) {
                CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
                CFRelease(sampleBuffer);
                return -1;
            }
            memcpy(copyData, baseAddr, totalBytes);

            CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
            CFRelease(sampleBuffer);

            out_frame->width = width;
            out_frame->height = height;
            out_frame->bytes_per_row = bytesPerRow;
            out_frame->pts_value = pts.value;
            out_frame->pts_timescale = pts.timescale;
            out_frame->duration_value = duration.value;
            out_frame->duration_timescale = duration.timescale;
            out_frame->is_keyframe = isKeyframe;
            out_frame->data = copyData;
            out_frame->data_len = totalBytes;

            return 1; // Frame decoded successfully
        }
    }
}

void avf_decoder_free_frame(AvfRawFrame* frame) {
    if (frame && frame->data) {
        free(frame->data);
        frame->data = NULL;
        frame->data_len = 0;
    }
}

int avf_decoder_seek_pts(void* handle, int64_t target_value, int32_t target_timescale) {
    if (!handle) return -1;
    AvfBridgeDecoder *dec = (__bridge AvfBridgeDecoder*)handle;

    @autoreleasepool {
        [dec.reader cancelReading];

        CMTime seekTime = CMTimeMake(target_value, target_timescale);
        NSError *error = nil;
        dec.reader = [AVAssetReader assetReaderWithAsset:dec.asset error:&error];
        if (!dec.reader || error) return -1;

        dec.reader.timeRange = CMTimeRangeMake(seekTime, kCMTimePositiveInfinity);

        NSMutableDictionary *settings = [NSMutableDictionary dictionaryWithDictionary:@{
            (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)
        }];
        if (dec.targetWidth > 0 && dec.targetHeight > 0) {
            settings[(id)kCVPixelBufferWidthKey] = @(dec.targetWidth);
            settings[(id)kCVPixelBufferHeightKey] = @(dec.targetHeight);
        }
        dec.output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:dec.videoTrack
                                                               outputSettings:settings];
        dec.output.alwaysCopiesSampleData = NO;
        [dec.reader addOutput:dec.output];

        if (![dec.reader startReading]) {
            return -1;
        }
        return 0;
    }
}

void avf_decoder_destroy(void* handle) {
    if (!handle) return;
    @autoreleasepool {
        AvfBridgeDecoder *dec = (__bridge_transfer AvfBridgeDecoder*)handle;
        [dec.reader cancelReading];
        dec.reader = nil;
        dec.output = nil;
        dec.videoTrack = nil;
        dec.asset = nil;
    }
}

@interface AvfBridgeAudioDecoder : NSObject
@property (nonatomic, strong) AVURLAsset *asset;
@property (nonatomic, strong) AVAssetTrack *audioTrack;
@property (nonatomic, strong) AVAssetReader *reader;
@property (nonatomic, strong) AVAssetReaderTrackOutput *output;
@property (nonatomic, assign) AvfAudioInfo info;
@end

@implementation AvfBridgeAudioDecoder
@end

static NSDictionary* avf_pcm_float32_settings(void) {
    return @{
        AVFormatIDKey: @(kAudioFormatLinearPCM),
        AVLinearPCMBitDepthKey: @(32),
        AVLinearPCMIsFloatKey: @YES,
        AVLinearPCMIsBigEndianKey: @NO,
        AVLinearPCMIsNonInterleaved: @NO
    };
}

void* avf_audio_decoder_create(const char* file_path, AvfAudioInfo* out_info, char* err_buf, size_t err_len) {
    @autoreleasepool {
        if (!file_path) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "Null file path");
            return NULL;
        }

        NSString *pathStr = [NSString stringWithUTF8String:file_path];
        if (!pathStr || ![[NSFileManager defaultManager] fileExistsAtPath:pathStr]) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "File does not exist: %s", file_path);
            return NULL;
        }

        NSURL *url = [NSURL fileURLWithPath:pathStr];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:url options:nil];
        NSArray<AVAssetTrack *> *tracks = [asset tracksWithMediaType:AVMediaTypeAudio];
        if (tracks.count == 0) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "No audio track found in media file: %s", file_path);
            return NULL;
        }

        AVAssetTrack *track = tracks.firstObject;
        CMTime duration = asset.duration;
        if (duration.timescale <= 0) {
            if (err_buf && err_len > 0) snprintf(err_buf, err_len, "Invalid container timescale (<= 0)");
            return NULL;
        }

        uint32_t sampleRate = (uint32_t)(track.naturalTimeScale > 0 ? track.naturalTimeScale : 48000);
        uint16_t channels = 2;

        NSArray *formatDescs = track.formatDescriptions;
        if (formatDescs.count > 0) {
            CMAudioFormatDescriptionRef fmt = (__bridge CMAudioFormatDescriptionRef)formatDescs.firstObject;
            const AudioStreamBasicDescription *asbd = CMAudioFormatDescriptionGetStreamBasicDescription(fmt);
            if (asbd != NULL) {
                if (asbd->mSampleRate > 0.0) {
                    sampleRate = (uint32_t)(asbd->mSampleRate + 0.5);
                }
                if (asbd->mChannelsPerFrame > 0) {
                    channels = (uint16_t)asbd->mChannelsPerFrame;
                }
            }
        }

        double durationSeconds = CMTimeGetSeconds(duration);
        if (isnan(durationSeconds) || durationSeconds < 0.0) {
            durationSeconds = 0.0;
        }

        AvfBridgeAudioDecoder *dec = [[AvfBridgeAudioDecoder alloc] init];
        dec.asset = asset;
        dec.audioTrack = track;
        dec.info = (AvfAudioInfo){
            .sample_rate = sampleRate,
            .channels = channels,
            .duration_seconds = durationSeconds,
            .duration_value = duration.value,
            .duration_timescale = duration.timescale,
        };

        if (out_info) {
            *out_info = dec.info;
        }

        NSError *error = nil;
        dec.reader = [AVAssetReader assetReaderWithAsset:dec.asset error:&error];
        if (!dec.reader || error) {
            if (err_buf && err_len > 0) {
                snprintf(err_buf, err_len, "AVAssetReader audio init failed: %s",
                         error.localizedDescription.UTF8String ?: "Unknown error");
            }
            return NULL;
        }

        dec.output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:dec.audioTrack
                                                               outputSettings:avf_pcm_float32_settings()];
        dec.output.alwaysCopiesSampleData = NO;
        [dec.reader addOutput:dec.output];

        if (![dec.reader startReading]) {
            if (err_buf && err_len > 0) {
                snprintf(err_buf, err_len, "AVAssetReader audio failed to start reading");
            }
            return NULL;
        }

        return (__bridge_retained void*)dec;
    }
}

int avf_audio_decoder_next_buffer(void* handle, AvfRawAudioBuffer* out_buf) {
    if (!handle || !out_buf) return -1;
    AvfBridgeAudioDecoder *dec = (__bridge AvfBridgeAudioDecoder*)handle;

    @autoreleasepool {
        CMSampleBufferRef sampleBuffer = [dec.output copyNextSampleBuffer];
        if (!sampleBuffer) {
            if (dec.reader.status == AVAssetReaderStatusFailed) {
                return -1;
            }
            return 0; // End of stream
        }

        CMTime pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
        CMItemCount numFrames = CMSampleBufferGetNumSamples(sampleBuffer);
        CMBlockBufferRef blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer);
        if (!blockBuffer || numFrames <= 0) {
            CFRelease(sampleBuffer);
            return 0;
        }

        size_t byteLen = CMBlockBufferGetDataLength(blockBuffer);
        size_t floatCount = byteLen / sizeof(float);
        if (floatCount == 0) {
            CFRelease(sampleBuffer);
            return 0;
        }

        float *copySamples = (float*)malloc(floatCount * sizeof(float));
        if (!copySamples) {
            CFRelease(sampleBuffer);
            return -1;
        }

        OSStatus status = CMBlockBufferCopyDataBytes(blockBuffer, 0, floatCount * sizeof(float), copySamples);
        CFRelease(sampleBuffer);
        if (status != kCMBlockBufferNoErr) {
            free(copySamples);
            return -1;
        }

        uint16_t channels = dec.info.channels > 0 ? dec.info.channels : 1;
        uint32_t frameCount = (uint32_t)(floatCount / channels);

        out_buf->sample_rate = dec.info.sample_rate;
        out_buf->channels = channels;
        out_buf->frame_count = frameCount;
        out_buf->pts_value = pts.value;
        out_buf->pts_timescale = pts.timescale;
        out_buf->samples = copySamples;
        out_buf->sample_count = floatCount;
        return 1;
    }
}

void avf_audio_decoder_free_buffer(AvfRawAudioBuffer* buf) {
    if (buf && buf->samples) {
        free(buf->samples);
        buf->samples = NULL;
        buf->sample_count = 0;
    }
}

int avf_audio_decoder_seek_pts(void* handle, int64_t target_value, int32_t target_timescale) {
    if (!handle) return -1;
    AvfBridgeAudioDecoder *dec = (__bridge AvfBridgeAudioDecoder*)handle;

    @autoreleasepool {
        [dec.reader cancelReading];

        CMTime seekTime = CMTimeMake(target_value, target_timescale);
        NSError *error = nil;
        dec.reader = [AVAssetReader assetReaderWithAsset:dec.asset error:&error];
        if (!dec.reader || error) return -1;

        dec.reader.timeRange = CMTimeRangeMake(seekTime, kCMTimePositiveInfinity);
        dec.output = [AVAssetReaderTrackOutput assetReaderTrackOutputWithTrack:dec.audioTrack
                                                               outputSettings:avf_pcm_float32_settings()];
        dec.output.alwaysCopiesSampleData = NO;
        [dec.reader addOutput:dec.output];

        if (![dec.reader startReading]) {
            return -1;
        }
        return 0;
    }
}

void avf_audio_decoder_destroy(void* handle) {
    if (!handle) return;
    @autoreleasepool {
        AvfBridgeAudioDecoder *dec = (__bridge_transfer AvfBridgeAudioDecoder*)handle;
        [dec.reader cancelReading];
        dec.reader = nil;
        dec.output = nil;
        dec.audioTrack = nil;
        dec.asset = nil;
    }
}

#pragma clang diagnostic pop
