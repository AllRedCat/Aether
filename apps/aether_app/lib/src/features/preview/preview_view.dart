import 'package:flutter/material.dart';
import '../../theme/catppuccin.dart';

class PreviewView extends StatefulWidget {
  final Widget? customViewport;

  const PreviewView({
    super.key,
    this.customViewport,
  });

  @override
  State<PreviewView> createState() => _PreviewViewState();
}

class _PreviewViewState extends State<PreviewView> {
  bool _isPlaying = false;

  void _togglePlayPause() {
    setState(() {
      _isPlaying = !_isPlaying;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      key: const Key('preview_view'),
      color: CatppuccinMocha.base,
      padding: const EdgeInsets.all(16.0),
      child: Column(
        children: [
          Expanded(
            child: widget.customViewport ??
                Container(
                  key: const Key('preview_gpu_viewport'),
                  decoration: BoxDecoration(
                    color: Colors.black,
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: CatppuccinMocha.surface0),
                  ),
                  child: const Center(
                    child: SingleChildScrollView(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            Icons.tv_rounded,
                            size: 32,
                            color: CatppuccinMocha.surface2,
                          ),
                          SizedBox(height: 8),
                          Text(
                            'Frame Visualizado da GPU',
                            style: TextStyle(
                              color: CatppuccinMocha.surface2,
                              fontSize: 12,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
          ),
          const SizedBox(height: 12),
          Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              IconButton(
                key: const Key('preview_prev_button'),
                tooltip: 'Voltar Frame',
                onPressed: () {},
                icon: const Icon(
                  Icons.skip_previous_rounded,
                  color: CatppuccinMocha.text,
                  size: 20,
                ),
              ),
              const SizedBox(width: 8),
              Container(
                decoration: const BoxDecoration(
                  color: CatppuccinMocha.surface0,
                  shape: BoxShape.circle,
                ),
                child: IconButton(
                  key: const Key('preview_play_button'),
                  tooltip: _isPlaying ? 'Pausar' : 'Reproduzir',
                  onPressed: _togglePlayPause,
                  icon: Icon(
                    _isPlaying ? Icons.pause_rounded : Icons.play_arrow_rounded,
                    color: CatppuccinMocha.text,
                    size: 24,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                key: const Key('preview_next_button'),
                tooltip: 'Avançar Frame',
                onPressed: () {},
                icon: const Icon(
                  Icons.skip_next_rounded,
                  color: CatppuccinMocha.text,
                  size: 20,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
