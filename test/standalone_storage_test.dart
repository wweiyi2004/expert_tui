import 'dart:io';
import 'package:expert_tui/platform/paths_host.dart';
import 'package:expert_tui/platform/preferences_host.dart';
import 'package:expert_tui/platform/credentials_host.dart';
import 'package:test/test.dart';
import 'package:path/path.dart' as p;

void main() {
  test('standalone state keeps documents, preferences and credentials together', () async {
    final state = await Directory.systemTemp.createTemp('expert-tui-storage-');
    HostPaths.stateDirectory = state.path;
    try {
      final support = await getApplicationSupportDirectory();
      final documents = await getApplicationDocumentsDirectory();
      expect(support.path, state.path);
      expect(documents.path, p.join(state.path, 'documents'));
      final preferences = await SharedPreferences.getInstance();
      await preferences.setString('standalone-fixture', 'saved');
      const credentials = FlutterSecureStorage();
      await credentials.write(key: 'standalone-fixture', value: 'synthetic');
      expect(await credentials.read(key: 'standalone-fixture'), 'synthetic');
      expect(await File(p.join(state.path, 'flutter_secure_storage.dat')).exists(), isTrue);
      expect((await state.list().toList()).length, greaterThanOrEqualTo(3));
    } finally {
      HostPaths.stateDirectory = null;
      await state.delete(recursive: true);
    }
  }, skip: !Platform.isWindows);
}
