import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import '../../bridge/api.dart';

class FrameRenderer extends StatefulWidget {
  final BridgeFrame? frame;
  final int textureId;

  const FrameRenderer({
    super.key,
    this.frame,
    this.textureId = 0,
  });

  @override
  State<FrameRenderer> createState() => _FrameRendererState();
}

class _FrameRendererState extends State<FrameRenderer> {
  ui.Image? _image;
  bool _isDecoding = false;
  BridgeFrame? _lastDecodedFrame;

  @override
  void initState() {
    super.initState();
    if (widget.frame != null) {
      _decodeFrame();
    }
  }

  @override
  void didUpdateWidget(FrameRenderer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.frame != oldWidget.frame) {
      _decodeFrame();
    }
  }

  Future<void> _decodeFrame() async {
    final frameToDecode = widget.frame;
    if (frameToDecode == null || _isDecoding) return;
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
            setState(() {
              _image?.dispose();
              _image = image;
              _lastDecodedFrame = frameToDecode;
            });
          } else {
            image.dispose();
          }
          _isDecoding = false;
          // Trigger next frame if missed
          if (widget.frame != frameToDecode) {
            _decodeFrame();
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
    _image?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textureWidget = Texture(
      key: const Key('preview_texture_widget'),
      textureId: widget.textureId,
      filterQuality: FilterQuality.medium,
    );

    if (_image == null) {
      return textureWidget;
    }
    return Stack(
      fit: StackFit.expand,
      children: [
        Opacity(opacity: 0.0, child: textureWidget),
        RawImage(
          image: _image,
          fit: BoxFit.contain,
          filterQuality: FilterQuality.low,
        ),
      ],
    );
  }
}
