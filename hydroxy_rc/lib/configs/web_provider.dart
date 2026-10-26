/// Conditional export that picks the right implementation based on platform.
///
/// On web: uses the real `web_self.dart` (which imports `package:web`).
/// On all other platforms: uses `web_self_stub.dart` (no web dependency).
export 'web_stub.dart'
    if (dart.library.js_interop) 'web_self.dart';
