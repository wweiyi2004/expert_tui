import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:dio/dio.dart';
import 'command_runner_contract.dart';
import 'shell_path.dart';
import '../research/command_output_buffer.dart';

Future<String> runWorkspaceCommand(
  String command,
  String directory,
  Duration timeout,
  CancelToken token, {
  WorkspaceCommandOutput? onOutput,
  WorkspaceCommandCompletion? onComplete,
}) async {
  if (token.isCancelled) throw token.cancelError!;
  final watch = Stopwatch()..start();
  final cwd = Platform.isWindows ? windowsShellPath(directory) : directory;
  final bypass = [
    Platform.environment['NO_PROXY'] ?? '',
    Platform.environment['no_proxy'] ?? '',
    'localhost,127.0.0.1,::1',
  ].where((s) => s.isNotEmpty).join(',');
  final process = await Process.start(
    Platform.isWindows ? 'powershell.exe' : '/bin/sh',
    Platform.isWindows
        ? [
            '-NoProfile',
            '-NonInteractive',
            '-Command',
            '\$OutputEncoding = [Console]::OutputEncoding = [Console]::InputEncoding = [System.Text.UTF8Encoding]::new(\$false); '
                "Set-Location -LiteralPath '${cwd.replaceAll("'", "''")}' -ErrorAction Stop; "
                '[Environment]::CurrentDirectory = (Get-Location).ProviderPath; '
                // Windows PowerShell 5.1 uses the system .NET proxy and ignores
                // NO_PROXY. Change only this child process's loopback bypass.
                '\$expertProxy = [Net.WebRequest]::DefaultWebProxy; '
                'if (\$expertProxy -is [Net.WebProxy]) { '
                '\$expertProxy.BypassArray = @(\$expertProxy.BypassArray) + '
                '@(\'(?i)^https?://(localhost|127\\.[0-9]+\\.[0-9]+\\.[0-9]+|\\[::1\\])(:[0-9]+)?/?\$\') }; '
                '\$global:LASTEXITCODE = \$null;\n$command\n'
                '\$expertCommandSucceeded = \$?; '
                'if (\$null -ne \$global:LASTEXITCODE) { exit \$global:LASTEXITCODE }; '
                'if (-not \$expertCommandSucceeded) { exit 1 }; exit 0',
          ]
        : ['-c', command],
    workingDirectory: cwd,
    runInShell: false,
    environment: Platform.isWindows
        ? {
            'PYTHONIOENCODING': 'utf-8',
            'PYTHONUTF8': '1',
            'NO_PROXY': bypass,
            'no_proxy': bypass,
          }
        : null,
  );
  final out = CommandOutputBuffer(maxChars: 64000),
      err = CommandOutputBuffer(maxChars: 64000);
  final output = decodeCommandOutput(process.stdout).listen((v) {
    out.add(v);
    onOutput?.call(v, false);
  });
  final errors = decodeCommandOutput(process.stderr).listen((v) {
    err.add(v);
    onOutput?.call(v, true);
  });
  var stopped = false;
  var terminationConfirmed = false;
  Future<void> stop() async {
    if (stopped) return;
    stopped = true;
    if (Platform.isWindows) {
      final killed = await Process.run('taskkill.exe', [
        '/PID',
        '${process.pid}',
        '/T',
        '/F',
      ]);
      terminationConfirmed = killed.exitCode == 0;
    } else {
      // A successful signal does not confirm termination of descendants.
      process.kill();
    }
  }

  final outputDone = output.asFuture<void>(),
      errorsDone = errors.asFuture<void>();
  final done = process.exitCode.then((code) async {
    await Future.wait([outputDone, errorsDone]);
    return code;
  });
  final cancel = token.whenCancel.then((_) async {
    if (!stopped) await stop();
    return -1;
  });
  int exit;
  var timedOut = false;
  try {
    exit = await Future.any([done, cancel]).timeout(timeout);
  } on TimeoutException {
    timedOut = true;
    await stop();
    exit = -1;
  } finally {
    if (stopped) {
      // Drain final output after taskkill without waiting forever on a child
      // that inherited a pipe. The timeout/cancel status remains authoritative.
      await done.timeout(const Duration(seconds: 3), onTimeout: () => -1);
    }
    stopped = true;
    await output.cancel();
    await errors.cancel();
  }
  final outcome = WorkspaceCommandOutcome(
    exitCode: timedOut || token.isCancelled ? null : exit,
    cancelled: token.isCancelled,
    timedOut: timedOut,
    stdoutTruncated: out.truncated,
    stderrTruncated: err.truncated,
    stdoutChars: out.totalChars,
    stderrChars: err.totalChars,
    elapsedMilliseconds: watch.elapsedMilliseconds,
    terminationRequested: timedOut || token.isCancelled,
    terminationConfirmed: terminationConfirmed,
  );
  onComplete?.call(outcome);
  final heading = timedOut
      ? '命令超时，已请求终止本机进程。'
      : '退出码：$exit${token.isCancelled ? '（已取消）' : ''}';
  return '$heading\n${out.text}${err.totalChars == 0 ? '' : '\n标准错误：\n${err.text}'}\n命令状态：${jsonEncode(outcome.toJson())}';
}
