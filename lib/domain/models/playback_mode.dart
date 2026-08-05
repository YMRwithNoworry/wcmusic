enum PlaybackMode { sequence, listLoop, shuffle, singleLoop }

extension PlaybackModeLabel on PlaybackMode {
  String get label => switch (this) {
    PlaybackMode.sequence => '顺序播放',
    PlaybackMode.listLoop => '列表循环',
    PlaybackMode.shuffle => '随机循环',
    PlaybackMode.singleLoop => '单曲循环',
  };
}
