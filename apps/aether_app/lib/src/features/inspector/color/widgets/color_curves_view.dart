import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../../../../theme/catppuccin.dart';
import '../color_grading_provider.dart';
import 'color_curves_widget.dart';

/// Complete Tone Curves View featuring multi-channel selection (RGB Master, Red, Green, Blue),
/// interactive spline canvas with point editing and deletion, point coordinates readout,
/// individual channel reset, and reset all curves action.
///
/// Supports both direct Riverpod state integration via [clipId] and standalone
/// parameter overrides for decoupled unit/widget testing.
class ColorCurvesView extends ConsumerStatefulWidget {
  final UuidValue? clipId;

  // Standalone properties (optional for isolated unit and widget tests)
  final CurveChannel? activeChannel;
  final List<Offset>? points;
  final Map<CurveChannel, List<Offset>>? allCurves;
  final ValueChanged<CurveChannel>? onChannelChanged;
  final ValueChanged<List<Offset>>? onPointsChanged;
  final VoidCallback? onResetChannel;
  final VoidCallback? onResetAllCurves;

  const ColorCurvesView({
    super.key,
    this.clipId,
    this.activeChannel,
    this.points,
    this.allCurves,
    this.onChannelChanged,
    this.onPointsChanged,
    this.onResetChannel,
    this.onResetAllCurves,
  });

  @override
  ConsumerState<ColorCurvesView> createState() => _ColorCurvesViewState();
}

class _ColorCurvesViewState extends ConsumerState<ColorCurvesView> {
  int? _selectedPointIndex;

