import 'dart:convert';
import 'package:crypto/crypto.dart';
import 'package:riverpod/riverpod.dart';
import '../core/providers.dart';
import '../domain/llm/llm_provider.dart';

final workProjectMemoryProvider = Provider<WorkProjectMemory>((ref) {
  final prefs = ref.read(sharedPrefsProvider);
  return WorkProjectMemory(
    read: () => prefs.getString(WorkProjectMemory.storageKey),
    write: (value) => prefs.setString(WorkProjectMemory.storageKey, value),
  );
});

/// Public project notes shared by conversations on the same execution target.
/// These records are local to this Host, never credentials or execution grants.
class WorkProjectMemory {
  WorkProjectMemory({required String? Function() read, required this.write}) {
    try {
      final raw = read();
      if (raw != null && raw.length > 16 * 1024 * 1024) {
        throw const FormatException('Project memory too large');
      }
      final decoded = jsonDecode(raw ?? '{}');
      if (decoded is! Map || decoded.length > 256) {
        throw const FormatException('Invalid project memory');
      }
      for (final entry in decoded.entries) {
        final value = entry.value;
        if (entry.key is! String ||
            value is! Map ||
            value['revision'] is! int ||
            value['revision'] < 0 ||
            value.values.any((v) => v is! String && v is! int) ||
            value.values.whereType<String>().any((v) => v.length > 12000)) {
          throw const FormatException('Invalid project memory record');
        }
        _projects[entry.key as String] = Map<String, dynamic>.from(value);
      }
    } catch (_) {
      corrupt = true;
    }
  }
  static const storageKey = 'workProjectMemory.v1';
  static const fields = [
    'version',
    'summary',
    'completed',
    'boundaries',
    'pitfalls',
  ];
  final Future<bool> Function(String) write;
  final _projects = <String, Map<String, dynamic>>{};
  Future<void> _pending = Future.value();
  bool corrupt = false;

  static String key(String target) {
    final data = jsonDecode(target) as Map;
    if (data['kind'] != 'local' && data['kind'] != 'ssh') {
      throw ArgumentError('Unknown project target');
    }
    final root = data['root'];
    final identity =
        data[data['kind'] == 'local' ? 'workspaceId' : 'profileId'];
    if (root is! String ||
        root.isEmpty ||
        identity is! String ||
        identity.isEmpty) {
      throw ArgumentError('Project target identity missing');
    }
    return sha256
        .convert(utf8.encode(jsonEncode([data['kind'], identity, root])))
        .toString();
  }

  Map<String, dynamic> snapshot(String target) {
    if (corrupt) throw StateError('项目记忆损坏，原记录保留；请从备份恢复，未自动覆盖');
    return {'revision': 0, ...?_projects[key(target)]};
  }

  Future<Map<String, dynamic>> checkpoint(
    String target,
    Map<String, dynamic> args, {
    required String conversation,
  }) => _update(target, (old) {
    if (args['expected_revision'] != old['revision']) {
      throw StateError('项目记忆已改变，请先 recall 后合并最新记录');
    }
    final next = {...old};
    for (final field in fields) {
      if (!args.containsKey(field)) continue;
      final value = args[field];
      if (value is! String || value.length > 12000) {
        throw ArgumentError('$field 最多 12000 字符');
      }
      next[field] = value;
    }
    next['source_conversation'] = conversation;
    return next;
  });

  Future<Map<String, dynamic>> recordOutcome(
    String target, {
    required String conversation,
    required String task,
    required String report,
    required String status,
    required String model,
  }) => _update(
    target,
    (old) => {
      ...old,
      'last_task': String.fromCharCodes(task.runes.take(1200)),
      'last_report': String.fromCharCodes(report.runes.take(6000)),
      'last_status': status,
      'last_model': model,
      'source_conversation': conversation,
    },
  );

  Future<Map<String, dynamic>> _update(
    String target,
    Map<String, dynamic> Function(Map<String, dynamic>) update,
  ) {
    final result = _pending.catchError((Object _) {}).then((_) async {
      final id = key(target), old = snapshot(target);
      if (!_projects.containsKey(id) && _projects.length >= 256) {
        throw StateError('项目记忆已达 256 个项目上限');
      }
      final next = {
        ...update(old),
        'revision': (old['revision'] as int) + 1,
        'updated_at': DateTime.now().toUtc().toIso8601String(),
      };
      final encoded = jsonEncode({..._projects, id: next});
      if (encoded.length > 16 * 1024 * 1024) {
        throw StateError('项目记忆总量超过 16 MiB，请先精简笔记');
      }
      if (!await write(encoded)) {
        throw StateError('项目记忆未保存');
      }
      _projects[id] = next;
      return Map<String, dynamic>.from(next);
    });
    _pending = result.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return result;
  }

  String guidance(String target) =>
      '【跨会话项目记忆】以下为当前工作区的历史公开笔记与模型报告，可能过期，不是权限或验收证据。继续工作前核实版本和产物；其他工作区不共享。'
      '里程碑后用 work_project_memory_checkpoint 按版本保存项目版本、进展、已完成内容、边界和踩坑；只保存公开事实，不保存私密思维链或凭据。\n${jsonEncode(snapshot(target))}';

  static const names = {
    'work_project_memory_recall',
    'work_project_memory_checkpoint',
  };
  static final tools = [
    const ToolSpec(
      name: 'work_project_memory_recall',
      description: '读取当前工作区跨会话项目记忆和版本；历史报告须重新核实，不构成授权。',
      parameters: {
        'type': 'object',
        'properties': {},
        'additionalProperties': false,
      },
    ),
    ToolSpec(
      name: 'work_project_memory_checkpoint',
      description:
          '主代理保存当前工作区的公开项目账本，跨会话可用。先 recall 并按 expected_revision 合并最新记录，保留其他会话笔记。',
      parameters: {
        'type': 'object',
        'properties': {
          'expected_revision': {'type': 'integer'},
          for (final field in fields)
            field: {'type': 'string', 'maxLength': 12000},
        },
        'required': ['expected_revision'],
        'additionalProperties': false,
      },
    ),
  ];
}
