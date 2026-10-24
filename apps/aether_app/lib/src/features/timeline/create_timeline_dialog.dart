import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../theme/catppuccin.dart';
import 'active_timeline_provider.dart';

enum ResolutionPreset {
  fhd(1920, 1080, '1080p Full HD (1920x1080)'),
  uhd(3840, 2160, '4K UHD (3840x2160)'),
  vertical(1080, 1920, '1080x1920 Vertical (Reels/TikTok)');

  final int width;
  final int height;
  final String label;

  const ResolutionPreset(this.width, this.height, this.label);
}

/// Dialog for creating a new custom timeline.
class CreateTimelineDialog extends ConsumerStatefulWidget {
  const CreateTimelineDialog({super.key});

  /// Static helper to display the dialog.
  static Future<void> show(BuildContext context) {
    return showDialog<void>(
      context: context,
      barrierDismissible: true,
      builder: (context) => const CreateTimelineDialog(),
    );
  }

  @override
  ConsumerState<CreateTimelineDialog> createState() => _CreateTimelineDialogState();
}

class _CreateTimelineDialogState extends ConsumerState<CreateTimelineDialog> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;

  ResolutionPreset _selectedResolution = ResolutionPreset.fhd;
  int _selectedFps = 60;
  bool _isSubmitting = false;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: 'Timeline 1');
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  Future<void> _handleSubmit() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() {
      _isSubmitting = true;
      _errorMessage = null;
    });

    try {
      final name = _nameController.text.trim();
      await ref.read(activeTimelineProvider.notifier).createCustomTimeline(
            name: name,
            width: _selectedResolution.width,
            height: _selectedResolution.height,
            fps: _selectedFps,
          );

      if (mounted) {
        Navigator.of(context).pop();
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _isSubmitting = false;
          _errorMessage = e.toString();
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      key: const Key('create_timeline_dialog'),
      backgroundColor: CatppuccinMocha.mantle,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: CatppuccinMocha.surface1),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Form(
            key: _formKey,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                // Header
                Row(
                  children: [
                    Container(
                      padding: const EdgeInsets.all(8),
                      decoration: BoxDecoration(
                        color: CatppuccinMocha.mauve.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: const Icon(
                        Icons.view_timeline_rounded,
                        color: CatppuccinMocha.mauve,
                        size: 22,
                      ),
                    ),
                    const SizedBox(width: 12),
                    const Expanded(
                      child: Text(
                        'Nova Timeline',
                        style: TextStyle(
                          color: CatppuccinMocha.text,
                          fontSize: 18,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ),
                    IconButton(
                      icon: const Icon(Icons.close, size: 20, color: CatppuccinMocha.subtext0),
                      onPressed: () => Navigator.of(context).pop(),
                      splashRadius: 18,
                    ),
                  ],
                ),
                const SizedBox(height: 20),

                // Error message
                if (_errorMessage != null) ...[
                  Container(
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(
                      color: CatppuccinMocha.red.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(6),
                    ),
                    child: Text(
                      _errorMessage!,
                      style: const TextStyle(color: CatppuccinMocha.red, fontSize: 13),
                    ),
                  ),
                  const SizedBox(height: 16),
                ],

                // Timeline Name Field
                const Text(
                  'Nome da Timeline',
                  style: TextStyle(
                    color: CatppuccinMocha.subtext1,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 6),
                TextFormField(
                  key: const Key('timeline_name_field'),
                  controller: _nameController,
                  style: const TextStyle(color: CatppuccinMocha.text, fontSize: 14),
                  decoration: InputDecoration(
                    hintText: 'Ex: Timeline 1',
                    hintStyle: const TextStyle(color: CatppuccinMocha.surface2),
                    filled: true,
                    fillColor: CatppuccinMocha.base,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.mauve, width: 1.5),
                    ),
                  ),
                  validator: (val) {
                    if (val == null || val.trim().isEmpty) {
                      return 'Por favor, insira o nome da timeline';
                    }
                    return null;
                  },
                ),
                const SizedBox(height: 16),

                // Resolution Preset Dropdown
                const Text(
                  'Resolução',
                  style: TextStyle(
                    color: CatppuccinMocha.subtext1,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 6),
                DropdownButtonFormField<ResolutionPreset>(
                  key: const Key('resolution_preset_dropdown'),
                  initialValue: _selectedResolution,
                  dropdownColor: CatppuccinMocha.base,
                  style: const TextStyle(color: CatppuccinMocha.text, fontSize: 14),
                  decoration: InputDecoration(
                    filled: true,
                    fillColor: CatppuccinMocha.base,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.mauve, width: 1.5),
                    ),
                  ),
                  items: ResolutionPreset.values.map((preset) {
                    return DropdownMenuItem(
                      value: preset,
                      child: Text(preset.label),
                    );
                  }).toList(),
                  onChanged: (val) {
                    if (val != null) {
                      setState(() => _selectedResolution = val);
                    }
                  },
                ),
                const SizedBox(height: 16),

                // Frame Rate (FPS) Dropdown
                const Text(
                  'Taxa de Quadros (FPS)',
                  style: TextStyle(
                    color: CatppuccinMocha.subtext1,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 6),
                DropdownButtonFormField<int>(
                  key: const Key('fps_preset_dropdown'),
                  initialValue: _selectedFps,
                  dropdownColor: CatppuccinMocha.base,
                  style: const TextStyle(color: CatppuccinMocha.text, fontSize: 14),
                  decoration: InputDecoration(
                    filled: true,
                    fillColor: CatppuccinMocha.base,
                    contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.surface1),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: CatppuccinMocha.mauve, width: 1.5),
                    ),
                  ),
                  items: const [
                    DropdownMenuItem(value: 24, child: Text('24 FPS (Cinemático)')),
                    DropdownMenuItem(value: 30, child: Text('30 FPS (Padrão)')),
                    DropdownMenuItem(value: 60, child: Text('60 FPS (Fluido / Web)')),
                  ],
                  onChanged: (val) {
                    if (val != null) {
                      setState(() => _selectedFps = val);
                    }
                  },
                ),
                const SizedBox(height: 24),

                // Actions
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    TextButton(
                      key: const Key('cancel_create_timeline_button'),
                      onPressed: _isSubmitting ? null : () => Navigator.of(context).pop(),
                      style: TextButton.styleFrom(
                        foregroundColor: CatppuccinMocha.subtext0,
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                      ),
                      child: const Text('Cancelar'),
                    ),
                    const SizedBox(width: 8),
                    ElevatedButton(
                      key: const Key('confirm_create_timeline_button'),
                      onPressed: _isSubmitting ? null : _handleSubmit,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: CatppuccinMocha.mauve,
                        foregroundColor: CatppuccinMocha.base,
                        elevation: 0,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(8),
                        ),
                        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
                      ),
                      child: _isSubmitting
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: CatppuccinMocha.base,
                              ),
                            )
                          : const Text(
                              'Criar Timeline',
                              style: TextStyle(fontWeight: FontWeight.bold),
                            ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
