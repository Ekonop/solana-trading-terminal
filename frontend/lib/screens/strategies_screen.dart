import 'package:flutter/material.dart';

import '../config.dart';
import '../models/models.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

class StrategiesScreen extends StatefulWidget {
  const StrategiesScreen({super.key, required this.state});
  final AppState state;

  @override
  State<StrategiesScreen> createState() => _StrategiesScreenState();
}

class _StrategiesScreenState extends State<StrategiesScreen> {
  String? _busy;

  Future<void> _act(String id, String action) async {
    setState(() => _busy = '$id:$action');
    final st = widget.state;
    switch (action) {
      case 'start':
        await st.withCall(() => st.api.strategyStart(id));
        break;
      case 'pause':
        await st.withCall(() => st.api.strategyPause(id));
        break;
      case 'stop':
        await st.withCall(() => st.api.strategyStop(id));
        break;
    }
    if (mounted) setState(() => _busy = null);
  }

  @override
  Widget build(BuildContext context) {
    final strategies = widget.state.strategies;
    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text('STRATEGIES', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                Text('5 VERSIONS · PAPER', style: Type.labelCaps),
              ],
            ),
            const SizedBox(height: 16),
            for (final s in strategies) ...[
              _StrategyCard(
                s: s,
                busy: _busy == '${s.id}:${s.isRunning ? 'stop' : 'start'}',
                onAction: (a) => _act(s.id, a),
              ),
              const SizedBox(height: 8),
            ],
          ],
        ),
      ),
    );
  }
}

class _StrategyCard extends StatelessWidget {
  const _StrategyCard({required this.s, required this.busy, required this.onAction});
  final StrategyView s;
  final bool busy;
  final ValueChanged<String> onAction;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: T.panel,
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(Icons.account_tree_rounded, size: 14, color: s.isRunning ? T.up : T.fg3),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(s.displayName.toUpperCase(), style: Type.dataBold.copyWith(fontSize: 13)),
                    Text(s.description, style: Type.body, maxLines: 2, overflow: TextOverflow.ellipsis),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              StatusChip(label: s.status, state: s.status),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Wrap(
                  spacing: 16,
                  runSpacing: 6,
                  children: [
                    _kv('VERSION', 'v${s.configVersion}'),
                    _kv('SIGNALS', '${s.signalsTotal}'),
                    _kv('REJECTED', '${s.signalsRejected}'),
                    _kv('LAST EVAL', s.lastEvalAt == null ? '—' : Fmt.micros(s.lastEvalAt!)),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              if (!s.isRunning)
                TermButton(
                  label: 'START',
                  icon: Icons.play_arrow_rounded,
                  onPressed: busy ? null : () => onAction('start'),
                )
              else ...[
                TermButton(
                  label: 'PAUSE',
                  color: T.warn,
                  icon: Icons.pause_rounded,
                  onPressed: busy ? null : () => onAction('pause'),
                ),
                const SizedBox(width: 8),
                TermButton(
                  label: 'STOP',
                  color: T.down,
                  icon: Icons.stop_rounded,
                  onPressed: busy ? null : () => onAction('stop'),
                ),
              ],
            ],
          ),
          if (s.lastError != null && s.lastError!.isNotEmpty) ...[
            const SizedBox(height: 10),
            Text('FAULT: ${s.lastError}', style: Type.data.copyWith(color: T.down, fontSize: 10)),
          ],
        ],
      ),
    );
  }

  Widget _kv(String k, String v) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text('${k.toUpperCase()} ', style: Type.labelCaps.copyWith(fontSize: 8)),
        Text(v, style: Type.data.copyWith(fontSize: 11)),
      ],
    );
  }
}