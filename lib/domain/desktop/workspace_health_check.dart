import 'dart:convert';
import 'package:dio/dio.dart';
import '../llm/llm_provider.dart';
import 'workspace_tools.dart';
import 'command_runner.dart';

class WorkspaceCheckExecution {
  const WorkspaceCheckExecution(this.output, this.outcome);
  final String output;
  final WorkspaceCommandOutcome? outcome;
}

class WorkspaceCheck {
  const WorkspaceCheck(this.id, this.label, this.command);
  final String id, label, command;
  Map<String, dynamic> toJson() => {
    'id': id,
    'label': label,
    'command': command,
  };
}

/// Independent checks keep their actual exit statuses; no fail-fast summary.
class WorkspaceHealthCheck {
  static Future<WorkspaceCheckExecution> execute(
    WorkToolSession session,
    WorkspaceCheck check,
    CancelToken token,
  ) async {
    WorkspaceCommandOutcome? outcome;
    final output = await session.call(
      'work_run_command',
      {'command': check.command, 'timeout_seconds': 600},
      token,
      onComplete: (value) => outcome = value,
    );
    return WorkspaceCheckExecution(output, outcome);
  }

  static const tool = ToolSpec(
    name: 'work_check',
    description:
        '一键执行当前工作区已检测到的仓库检查、Rust 测试和 Flutter 测试，逐项返回实际退出码；失败后继续其他检查。检查会运行项目代码，使用现有命令批准和目录规则，工作目录不是沙箱。',
    parameters: {
      'type': 'object',
      'properties': {},
      'additionalProperties': false,
    },
  );

  static Future<List<WorkspaceCheck>> discover(
    WorkToolSession session,
    CancelToken token,
  ) async {
    Future<Set<String>> list(String path) async {
      final result = jsonDecode(
        await session.call('coding_list_directory', {
          'scope': 'root',
          'path': path,
        }, token),
      );
      if (result is Map && result['truncated'] == true) {
        throw StateError('体检目录列表被截断，请缩小工作区');
      }
      final entries = result is List
          ? result
          : (result as Map)['entries'] as List;
      return {
        for (final e in entries)
          if (e is Map &&
              e['kind'] != 'link-unavailable' &&
              e['kind'] != 'path-unavailable')
            e['name'] as String,
      };
    }

    final root = await list('.');
    final scripts = root.contains('scripts')
        ? await list('scripts')
        : <String>{};
    final next = root.contains('next') ? await list('next') : <String>{};
    return [
      if (root.contains('check.py') || scripts.contains('check.py'))
        WorkspaceCheck(
          'repository',
          '仓库检查',
          'python ${root.contains('check.py') ? 'check.py' : 'scripts/check.py'}${root.contains('pubspec.yaml') && next.contains('Cargo.toml') ? ' --scope client --quick' : ''}',
        ),
      if (root.contains('Cargo.toml') || next.contains('Cargo.toml'))
        WorkspaceCheck(
          'rust',
          'Rust 测试',
          'cargo test --locked${root.contains('Cargo.toml') ? '' : ' --manifest-path next/Cargo.toml'}',
        ),
      if (root.contains('pubspec.yaml'))
        const WorkspaceCheck('flutter', 'Flutter 测试', 'flutter test --no-pub'),
    ];
  }

  static Future<Map<String, dynamic>> run({
    required List<WorkspaceCheck> checks,
    required CancelToken token,
    required Future<WorkspaceCheckExecution> Function(WorkspaceCheck) execute,
    void Function(List<Map<String, dynamic>>)? onUpdate,
  }) async {
    final results = [
      for (final check in checks) {...check.toJson(), 'status': 'pending'},
    ];
    void publish() => onUpdate?.call([
      for (final row in results) Map<String, dynamic>.from(row),
    ]);
    publish();
    for (var index = 0; index < checks.length; index++) {
      final row = results[index];
      if (token.isCancelled) {
        row['status'] = 'not_run';
        row['reason'] = '用户停止，未执行';
        continue;
      }
      row['status'] = 'running';
      publish();
      final watch = Stopwatch()..start();
      try {
        final response = await execute(checks[index]);
        final code = response.outcome?.exitCode;
        final cancelled = response.outcome?.cancelled == true;
        final timeout = response.outcome?.timedOut == true;
        row['exit_code'] = code;
        row['status'] = cancelled
            ? 'cancelled'
            : timeout
            ? 'timed_out'
            : code == 0
            ? 'passed'
            : 'failed';
        final text = response.output;
        row['output'] = text.length > 8000
            ? text.substring(text.length - 8000)
            : text;
        if (code == null && !cancelled && !timeout) row['reason'] = '没有可核验退出码';
      } catch (error) {
        row['status'] = token.isCancelled ? 'cancelled' : 'failed';
        row['reason'] = '$error';
      }
      row['duration_ms'] = watch.elapsedMilliseconds;
      publish();
    }
    publish();
    return {
      'ok': results.isNotEmpty && results.every((r) => r['status'] == 'passed'),
      'checks': results,
      if (results.isEmpty)
        'note': '未检测到 check.py、Cargo.toml 或 pubspec.yaml，没有执行检查',
    };
  }
}
