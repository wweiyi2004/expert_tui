import 'dart:convert';
import 'package:expert_tui/platform/media_worker.dart';
import 'package:test/test.dart';

void main() {
  test(
    'Dart routes image requests to the Rust worker and retains unchanged bytes',
    () {
      final result = runMediaWorker({
        'op': 'image_resize',
        'data':
            'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC',
        'max_side': 1,
      });
      expect(result, {'ok': true, 'changed': false});
    },
  );

  test('Dart routes compressed sessions to the Rust worker', () {
    final result = runMediaWorker({
      'op': 'session_zstd',
      'data': 'KLUv/SANaQAA5Lit5paH5Lya6K+dCg==',
    });
    expect(result['text'], '中文会话\n');
  });

  test(
    'Dart continues routing PDF text requests to the separate Python worker',
    () {
      final result = runMediaWorker({
        'op': 'pdf_text',
        'data':
            'JVBERi0xLjMKJeLjz9MKMSAwIG9iago8PAovUHJvZHVjZXIgKHB5cGRmKQo+PgplbmRvYmoKMiAwIG9iago8PAovVHlwZSAvUGFnZXMKL0NvdW50IDEKL0tpZHMgWyA0IDAgUiBdCj4+CmVuZG9iagozIDAgb2JqCjw8Ci9UeXBlIC9DYXRhbG9nCi9QYWdlcyAyIDAgUgo+PgplbmRvYmoKNCAwIG9iago8PAovVHlwZSAvUGFnZQovUmVzb3VyY2VzIDw8Cj4+Ci9NZWRpYUJveCBbIDAuMCAwLjAgMTAwIDEwMCBdCi9QYXJlbnQgMiAwIFIKPj4KZW5kb2JqCnhyZWYKMCA1CjAwMDAwMDAwMDAgNjU1MzUgZiAKMDAwMDAwMDAxNSAwMDAwMCBuIAowMDAwMDAwMDU0IDAwMDAwIG4gCjAwMDAwMDAxMTMgMDAwMDAgbiAKMDAwMDAwMDE2MiAwMDAwMCBuIAp0cmFpbGVyCjw8Ci9TaXplIDUKL1Jvb3QgMyAwIFIKL0luZm8gMSAwIFIKPj4Kc3RhcnR4cmVmCjI1NgolJUVPRgo=',
      });
      expect(result['ok'], true);
      expect(result['text'], '');
    },
  );

  test('malformed images fail without returning their private contents', () {
    expect(
      () => runMediaWorker({
        'op': 'image_resize',
        'data': base64Encode(utf8.encode('PRIVATE_IMAGE_PAYLOAD')),
      }),
      throwsA(
        isA<StateError>().having(
          (error) => error.toString(),
          'message',
          isNot(contains('PRIVATE_IMAGE_PAYLOAD')),
        ),
      ),
    );
  });
}
