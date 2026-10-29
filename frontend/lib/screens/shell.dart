import 'package:flutter/material.dart';

import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';
import 'agent_bot_screen.dart';
import 'backtests_screen.dart';
import 'dashboard_screen.dart';
import 'markets_screen.dart';
import 'portfolio_screen.dart';
import 'research_screen.dart';
import 'settings_screen.dart';
import 'strategies_screen.dart';

class Shell extends StatefulWidget {
  const Shell({super.key, required this.state});
  final AppState state;

  @override
  State<Shell> createState() => _ShellState();
}

const _navTabs = [
  (Icons.space_dashboard_rounded, 'DASHBOARD'),
  (Icons.psychology_rounded, 'AGENT BOT'),
  (Icons.insights_rounded, 'MARKETS'),
  (Icons.account_tree_rounded, 'STRATEGIES'),
  (Icons.folder_open_rounded, 'PORTFOLIO'),
  (Icons.history_rounded, 'BACKTESTS'),
  (Icons.science_rounded, 'RESEARCH'),
  (Icons.tune_rounded, 'SETTINGS'),
];

class _ShellState extends State<Shell> {
  int _tab = 1; // Default to the Grok Agent Bot screen

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final screens = [
      DashboardScreen(state: state),
      AgentBotScreen(state: state),
      MarketsScreen(state: state),
      StrategiesScreen(state: state),
      PortfolioScreen(state: state),
      BacktestsScreen(state: state),
      ResearchScreen(state: state),
      SettingsScreen(state: state),
    ];

    return Scaffold(
      body: Row(
        children: [
          _Sidebar(
            active: _tab,
            connected: state.wsConnected,
            backendUp: state.backendUp,
            onSelect: (i) => setState(() => _tab = i),
          ),
          Expanded(
            child: Column(
              children: [
                if (!state.backendUp)
                  ErrorBanner(
                      message: state.lastError ?? 'NO CONNECTION TO BACKEND'),
                Expanded(child: screens[_tab]),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.active,
    required this.connected,
    required this.backendUp,
    required this.onSelect,
  });

  final int active;
  final bool connected;
  final bool backendUp;
  final ValueChanged<int> onSelect;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 200,
      decoration: BoxDecoration(
        color: T.bgSidebar,
        border: Border(
          right: BorderSide(color: T.hairline, width: 0.8),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const SizedBox(height: 16),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Container(
                  width: 26,
                  height: 26,
                  decoration: BoxDecoration(
                    color: T.accent.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(6),
                  ),
                  child: const Icon(Icons.bolt_rounded, color: T.accent, size: 16),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'GROK BOT',
                    style: Type.panelTitle.copyWith(letterSpacing: 0.3, color: T.fg, fontWeight: FontWeight.bold),
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 2, 16, 14),
            child: Text('AUTONOMOUS TERMINAL',
                style: Type.labelCaps.copyWith(color: T.fg3, fontSize: 8)),
          ),
          const SizedBox(height: 4),
          for (var i = 0; i < _navTabs.length; i++)
            _NavItem(
              index: i,
              icon: _navTabs[i].$1,
              label: _navTabs[i].$2,
              active: active == i,
              onTap: () => onSelect(i),
            ),
          const Spacer(),
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              children: [
                LiveDot(connected: connected && backendUp),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    connected && backendUp ? 'LIVE' : 'OFFLINE',
                    style: Type.labelCaps.copyWith(
                      color: connected && backendUp ? T.accent : T.warn,
                      fontSize: 9,
                    ),
                  ),
                ),
                const SimBadge(),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _NavItem extends StatelessWidget {
  const _NavItem({
    required this.index,
    required this.icon,
    required this.label,
    required this.active,
    required this.onTap,
  });

  final int index;
  final IconData icon;
  final String label;
  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 1.5),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(T.radiusSm),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOut,
          height: 34,
          decoration: BoxDecoration(
            color: active ? T.accent.withValues(alpha: 0.12) : Colors.transparent,
            borderRadius: BorderRadius.circular(T.radiusSm),
          ),
          child: Row(
            children: [
              const SizedBox(width: 12),
              Icon(
                icon,
                size: 15,
                color: active ? T.accent : T.fg3,
              ),
              const SizedBox(width: 11),
              Text(
                label,
                style: Type.labelCaps.copyWith(
                  color: active ? T.accent : T.fg3,
                  fontWeight: active ? FontWeight.bold : FontWeight.w600,
                  letterSpacing: 0.12,
                  fontSize: 10.5,
                ),
              ),
              const Spacer(),
              if (active)
                Padding(
                  padding: const EdgeInsets.only(right: 10),
                  child: Container(
                    width: 5,
                    height: 5,
                    decoration: BoxDecoration(
                      color: T.accent,
                      shape: BoxShape.circle,
                      boxShadow: [
                        BoxShadow(color: T.accent.withValues(alpha: 0.55), blurRadius: 5),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}