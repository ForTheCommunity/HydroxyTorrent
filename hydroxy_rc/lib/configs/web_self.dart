import 'package:web/web.dart' as web;

class WebUrlDetails {
  final String fullUrl;
  final String hostname;
  final String port;
  final String protocol;
  final String origin;
  final String pathname;

  const WebUrlDetails({
    required this.fullUrl,
    required this.hostname,
    required this.port,
    required this.protocol,
    required this.origin,
    required this.pathname,
  });

  factory WebUrlDetails.current() {
    final location = web.window.location;

    return WebUrlDetails(
      fullUrl: location.href,
      hostname: location.hostname,
      port: location.port.isNotEmpty
          ? location.port
          : (location.protocol == 'https:' ? '443' : '80'),
      protocol: location.protocol,
      origin: location.origin,
      pathname: location.pathname,
    );
  }

  String buildApiUrl({String endpoint = '/api'}) {
    return '$origin$endpoint';
  }
}