  @override
  Widget build(BuildContext context) {
    CurveChannel currentChannel = widget.activeChannel ?? CurveChannel.rgb;
    List<Offset> currentPoints = widget.points ?? ColorGradingModel.defaultCurvePoints;
    Map<CurveChannel, List<Offset>> currentAllCurves = widget.allCurves ?? {
      CurveChannel.rgb: ColorGradingModel.defaultCurvePoints,
      CurveChannel.red: ColorGradingModel.defaultCurvePoints,
      CurveChannel.green: ColorGradingModel.defaultCurvePoints,
      CurveChannel.blue: ColorGradingModel.defaultCurvePoints,
    };

    ColorGradingNotifier? notifier;
    if (widget.clipId != null) {
      try {
        final gradingState = ref.watch(colorGradingProvider);
        final clipGrading = gradingState.gradingFor(widget.clipId!);
        currentChannel = widget.activeChannel ?? clipGrading.activeCurveChannel;
        currentPoints = widget.points ?? clipGrading.pointsForChannel(currentChannel);
        currentAllCurves = widget.allCurves ?? {
          CurveChannel.rgb: clipGrading.rgbCurvePoints,
          CurveChannel.red: clipGrading.redCurvePoints,
          CurveChannel.green: clipGrading.greenCurvePoints,
          CurveChannel.blue: clipGrading.blueCurvePoints,
        };
        notifier = ref.read(colorGradingProvider.notifier);
      } catch (_) {
        // Fallback gracefully in mock test environments
      }
    }

    final bool isInteriorPointSelected = _selectedPointIndex != null &&
        _selectedPointIndex! > 0 &&
        _selectedPointIndex! < currentPoints.length - 1;

    return Container(
      key: const Key('color_curves_view'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Section Header: Title & Reset All Curves Action
          Row(
            children: [
              Container(
                width: 3,
                height: 12,
                decoration: BoxDecoration(
                  color: CatppuccinMocha.blue,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'CURVAS DE TOM',
                  style: TextStyle(
                    color: CatppuccinMocha.blue,
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                    letterSpacing: 0.5,
                  ),
                ),
              ),
              InkWell(
                key: const Key('reset_all_curves'),
                onTap: () {
                  if (widget.onResetAllCurves != null) {
                    widget.onResetAllCurves!();
                  } else if (widget.clipId != null && notifier != null) {
                    notifier.resetAllCurves(widget.clipId!);
                  }
                  setState(() => _selectedPointIndex = null);
                },
                borderRadius: BorderRadius.circular(4),
                child: const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  child: Row(
                    children: [
                      Icon(Icons.restart_alt_rounded, size: 13, color: CatppuccinMocha.overlay0),
                      SizedBox(width: 4),
                      Text(
                        'Reset Curves',
                        style: TextStyle(color: CatppuccinMocha.overlay0, fontSize: 11),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),

          // Channel Selector Segmented Row (RGB, Red, Green, Blue)
          Container(
            padding: const EdgeInsets.all(3),
            decoration: BoxDecoration(
              color: CatppuccinMocha.surface0,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
            ),
            child: Row(
              children: CurveChannel.values.map((channel) {
                final isSelected = channel == currentChannel;

                return Expanded(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 2.0),
                    child: InkWell(
                      key: Key('curve_channel_${channel.name}'),
                      onTap: () {
                        if (widget.onChannelChanged != null) {
                          widget.onChannelChanged!(channel);
                        } else if (widget.clipId != null && notifier != null) {
                          notifier.setActiveCurveChannel(widget.clipId!, channel);
                        }
                        setState(() => _selectedPointIndex = null);
                      },
                      borderRadius: BorderRadius.circular(4),
                      child: Container(
                        padding: const EdgeInsets.symmetric(vertical: 5),
                        alignment: Alignment.center,
                        decoration: BoxDecoration(
                          color: isSelected
                              ? channel.color.withValues(alpha: 0.22)
                              : Colors.transparent,
                          borderRadius: BorderRadius.circular(4),
                          border: isSelected
                              ? Border.all(color: channel.color, width: 1.0)
                              : null,
                        ),
                        child: FittedBox(
                          fit: BoxFit.scaleDown,
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Container(
                                width: 6,
                                height: 6,
                                decoration: BoxDecoration(
                                  color: channel.color,
                                  shape: BoxShape.circle,
                                ),
                              ),
                              const SizedBox(width: 4),
                              Text(
                                channel.label,
                                style: TextStyle(
                                  color: isSelected ? channel.color : CatppuccinMocha.subtext0,
                                  fontWeight: isSelected ? FontWeight.bold : FontWeight.w500,
                                  fontSize: 11,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
          const SizedBox(height: 10),

          // Interactive Spline Canvas
          ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: ColorCurvesWidget(
              key: const Key('color_curves_widget'),
              activeChannel: currentChannel,
              points: currentPoints,
              allCurves: currentAllCurves,
              onSelectedPointChanged: (idx) {
                setState(() => _selectedPointIndex = idx);
              },
              onPointsChanged: (newPoints) {
                if (widget.onPointsChanged != null) {
                  widget.onPointsChanged!(newPoints);
                } else if (widget.clipId != null && notifier != null) {
                  notifier.setCurvePoints(widget.clipId!, currentChannel, newPoints);
                }
              },
              onReset: () {
                if (widget.onResetChannel != null) {
                  widget.onResetChannel!();
                } else if (widget.clipId != null && notifier != null) {
                  notifier.resetCurveChannel(widget.clipId!, currentChannel);
                }
                setState(() => _selectedPointIndex = null);
              },
            ),
          ),
          const SizedBox(height: 8),

          // Footer Readout & Point Controls
          Row(
            children: [
              // Point In/Out Readout or General Guide
              Expanded(
                child: _buildPointInfoReadout(currentPoints),
              ),
              const SizedBox(width: 8),

              // Delete Selected Point Action (active when interior point selected)
              if (isInteriorPointSelected) ...[
                InkWell(
                  key: const Key('curve_delete_point'),
                  onTap: () {
                    final idx = _selectedPointIndex!;
                    if (widget.clipId != null && notifier != null) {
                      notifier.removeCurvePoint(widget.clipId!, currentChannel, idx);
                    } else if (widget.onPointsChanged != null) {
                      final updated = List<Offset>.from(currentPoints)..removeAt(idx);
                      widget.onPointsChanged!(List.unmodifiable(updated));
                    }
                    setState(() => _selectedPointIndex = null);
                  },
                  borderRadius: BorderRadius.circular(4),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
                    decoration: BoxDecoration(
                      color: CatppuccinMocha.surface0,
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(color: CatppuccinMocha.red.withValues(alpha: 0.5), width: 0.5),
                    ),
                    child: const Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.remove_circle_outline, size: 12, color: CatppuccinMocha.red),
                        SizedBox(width: 4),
                        Text(
                          'Del',
                          style: TextStyle(color: CatppuccinMocha.red, fontSize: 10, fontWeight: FontWeight.bold),
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(width: 6),
              ],

              // Reset Current Channel Action
              InkWell(
                key: const Key('reset_curve_channel'),
                onTap: () {
                  if (widget.onResetChannel != null) {
                    widget.onResetChannel!();
                  } else if (widget.clipId != null && notifier != null) {
                    notifier.resetCurveChannel(widget.clipId!, currentChannel);
                  }
                  setState(() => _selectedPointIndex = null);
                },
                borderRadius: BorderRadius.circular(4),
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
                  decoration: BoxDecoration(
                    color: CatppuccinMocha.surface0,
                    borderRadius: BorderRadius.circular(4),
                    border: Border.all(color: CatppuccinMocha.surface1, width: 0.5),
                  ),
                  child: const Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.restart_alt_rounded, size: 12, color: CatppuccinMocha.overlay0),
                      SizedBox(width: 4),
                      Text(
                        'Reset Canal',
                        style: TextStyle(color: CatppuccinMocha.overlay0, fontSize: 10),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildPointInfoReadout(List<Offset> points) {
    if (_selectedPointIndex != null &&
        _selectedPointIndex! >= 0 &&
        _selectedPointIndex! < points.length) {
      final p = points[_selectedPointIndex!];
      final inPct = (p.dx * 100).toStringAsFixed(0);
      final outPct = (p.dy * 100).toStringAsFixed(0);

      return Container(
        key: const Key('curve_point_info'),
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
        decoration: BoxDecoration(
          color: CatppuccinMocha.surface0,
          borderRadius: BorderRadius.circular(4),
        ),
        child: Text(
          'In: $inPct%  Out: $outPct%',
          style: const TextStyle(
            color: CatppuccinMocha.text,
            fontFamily: 'monospace',
            fontSize: 10,
            fontWeight: FontWeight.w600,
          ),
        ),
      );
    }

    return Container(
      key: const Key('curve_point_info'),
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
      child: Text(
        'Pontos: ${points.length}  (Toque para adicionar)',
        style: const TextStyle(
          color: CatppuccinMocha.subtext0,
          fontSize: 10,
        ),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}
