/// Extended Windows paths belong to filesystem APIs. Legacy shells interpret
/// them as providers/UNC paths and can silently start in a different directory.
String windowsShellPath(String path) {
  if (path.toLowerCase().startsWith(r'\\?\unc\')) {
    return r'\\' + path.substring(8);
  }
  if (RegExp(r'^\\\\\?\\[a-zA-Z]:\\').hasMatch(path)) {
    return path.substring(4);
  }
  return path;
}
