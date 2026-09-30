import 'dart:ui' as ui;
import 'dart:typed_data';
import 'package:flutter/material.dart';
import '../../bridge/api.dart';

class FrameRenderer extends StatefulWidget {
  final BridgeFrame? frame;
  const FrameRenderer({super.key, this.frame});

  @override
  State<FrameRenderer> createState() => _FrameRendererState();
}

class _FrameRendererState extends State<FrameRenderer> {
  ui.Image? _image;
  bool _isDecoding = false;
  BridgeFrame? _lastDecodedFrame;

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

    _isDecoding = true;
    try {
      ui.decodeImageFromPixels(
        frameToDecode.rgbaBytes,
        frameToDecode.width,
        frameToDecode.height,
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
        rowBytes: frameToDecode.rowStrideBytes,
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
    if (_image == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return RawImage(
      image: _image,
      fit: BoxFit.contain,
      filterQuality: FilterQuality.low,
    );
  }
}
