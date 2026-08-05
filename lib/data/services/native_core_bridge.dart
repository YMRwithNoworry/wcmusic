import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _VersionNative = Pointer<Utf8> Function();
typedef _VersionDart = Pointer<Utf8> Function();
typedef _ValidateNative = Pointer<Utf8> Function(Pointer<Uint8>, IntPtr, Bool);
typedef _ValidateDart = Pointer<Utf8> Function(Pointer<Uint8>, int, bool);
typedef _ResolveNative =
    Pointer<Utf8> Function(
      Pointer<Uint8>,
      IntPtr,
      Pointer<Uint8>,
      IntPtr,
      Pointer<Uint8>,
      IntPtr,
      Pointer<Uint8>,
      IntPtr,
      Bool,
    );
typedef _ResolveDart =
    Pointer<Utf8> Function(
      Pointer<Uint8>,
      int,
      Pointer<Uint8>,
      int,
      Pointer<Uint8>,
      int,
      Pointer<Uint8>,
      int,
      bool,
    );
typedef _FreeNative = Void Function(Pointer<Utf8>);
typedef _FreeDart = void Function(Pointer<Utf8>);

class NativeCoreBridge {
  NativeCoreBridge() {
    try {
      final libraryName = Platform.isWindows
          ? 'wcmusic_core.dll'
          : 'libwcmusic_core.so';
      _library = DynamicLibrary.open(libraryName);
      _version = _library!.lookupFunction<_VersionNative, _VersionDart>(
        'wcmusic_core_version',
      );
      _validate = _library!.lookupFunction<_ValidateNative, _ValidateDart>(
        'wcmusic_validate_source',
      );
      _resolve = _library!.lookupFunction<_ResolveNative, _ResolveDart>(
        'wcmusic_resolve_source_url',
      );
      _free = _library!.lookupFunction<_FreeNative, _FreeDart>(
        'wcmusic_string_free',
      );
    } catch (_) {
      _library = null;
      _version = null;
      _validate = null;
      _resolve = null;
      _free = null;
    }
  }

  DynamicLibrary? _library;
  _VersionDart? _version;
  _ValidateDart? _validate;
  _ResolveDart? _resolve;
  _FreeDart? _free;

  bool get isLoaded => _library != null;
  String get version => isLoaded ? _version!().toDartString() : 'dart-fallback';

  Map<String, dynamic>? validateSource(String script) {
    if (!isLoaded) return null;
    final encoded = utf8.encode(script);
    final input = calloc<Uint8>(encoded.length);
    input.asTypedList(encoded.length).setAll(0, encoded);
    final output = _validate!(input, encoded.length, Platform.isAndroid);
    calloc.free(input);
    try {
      return jsonDecode(output.toDartString()) as Map<String, dynamic>;
    } finally {
      _free!(output);
    }
  }

  String resolveSourceUrl({
    required String script,
    required String source,
    required String songId,
    required String quality,
  }) {
    if (!isLoaded || _resolve == null) {
      throw UnsupportedError('Rust 音源运行时未加载');
    }
    final values = [script, source, songId, quality];
    final encoded = values.map(utf8.encode).toList(growable: false);
    final pointers = encoded
        .map((bytes) => calloc<Uint8>(bytes.length))
        .toList(growable: false);
    for (var index = 0; index < encoded.length; index++) {
      pointers[index]
          .asTypedList(encoded[index].length)
          .setAll(0, encoded[index]);
    }
    Pointer<Utf8> output;
    try {
      output = _resolve!(
        pointers[0],
        encoded[0].length,
        pointers[1],
        encoded[1].length,
        pointers[2],
        encoded[2].length,
        pointers[3],
        encoded[3].length,
        Platform.isAndroid,
      );
    } finally {
      for (final pointer in pointers) {
        calloc.free(pointer);
      }
    }
    if (output == nullptr) throw StateError('音源运行时没有返回结果');
    try {
      final result = jsonDecode(output.toDartString());
      if (result is! Map || result['ok'] != true) {
        final error = result is Map ? result['error'] : null;
        throw StateError(error?.toString() ?? '音源没有返回播放地址');
      }
      final data = result['data'];
      final url = data is Map ? data['url']?.toString() : null;
      if (url == null || url.isEmpty) throw StateError('音源播放地址为空');
      return url;
    } finally {
      _free!(output);
    }
  }
}
