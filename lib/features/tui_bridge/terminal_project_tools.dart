part of 'backend.dart';

extension TerminalProjectTools on ControllerBridgeBackend {
  void _auditUserCommand(
    String session,
    String invocation,
    String command,
    String target,
    String status, {
    int? exitCode,
  }) {
    container
        .read(workRuntimeLogsProvider.notifier)
        .add(
          conversationId: session,
          toolCallId: invocation,
          toolName: 'work_run_command',
          phase: 'command_audit',
          status: status,
          detail: jsonEncode({
            'origin': 'tui_user',
            'target_sha256': sha256.convert(utf8.encode(target)).toString(),
            'command_sha256': sha256.convert(utf8.encode(command)).toString(),
            'exit_code': exitCode,
            'shell_os_sandbox': false,
          }),
        );
  }

  Future<WorkToolSession> _projectWork(String id) async {
    final policy = _policy(id);
    final work = policy.sshProfileId.isEmpty
        ? await _localWork(id)
        : await container
              .read(researchTerminalProvider.notifier)
              .openWorkSession(policy.sshProfileId, policy.remoteRoot);
    try {
      validateWorkspaceScope(work.tools, 'coding_read_file', {'scope': 'root'});
      return work;
    } catch (_) {
      await work.close();
      rethrow;
    }
  }

  Future<Map<String, Object?>> _projectPanel(
    String op,
    String id,
    Map<String, dynamic> args,
  ) async {
    final work = await _projectWork(id);
    try {
      if (op.endsWith('.save') &&
          args['expected_target'] != work.changeTarget) {
        throw const BridgeFailure('conflict', '工作区已改变，请重新打开项目记忆');
      }
      final memory = container.read(workProjectMemoryProvider);
      if (op == 'wb.project.save') {
        await memory.checkpoint(work.changeTarget, {
          'expected_revision': args['expected_revision'],
          for (final field in WorkProjectMemory.fields)
            field: args[field] ?? '',
        }, conversation: id);
      }
      final notes = memory.snapshot(work.changeTarget);
      return _panel(
        '项目记忆 · ${work.target}',
        [
          _field('__note', '跨会话共享于当前工作区；历史报告须复核，不代表授权或验收。', '', false),
          for (final (field, label) in [
            ('version', '项目版本'),
            ('summary', '当前进展'),
            ('completed', '已完成'),
            ('boundaries', '边界与约束'),
            ('pitfalls', '踩坑记录'),
          ])
            _field(field, label, notes[field] as String? ?? ''),
          _field('__save', '保存项目记忆', '', false, 'wb.project.save'),
          _field(
            '__revision',
            '版本 ${notes['revision']} · ${notes['updated_at'] ?? '尚无记录'}',
            '',
            false,
          ),
          if (notes['last_report'] != null)
            ...(_textPanel(
                  '最近报告',
                  '最近任务：${notes['last_task']}\n状态：${notes['last_status']} · 模型：${notes['last_model']}\n${notes['last_report']}',
                )['fields']
                as List<Map<String, Object?>>),
        ],
        {
          'expected_target': work.changeTarget,
          'expected_revision': notes['revision'],
        },
      );
    } finally {
      await work.close();
    }
  }

  Future<Map<String, Object?>> _checkPanel(
    String op,
    String id,
    String scope,
    Map<String, dynamic> args,
  ) async {
    final work = await _projectWork(id);
    try {
      final target = work.changeTarget;
      final discovered = await WorkspaceHealthCheck.discover(
        work,
        CancelToken(),
      );
      if (op == 'wb.check') {
        return _panel(
          '项目体检 · ${work.target}',
          [
            _field(
              '__note',
              '以下命令运行项目代码，使用当前系统账号；工作目录不是沙箱。逐项执行，失败后继续，停止后未运行项不会计为通过。',
              '',
              false,
            ),
            for (final check in discovered)
              _field('command:${check.id}', check.label, check.command),
            if (discovered.isNotEmpty)
              _field('__run', '运行以上检查', '', false, 'wb.check.run')
            else
              _field(
                '__empty',
                '未检测到 check.py、Cargo.toml 或 pubspec.yaml',
                '',
                false,
              ),
          ],
          {
            'expected_target': target,
            'checks': discovered.map((c) => c.toJson()).toList(),
            'expected_plan': _terminalDigest(
              discovered.map((c) => c.toJson()).toList(),
            ),
          },
        );
      }
      if (op != 'wb.check.run') {
        throw const BridgeFailure('unsupported', '未知体检操作');
      }
      if (args['expected_target'] != target) {
        throw const BridgeFailure('conflict', '工作区已改变，请重新打开体检');
      }
      if (args['expected_plan'] !=
          _terminalDigest(discovered.map((c) => c.toJson()).toList())) {
        throw const BridgeFailure('conflict', '检测到的检查项已改变，请重新打开体检');
      }
      _idle(id);
      final checks = [
        for (final check in discovered)
          WorkspaceCheck(
            check.id,
            check.label,
            args['command:${check.id}'] as String? ?? check.command,
          ),
      ];
      if (checks.any(
        (c) => c.command.trim().isEmpty || c.command.length > 16000,
      )) {
        throw const BridgeFailure('invalid_params', '体检命令须非空且不超过 16000 字符');
      }
      return _startJob(scope, id, '项目体检', (job) async {
        final live = await _projectWork(id);
        try {
          if (live.changeTarget != target) {
            throw const BridgeFailure('conflict', '工作区已改变，体检未执行');
          }
          final audited = <String, String>{};
          final result = await WorkspaceHealthCheck.run(
            checks: checks,
            token: job.token,
            execute: (check) =>
                WorkspaceHealthCheck.execute(live, check, job.token),
            onUpdate: (rows) {
              job.output = rows
                  .map(
                    (r) =>
                        '${r['label']}: ${r['status']} · 退出码 ${r['exit_code'] ?? '—'}\n${r['reason'] ?? ''}\n${r['output'] ?? ''}',
                  )
                  .join('\n');
              for (final row in rows) {
                final status = row['status'] as String,
                    key = row['id'] as String;
                if (status == 'pending' || audited[key] == status) continue;
                audited[key] = status;
                container
                    .read(workRuntimeLogsProvider.notifier)
                    .add(
                      conversationId: id,
                      phase: 'command_audit',
                      status: status,
                      toolName: 'work_check',
                      toolCallId: '${job.id}:$key',
                      detail: jsonEncode({
                        'origin': 'tui_user',
                        'target_sha256': sha256
                            .convert(utf8.encode(target))
                            .toString(),
                        'command_sha256': sha256
                            .convert(utf8.encode(row['command'] as String))
                            .toString(),
                        'exit_code': row['exit_code'],
                        'shell_os_sandbox': false,
                      }),
                    );
              }
            },
          );
          await container.read(workRuntimeLogsProvider.notifier).flush();
          return {
            ..._textPanel(
              result['ok'] == true ? '体检通过' : '体检未全部通过',
              job.output,
            ),
            'done': true,
            'job_id': job.id,
            'checks': result['checks'],
            'ok': result['ok'],
          };
        } finally {
          await live.close();
        }
      });
    } finally {
      await work.close();
    }
  }
}
