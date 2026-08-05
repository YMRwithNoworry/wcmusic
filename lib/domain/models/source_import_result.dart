class SourceImportResult {
  const SourceImportResult({required this.imported, required this.failures});

  final int imported;
  final List<String> failures;
}
