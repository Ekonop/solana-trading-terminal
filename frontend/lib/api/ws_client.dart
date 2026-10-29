import 'dart:async';
import 'dart:convert';

import 'package:web_socket_channel/web_socket_channel.dart';

import '../config.dart';
import '../models/models.dart';

/// WebSocket feed with automatic reconnect and backoff.
///
/// Emits decoded [WsEvent] objects to [onEvent]. Connection state changes are
/// pushed to [onState] (connected/disconnected) so the UI can show a live dot.
class WsClient {
  WsClient({required this.cfg, required this.onEvent, this.onState});

  final AppConfig cfg;
  final void Function(WsEvent) onEvent;
  final void Function(bool connected)? onState;

  WebSocketChannel? _channel;
  StreamSubscription? _sub;
  Timer? _reconnectTimer;
  bool _closed = false;
  bool _connected = false;
  var _attempt = 0;

  bool get isConnected => _connected;

  void start() {
    _closed = false;
    _connect();
  }

  void _connect() {
    if (_closed) return;
    try {
      _channel = WebSocketChannel.connect(Uri.parse(cfg.wsUrl));
      _sub = _channel!.stream.listen(_onMessage, onDone: _onDone, onError: (_) => _onDone());
      _connected = true;
      _attempt = 0;
      onState?.call(true);
    } catch (_) {
      _scheduleReconnect();
    }
  }

  void _onMessage(dynamic raw) {
    if (raw is! String) return;
    try {
      final json = jsonDecode(raw);
      if (json is Map<String, dynamic>) {
        onEvent(WsEvent.fromJson(json));
      }
    } catch (_) {
      // ignore malformed frames
    }
  }

  void _onDone() {
    _connected = false;
    onState?.call(false);
    _scheduleReconnect();
  }

  void _scheduleReconnect() {
    if (_closed) return;
    _attempt++;
    final delay = Duration(milliseconds: (_attempt > 10 ? 10 : _attempt) * 1000);
    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(delay, _connect);
  }

  void close() {
    _closed = true;
    _reconnectTimer?.cancel();
    _sub?.cancel();
    _channel?.sink.close();
    _connected = false;
  }
}