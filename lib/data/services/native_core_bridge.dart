import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _VersionNative = Pointer<Utf8> Function();
typedef _VersionDart = Pointer<Utf8> Function();
typedef _ValidateNative = Pointer<Utf8> Function(Pointer<Uint8>, IntPtr, Bool);
typedef _ValidateDart = Pointer<Utf8> Function(Pointer<Uint8>, int, bool);
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
      _free = _library!.lookupFunction<_FreeNative, _FreeDart>(
        'wcmusic_string_free',
      );
    } catch (_) {
      _library = null;
      _version = null;
      _validate = null;
      _free = null;
    }
  }

  DynamicLibrary? _library;
  _VersionDart? _version;
  _ValidateDart? _validate;
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
}
