import 'package:flutter/material.dart';

import '../config.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    final s = state.settingsJson;
    final app = s?['application'] is Map<String, dynamic>
        ? s!['application'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final md = s?['market_data'] is Map<String, dynamic>
        ? s!['market_data'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final pt = s?['paper_trading'] is Map<String, dynamic>
        ? s!['paper_trading'] as Map<String, dynamic>
        : const <String, dynamic>{};
    final risk = s?['risk'] is Map<String, dynamic>
        ? s!['risk'] as Map<String, dynamic>
        : const <String, dynamic>{};

    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Text('SETTINGS', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                const SizedBox(width: 12),
                const SimBadge(),
                const Spacer(),
                Text('READ-ONLY CONFIG', style: Type.labelCaps),
              ],
            ),
            const SizedBox(height: 16),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: TermPanel(
                    title: 'APPLICATION',
                    children: [
                      KVRow(k: 'Mode', v: (app['mode'] ?? '—').toString()),
                      KVRow(k: 'Version', v: (app['version'] ?? '—').toString()),
                      KVRow(
                        k: 'Bind',
                        v: '${app['bind_address'] ?? '—'}:${app['port'] ?? '—'}',
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TermPanel(
                    title: 'MARKET DATA',
                    children: [
                      KVRow(k: 'Provider', v: (md['provider'] ?? '—').toString()),
                      KVRow(
                        k: 'Simulated',
                        v: (md['simulated'] ?? md['provider_state'] ?? '—').toString(),
                      ),
                      KVRow(
                        k: 'Refresh',
                        v: '${(md['refresh_interval_ms'] ?? 0) ~/ 1000}s',
                      ),
                      KVRow(k: 'Max Age', v: '${(md['maximum_data_age_ms'] ?? 0) ~/ 1000}s'),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TermPanel(
                    title: 'PAPER TRADING',
                    children: [
                      KVRow(k: 'Fees', v: '${pt['fee_bps'] ?? 0} bps'),
                      KVRow(k: 'Slippage', v: '${pt['slippage_bps'] ?? 0} bps'),
                      KVRow(k: 'Max Notional', v: Fmt.usd((pt['max_order_notional_usd'] ?? '0').toString())),
                      KVRow(k: 'Initial', v: Fmt.sol((pt['initial_balance_lamports'] ?? 0) as int)),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 2,
                  child: TermPanel(
                    title: 'RISK LIMITS',
                    children: [
                      TermTable(
                        columns: const [
                          TermColumn('LIMIT'),
                          TermColumn('VALUE', align: TextAlign.right),
                        ],
                        rows: [
                          for (final e in risk.entries)
                            [
                              Text(e.key.replaceAll('_', ' ').toUpperCase(), style: Type.labelCaps),
                              Text(switch (e.value) {
                                final int i => '$i',
                                final num d => d.toString(),
                                _ => e.value.toString(),
                              }, style: Type.data),
                            ],
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TermPanel(
                    title: 'OPERATIONS',
                    children: [
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: TermButton(
                          label: 'STOP ALL STRATEGIES',
                          color: T.down,
                          icon: Icons.stop_rounded,
                          onPressed: () => state.withCall(() async {
                            await state.api.stopAll();
                          }),
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: TermButton(
                          label: 'RESET DEMO STATE',
                          color: T.warn,
                          icon: Icons.refresh_rounded,
                          onPressed: () => state.withCall(() async {
                            await state.api.demoReset();
                          }),
                        ),
                      ),
                      const Divider(),
                      KVRow(k: 'SIM Boundary', v: 'NO LIVE TRADING', vColor: T.gold),
                      KVRow(k: 'WS', v: state.wsConnected ? 'CONNECTED' : 'DISCONNECTED',
                          vColor: state.wsConnected ? T.up : T.down),
                    ],
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}