import 'local_file_reader_io.dart';

Stream<List<int>>? openLocalFileReadStream(String? path) =>
    openLocalFileReadStreamImpl(path);
