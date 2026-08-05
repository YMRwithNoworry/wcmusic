enum PlaybackQuality { standard, high, lossless }

extension PlaybackQualityInfo on PlaybackQuality {
  String get sourceValue => switch (this) {
    PlaybackQuality.standard => '128k',
    PlaybackQuality.high => '320k',
    PlaybackQuality.lossless => 'flac',
  };

  String get label => switch (this) {
    PlaybackQuality.standard => '标准 128k',
    PlaybackQuality.high => '高品 320k',
    PlaybackQuality.lossless => '无损 flac',
  };
}
