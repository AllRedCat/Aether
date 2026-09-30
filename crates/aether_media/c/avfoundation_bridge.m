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
@end

@implementation AvfBridgeDecoder
@end

void* avf_decoder_create(const char* file_path, AvfMediaInfo* out_info, char* err_buf, size_t err_len) {
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
        double durationSeconds = CMTimeGetSeconds(duration);
        if (isnan(durationSeconds) || durationSeconds < 0.0) {
            durationSeconds = 0.0;
        }

        dec.info = (AvfMediaInfo){
            .width = (uint32_t)naturalSize.width,
            .height = (uint32_t)naturalSize.height,
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

        NSDictionary *settings = @{
            (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)
        };
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
        out_frame->data = copyData;
        out_frame->data_len = totalBytes;

        return 1; // Frame decoded successfully
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

        NSDictionary *settings = @{
            (id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)
        };
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

#pragma clang diagnostic pop
