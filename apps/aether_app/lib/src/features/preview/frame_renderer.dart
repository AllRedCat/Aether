import 'dart:ui' as ui;
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import '../../bridge/api.dart';

class FrameRenderer extends StatefulWidget {
  final BridgeFrame? frame;
  final ValueListenable<BridgeFrame?>? frameNotifier;
  final int textureId;

  const FrameRenderer({
    super.key,
    this.frame,
    this.frameNotifier,
    this.textureId = 0,
  });

  @override
  State<FrameRenderer> createState() => _FrameRendererState();
}

class _FrameRendererState extends State<FrameRenderer> {
  final ValueNotifier<ui.Image?> _imageNotifier = ValueNotifier<ui.Image?>(null);
  bool _isDecoding = false;
  BridgeFrame? _lastDecodedFrame;

  @override
  void initState() {
    super.initState();
    widget.frameNotifier?.addListener(_onFrameNotifierChanged);
    final initialFrame = widget.frameNotifier?.value ?? widget.frame;
    if (initialFrame != null) {
      _decodeFrame(initialFrame);
    }
  }

  void _onFrameNotifierChanged() {
    final frame = widget.frameNotifier?.value;
    if (frame != null && mounted) {
      _decodeFrame(frame);
    }
  }

  @override
  void didUpdateWidget(FrameRenderer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.frameNotifier != oldWidget.frameNotifier) {
      oldWidget.frameNotifier?.removeListener(_onFrameNotifierChanged);
      widget.frameNotifier?.addListener(_onFrameNotifierChanged);
    }
    if (widget.frame != oldWidget.frame && widget.frame != null) {
      _decodeFrame(widget.frame!);
    }
  }

  Future<void> _decodeFrame(BridgeFrame frameToDecode) async {
    if (_isDecoding) return;
    if (_lastDecodedFrame == frameToDecode) return;

    final width = frameToDecode.width;
    final height = frameToDecode.height;
    final stride = frameToDecode.rowStrideBytes >= width * 4
        ? frameToDecode.rowStrideBytes
        : width * 4;
    final minRequiredBytes =
        height > 0 && width > 0 ? (height - 1) * stride + width * 4 : 0;
    if (width <= 0 ||
        height <= 0 ||
        frameToDecode.rgbaBytes.length < minRequiredBytes) {
      _lastDecodedFrame = frameToDecode;
      return;
    }

    _isDecoding = true;
    try {
      ui.decodeImageFromPixels(
        frameToDecode.rgbaBytes,
        width,
        height,
        ui.PixelFormat.rgba8888,
        (image) {
          if (mounted) {
            final oldImage = _imageNotifier.value;
            _imageNotifier.value = image;
            oldImage?.dispose();
            _lastDecodedFrame = frameToDecode;
          } else {
            image.dispose();
          }
          _isDecoding = false;
          // Trigger next frame if newer target frame arrived during decode
          final currentTarget = widget.frameNotifier?.value ?? widget.frame;
          if (currentTarget != null && currentTarget != frameToDecode) {
            _decodeFrame(currentTarget);
          }
        },
        rowBytes: stride,
      );
    } catch (e) {
      _isDecoding = false;
    }
  }

  @override
  void dispose() {
    widget.frameNotifier?.removeListener(_onFrameNotifierChanged);
    final oldImage = _imageNotifier.value;
    _imageNotifier.value = null;
    oldImage?.dispose();
    _imageNotifier.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textureWidget = Texture(
      key: const Key('preview_texture_widget'),
      textureId: widget.textureId,
      filterQuality: FilterQuality.medium,
    );

    return ValueListenableBuilder<ui.Image?>(
      valueListenable: _imageNotifier,
      builder: (context, image, _) {
        if (image == null) {
          return textureWidget;
        }
        return Stack(
          fit: StackFit.expand,
          children: [
            Opacity(opacity: 0.0, child: textureWidget),
            RawImage(
              image: image,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.low,
            ),
          ],
        );
      },
    );
  }
}
