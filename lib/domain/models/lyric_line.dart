class LyricLine {
  const LyricLine({
    required this.time,
    required this.text,
    this.translation,
  });

  final Duration time;
  final String text;
  final String? translation;
}
