import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/playback_mode.dart';
import 'player_view_model.dart';

class PlaybackModeButton extends StatelessWidget {
  const PlaybackModeButton({super.key, this.iconSize});

  final double? iconSize;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return IconButton(
      iconSize: iconSize,
      onPressed: viewModel.cyclePlaybackMode,
      icon: Icon(_iconFor(viewModel.playbackMode)),
      tooltip: '${viewModel.playbackMode.label}（点击切换）',
    );
  }

  IconData _iconFor(PlaybackMode mode) => switch (mode) {
    PlaybackMode.sequence => Icons.playlist_play,
    PlaybackMode.listLoop => Icons.repeat,
    PlaybackMode.shuffle => Icons.shuffle,
    PlaybackMode.singleLoop => Icons.repeat_one,
  };
}
