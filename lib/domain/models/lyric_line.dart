class LyricLine {
  const LyricLine({required this.time, required this.text, this.translation});

  final Duration time;
  final String text;

  /// 中文翻译歌词，没有时为空
  final String? translation;

  LyricLine copyWith({String? translation}) => LyricLine(
    time: time,
    text: text,
    translation: translation ?? this.translation,
  );
}
