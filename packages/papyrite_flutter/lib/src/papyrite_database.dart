import 'dart:convert';

import 'papyrite_bindings.dart';

class PapyriteDatabase {
  PapyriteDatabase(this.path, {PapyriteBindings? bindings})
      : _bindings = bindings ?? PapyriteBindings.open();

  final String path;
  final PapyriteBindings _bindings;

  void create(Map<String, Object?> document) {
    createJson(jsonEncode(document));
  }

  void createJson(String documentJson) {
    _bindings.createJson(path, documentJson);
  }

  Map<String, Object?>? get(String id) {
    final json = getJson(jsonEncode({'_id': id}));
    if (json == null) {
      return null;
    }
    return _decodeObject(json);
  }

  String? getJson(String filterJson) {
    return _bindings.getJson(path, filterJson);
  }

  bool delete(String id) {
    return deleteJson(jsonEncode({'_id': id}));
  }

  bool deleteJson(String filterJson) {
    return _bindings.deleteJson(path, filterJson);
  }

  void update(
    String id, {
    Map<String, Object?> set = const {},
    List<String> unset = const [],
  }) {
    final request = <String, Object?>{
      'filter': {'_id': id},
    };
    if (set.isNotEmpty) {
      request['set'] = set;
    }
    if (unset.isNotEmpty) {
      request['unset'] = unset;
    }
    updateJson(jsonEncode(request));
  }

  void updateJson(String updateJson) {
    _bindings.updateJson(path, updateJson);
  }

  List<Map<String, Object?>> find(String fieldPath, Object? equals) {
    final json = findJson(jsonEncode({'path': fieldPath, 'eq': equals}));
    return _decodeObjectList(json);
  }

  String findJson(String queryJson) {
    return _bindings.findJson(path, queryJson);
  }

  List<Map<String, Object?>> dump() {
    return _decodeObjectList(dumpJson());
  }

  String dumpJson() {
    return _bindings.dumpJson(path);
  }

  Map<String, Object?> _decodeObject(String source) {
    final decoded = jsonDecode(source);
    if (decoded is! Map) {
      throw FormatException('Expected JSON object', source);
    }
    return decoded.cast<String, Object?>();
  }

  List<Map<String, Object?>> _decodeObjectList(String source) {
    final decoded = jsonDecode(source);
    if (decoded is! List) {
      throw FormatException('Expected JSON array', source);
    }
    return decoded.map((item) {
      if (item is! Map) {
        throw FormatException('Expected JSON object in array', source);
      }
      return item.cast<String, Object?>();
    }).toList(growable: false);
  }
}
