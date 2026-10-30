import base64
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import struct
import zlib
import zstandard
from PIL import Image, ImageOps
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject, NameObject, DecodedStreamObject

WORKER = Path(__file__).with_name("desktop_pdf_worker.py")
ROOT = Path(__file__).resolve().parents[1]
RUST = Path(os.environ.get('EXPERT_TEST_MEDIA_BINARY', str(
    ROOT / 'output/native/bundle/expert-media-worker/expert-media-worker.exe')))


class MediaWorkerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not RUST.is_file():
            raise RuntimeError('Build the Rust worker or set EXPERT_TEST_MEDIA_BINARY')
        if '(Rust)' not in subprocess.check_output([str(RUST), '--version'], text=True):
            raise RuntimeError('Image/Zstd acceptance requires the Rust executable')

    def request(self, operation, data, use_pdf=False, **extra):
        with tempfile.TemporaryDirectory(prefix="expert-media-test-") as directory:
            request = Path(directory) / "request.json"
            request.write_text(json.dumps({"op": operation, "data": base64.b64encode(data).decode(), **extra}), encoding="utf-8")
            if operation == 'pdf_text' or use_pdf:
                binary = os.environ.get('EXPERT_TEST_PDF_BINARY')
                default = ROOT / 'output/native/bundle/expert-pdf-worker/expert-pdf-worker.exe'
                binary = binary or (str(default) if default.is_file() else None)
                command = [binary] if binary else [sys.executable, str(WORKER)]
            else:
                command = [str(RUST)]
            result = subprocess.run([*command, str(request)], capture_output=True, timeout=25)
            self.assertEqual(result.stderr, b'', 'Worker leaked parser diagnostics')
            return result.returncode, json.loads(result.stdout)

    def test_resize_keeps_aspect_ratio(self):
        image = Image.new("RGB", (3200, 1600), "blue")
        source = io.BytesIO(); image.save(source, format="PNG")
        code, result = self.request("image_resize", source.getvalue(), max_side=1000)
        self.assertEqual(code, 0); self.assertTrue(result["changed"])
        with Image.open(io.BytesIO(base64.b64decode(result["data"]))) as resized:
            self.assertEqual(resized.size, (1000, 500))

    def test_small_image_preserves_original_bytes(self):
        source = io.BytesIO(); Image.new("RGBA", (16, 8), (0, 0, 0, 0)).save(source, format="PNG")
        code, result = self.request("image_resize", source.getvalue(), max_side=1000)
        self.assertEqual(code, 0); self.assertFalse(result["changed"])

    def test_empty_pdf_is_a_valid_empty_result(self):
        document = PdfWriter(); document.add_blank_page(width=100, height=100)
        source = io.BytesIO(); document.write(source)
        code, result = self.request("pdf_text", source.getvalue())
        self.assertEqual(code, 0); self.assertEqual(result["text"], "")

    def test_parser_failure_never_echoes_input(self):
        code, result = self.request("pdf_text", b"SYNTHETIC_PRIVATE_DOCUMENT")
        self.assertNotEqual(code, 0); self.assertFalse(result["ok"])
        self.assertNotIn("SYNTHETIC_PRIVATE_DOCUMENT", json.dumps(result))

    def image(self, size=(80, 40), format='PNG', **save):
        source = io.BytesIO()
        Image.new('RGB', size).save(source, format=format, **save)
        return source.getvalue()

    def test_supported_small_formats_retain_original_bytes(self):
        for format in ['PNG', 'JPEG', 'GIF', 'WEBP', 'BMP']:
            with self.subTest(format=format):
                code, result = self.request('image_resize', self.image(format=format), max_side=1000)
                self.assertEqual(code, 0)
                self.assertEqual(result, {'ok': True, 'changed': False})

    def test_alpha_survives_resize(self):
        source = io.BytesIO()
        Image.new('RGBA', (80, 40), (10, 20, 30, 64)).save(source, format='PNG')
        code, result = self.request('image_resize', source.getvalue(), max_side=20)
        self.assertEqual(code, 0)
        with Image.open(io.BytesIO(base64.b64decode(result['data']))) as image:
            self.assertEqual(image.size, (20, 10))
            self.assertEqual(image.convert('RGBA').getpixel((10, 5)), (10, 20, 30, 64))

    def test_icc_profile_survives_resize(self):
        from PIL import ImageCms
        profile = ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
        source = io.BytesIO()
        Image.new('RGB', (80, 40)).save(source, format='PNG', icc_profile=profile)
        code, result = self.request('image_resize', source.getvalue(), max_side=20)
        self.assertEqual(code, 0)
        with Image.open(io.BytesIO(base64.b64decode(result['data']))) as image:
            self.assertEqual(image.info['icc_profile'], profile)

    def test_all_eight_exif_orientations_match_visible_geometry(self):
        image = Image.new('RGB', (80, 40))
        for box, color in [((0, 0, 40, 20), 'red'), ((40, 0, 80, 20), 'green'),
                           ((0, 20, 40, 40), 'blue'), ((40, 20, 80, 40), 'yellow')]:
            image.paste(color, box)
        for orientation in range(1, 9):
            with self.subTest(orientation=orientation):
                source = io.BytesIO()
                exif = Image.Exif(); exif[274] = orientation
                image.save(source, format='JPEG', quality=100, subsampling=0, exif=exif)
                with Image.open(io.BytesIO(source.getvalue())) as original:
                    expected = ImageOps.exif_transpose(original).convert('RGB')
                    code, result = self.request('image_resize', source.getvalue(), max_side=1000)
                    self.assertEqual(code, 0)
                    if orientation == 1:
                        self.assertFalse(result['changed'])
                        continue
                    self.assertTrue(result['changed'])
                    with Image.open(io.BytesIO(base64.b64decode(result['data']))) as actual:
                        actual = actual.convert('RGB')
                        self.assertEqual(actual.size, expected.size)
                        for x in [actual.width // 4, actual.width * 3 // 4]:
                            for y in [actual.height // 4, actual.height * 3 // 4]:
                                self.assertLessEqual(max(abs(a-b) for a,b in zip(actual.getpixel((x,y)), expected.getpixel((x,y)))), 12)

    def test_png_and_webp_exif_are_also_applied(self):
        for format in ['PNG', 'WEBP']:
            with self.subTest(format=format):
                exif = Image.Exif(); exif[274] = 6
                code, result = self.request('image_resize', self.image(format=format, exif=exif), max_side=1000)
                self.assertEqual(code, 0)
                self.assertTrue(result['changed'])
                with Image.open(io.BytesIO(base64.b64decode(result['data']))) as image:
                    self.assertEqual(image.size, (40, 80))

    def test_animated_gif_resize_uses_first_frame(self):
        source = io.BytesIO()
        Image.new('RGB', (80, 40), 'red').save(source, format='GIF', save_all=True,
            append_images=[Image.new('RGB', (80, 40), 'blue')], duration=100, loop=0)
        code, result = self.request('image_resize', source.getvalue(), max_side=20)
        self.assertEqual(code, 0)
        with Image.open(io.BytesIO(base64.b64decode(result['data']))) as image:
            self.assertEqual(image.size, (20, 10))
            self.assertEqual(image.convert('RGB').getpixel((10,5)), (255,0,0))

    def test_invalid_dimensions_limit_is_rejected(self):
        for limit in [0, 4097, -1]:
            with self.subTest(limit=limit):
                code, result = self.request('image_resize', self.image(), max_side=limit)
                self.assertNotEqual(code, 0); self.assertFalse(result['ok'])

    def test_image_pixel_limit_is_checked_before_allocating_pixels(self):
        data = bytearray(self.image((1, 1)))
        data[16:24] = struct.pack('>II', 40001, 1000)
        data[29:33] = struct.pack('>I', zlib.crc32(data[12:29]))
        code, result = self.request('image_resize', data)
        self.assertNotEqual(code, 0); self.assertFalse(result['ok'])

    def test_malformed_image_does_not_echo_source(self):
        code, result = self.request('image_resize', b'SYNTHETIC_PRIVATE_IMAGE')
        self.assertNotEqual(code, 0)
        self.assertNotIn('SYNTHETIC_PRIVATE_IMAGE', json.dumps(result))

    def test_compressed_unicode_session_round_trip(self):
        text = '{"content":"中文会话"}\n'
        code, result = self.request('session_zstd', zstandard.ZstdCompressor().compress(text.encode()))
        self.assertEqual(code, 0); self.assertEqual(result['text'], text)

    def test_first_frame_semantics_are_preserved(self):
        compressor = zstandard.ZstdCompressor()
        code, result = self.request('session_zstd', compressor.compress(b'first') + compressor.compress(b'second'))
        self.assertEqual(code, 0); self.assertEqual(result['text'], 'first')

    def test_decompression_limit(self):
        for length in [16*1024*1024, 16*1024*1024+1]:
            with self.subTest(length=length):
                code, result = self.request('session_zstd', zstandard.ZstdCompressor().compress(b'x'*length))
                if length == 16*1024*1024:
                    self.assertEqual(code, 0); self.assertEqual(len(result['text']), length)
                else:
                    self.assertNotEqual(code, 0); self.assertFalse(result['ok'])

    def test_invalid_and_truncated_sessions_are_rejected(self):
        valid = zstandard.ZstdCompressor(write_checksum=True).compress(b'content')
        for data in [b'private malformed input', valid[:-1], valid[:-1]+bytes([valid[-1]^1])]:
            with self.subTest(data_length=len(data)):
                code, result = self.request('session_zstd', data)
                self.assertNotEqual(code, 0); self.assertFalse(result['ok'])
                self.assertNotIn('private malformed input', json.dumps(result))

    def test_non_utf8_session_is_rejected(self):
        code, result = self.request('session_zstd', zstandard.ZstdCompressor().compress(b'\xff'))
        self.assertNotEqual(code, 0); self.assertFalse(result['ok'])

    def test_serialized_output_is_bounded(self):
        code, result = self.request('session_zstd', zstandard.ZstdCompressor().compress(b'\x00'*(5*1024*1024)))
        self.assertNotEqual(code, 0); self.assertEqual(result['error'], 'OutputTooLarge')

    def test_binary_input_limit(self):
        code, result = self.request('session_zstd', b'x'*(15*1024*1024+1))
        self.assertNotEqual(code, 0); self.assertEqual(result['error'], 'InputTooLarge')

    def test_pdf_text_still_works_without_pillow_in_the_shipped_worker(self):
        document = PdfWriter(); page = document.add_blank_page(width=100, height=100)
        font = DictionaryObject({NameObject('/Type'):NameObject('/Font'),
            NameObject('/Subtype'):NameObject('/Type1'), NameObject('/BaseFont'):NameObject('/Helvetica')})
        page[NameObject('/Resources')] = DictionaryObject({NameObject('/Font'):
            DictionaryObject({NameObject('/F1'): document._add_object(font)})})
        stream = DecodedStreamObject(); stream.set_data(b'BT /F1 12 Tf 10 50 Td (PDF_ROUND1_OK) Tj ET')
        page[NameObject('/Contents')] = document._add_object(stream)
        source = io.BytesIO(); document.write(source)
        code, result = self.request('pdf_text', source.getvalue())
        self.assertEqual(code, 0); self.assertIn('PDF_ROUND1_OK', result['text'])

    def test_workers_do_not_silently_fall_back_to_each_other(self):
        code, result = self.request('unsupported', b'private')
        self.assertNotEqual(code, 0); self.assertEqual(result['error'], 'UnsupportedOperation')

    def test_pdf_worker_no_longer_handles_images_or_zstd(self):
        for operation in ['image_resize', 'session_zstd']:
            with self.subTest(operation=operation):
                code, result = self.request(operation, b'private', use_pdf=True)
                self.assertNotEqual(code, 0); self.assertFalse(result['ok'])


if __name__ == "__main__":
    unittest.main()
