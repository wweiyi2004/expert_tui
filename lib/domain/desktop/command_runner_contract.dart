/// Incremental decoded output. A chunk is not necessarily a complete line.
typedef WorkspaceCommandOutput = void Function(String text, bool isStderr);
typedef WorkspaceCommandCompletion =
    void Function(WorkspaceCommandOutcome outcome);

class WorkspaceCommandOutcome {
  const WorkspaceCommandOutcome({
    this.exitCode,
    this.cancelled = false,
    this.timedOut = false,
    this.stdoutTruncated = false,
    this.stderrTruncated = false,
    this.stdoutChars = 0,
    this.stderrChars = 0,
    this.elapsedMilliseconds,
    this.terminationRequested = false,
    this.terminationConfirmed = false,
  });

  final int? exitCode;
  final bool cancelled;
  final bool timedOut;
  final bool stdoutTruncated, stderrTruncated;
  final int stdoutChars, stderrChars;
  final int? elapsedMilliseconds;
  final bool terminationRequested, terminationConfirmed;

  Map<String, Object?> toJson() => {
    'exit_code': exitCode,
    'cancelled': cancelled,
    'timed_out': timedOut,
    'truncated': stdoutTruncated || stderrTruncated,
    'stdout_truncated': stdoutTruncated,
    'stderr_truncated': stderrTruncated,
    'stdout_chars': stdoutChars,
    'stderr_chars': stderrChars,
    'elapsed_ms': elapsedMilliseconds,
    'termination_requested': terminationRequested,
    'termination_confirmed': terminationConfirmed,
  };
}
