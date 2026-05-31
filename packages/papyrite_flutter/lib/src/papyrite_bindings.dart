import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

final class _PapyriteBuffer extends Struct {
  external Pointer<Uint8> ptr;

  @UintPtr()
  external int len;
}

final class _PapyriteResult extends Struct {
  @Int32()
  external int code;

  external _PapyriteBuffer data;
  external _PapyriteBuffer error;

  @Uint8()
  external int boolValue;
}

typedef _JsonOpNative = Int32 Function(
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
  Pointer<_PapyriteResult>,
);

typedef _JsonOpDart = int Function(
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
  int,
  Pointer<_PapyriteResult>,
);

typedef _DumpNative = Int32 Function(
    Pointer<Uint8>, UintPtr, Pointer<_PapyriteResult>);

typedef _DumpDart = int Function(Pointer<Uint8>, int, Pointer<_PapyriteResult>);

typedef _ResultFreeNative = Void Function(Pointer<_PapyriteResult>);
typedef _ResultFreeDart = void Function(Pointer<_PapyriteResult>);

class PapyriteStatus {
  static const ok = 0;
  static const invalidArgument = 1;
  static const engineError = 2;
  static const panic = 3;

  const PapyriteStatus._();
}

class PapyriteException implements Exception {
  PapyriteException(this.code, this.message);

  final int code;
  final String message;

  @override
  String toString() => 'PapyriteException($code): $message';
}

class PapyriteBindings {
  PapyriteBindings._(DynamicLibrary library)
      : _createJson = library.lookupFunction<_JsonOpNative, _JsonOpDart>(
          'papyrite_create_json',
        ),
        _getJson = library.lookupFunction<_JsonOpNative, _JsonOpDart>(
          'papyrite_get_json',
        ),
        _deleteJson = library.lookupFunction<_JsonOpNative, _JsonOpDart>(
          'papyrite_delete_json',
        ),
        _updateJson = library.lookupFunction<_JsonOpNative, _JsonOpDart>(
          'papyrite_update_json',
        ),
        _findJson = library.lookupFunction<_JsonOpNative, _JsonOpDart>(
          'papyrite_find_json',
        ),
        _dumpJson = library.lookupFunction<_DumpNative, _DumpDart>(
          'papyrite_dump_json',
        ),
        _resultFree =
            library.lookupFunction<_ResultFreeNative, _ResultFreeDart>(
          'papyrite_result_free',
        );

  factory PapyriteBindings.open({String? libraryPath}) {
    final library = libraryPath == null
        ? _openDefaultLibrary()
        : DynamicLibrary.open(libraryPath);
    return PapyriteBindings._(library);
  }

  final _JsonOpDart _createJson;
  final _JsonOpDart _getJson;
  final _JsonOpDart _deleteJson;
  final _JsonOpDart _updateJson;
  final _JsonOpDart _findJson;
  final _DumpDart _dumpJson;
  final _ResultFreeDart _resultFree;

  void createJson(String dbPath, String documentJson) {
    _callJson(_createJson, dbPath, documentJson);
  }

  String? getJson(String dbPath, String filterJson) {
    final result = _callJson(_getJson, dbPath, filterJson);
    if (!result.boolValue) {
      return null;
    }
    return utf8.decode(result.data);
  }

  bool deleteJson(String dbPath, String filterJson) {
    return _callJson(_deleteJson, dbPath, filterJson).boolValue;
  }

  void updateJson(String dbPath, String updateJson) {
    _callJson(_updateJson, dbPath, updateJson);
  }

  String findJson(String dbPath, String queryJson) {
    return utf8.decode(_callJson(_findJson, dbPath, queryJson).data);
  }

  String dumpJson(String dbPath) {
    return utf8.decode(_callDump(dbPath).data);
  }

  _OwnedResult _callJson(_JsonOpDart function, String dbPath, String json) {
    return _withUtf8Bytes(dbPath, (pathPtr, pathLen) {
      return _withUtf8Bytes(json, (jsonPtr, jsonLen) {
        return _withResult((resultPtr) {
          function(pathPtr, pathLen, jsonPtr, jsonLen, resultPtr);
        });
      });
    });
  }

  _OwnedResult _callDump(String dbPath) {
    return _withUtf8Bytes(dbPath, (pathPtr, pathLen) {
      return _withResult((resultPtr) {
        _dumpJson(pathPtr, pathLen, resultPtr);
      });
    });
  }

  T _withUtf8Bytes<T>(String value, T Function(Pointer<Uint8>, int) callback) {
    final bytes = utf8.encode(value);
    if (bytes.isEmpty) {
      return callback(nullptr, 0);
    }

    final ptr = calloc<Uint8>(bytes.length);
    try {
      ptr.asTypedList(bytes.length).setAll(0, bytes);
      return callback(ptr, bytes.length);
    } finally {
      calloc.free(ptr);
    }
  }

  _OwnedResult _withResult(void Function(Pointer<_PapyriteResult>) callback) {
    final resultPtr = calloc<_PapyriteResult>();
    try {
      callback(resultPtr);
      final result = resultPtr.ref;
      final data = _copyBuffer(result.data);
      final error = utf8.decode(
        _copyBuffer(result.error),
        allowMalformed: true,
      );
      final owned = _OwnedResult(
        code: result.code,
        data: data,
        error: error,
        boolValue: result.boolValue != 0,
      );

      _resultFree(resultPtr);

      if (owned.code != PapyriteStatus.ok) {
        throw PapyriteException(
          owned.code,
          owned.error.isEmpty ? 'Papyrite FFI call failed' : owned.error,
        );
      }
      return owned;
    } finally {
      calloc.free(resultPtr);
    }
  }

  Uint8List _copyBuffer(_PapyriteBuffer buffer) {
    if (buffer.ptr == nullptr || buffer.len == 0) {
      return Uint8List(0);
    }
    return Uint8List.fromList(buffer.ptr.asTypedList(buffer.len));
  }

  static DynamicLibrary _openDefaultLibrary() {
    if (Platform.isIOS) {
      return DynamicLibrary.process();
    }
    if (Platform.isAndroid || Platform.isLinux) {
      return DynamicLibrary.open('libpapyrite_ffi.so');
    }
    if (Platform.isMacOS) {
      return DynamicLibrary.open('libpapyrite_ffi.dylib');
    }
    if (Platform.isWindows) {
      return DynamicLibrary.open('papyrite_ffi.dll');
    }
    throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
  }
}

class _OwnedResult {
  _OwnedResult({
    required this.code,
    required this.data,
    required this.error,
    required this.boolValue,
  });

  final int code;
  final Uint8List data;
  final String error;
  final bool boolValue;
}
