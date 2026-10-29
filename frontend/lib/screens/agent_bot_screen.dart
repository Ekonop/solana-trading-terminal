import 'dart:async';
import 'dart:math' as math;
import 'package:flutter/material.dart';

import '../models/models.dart';
import '../state/app_state.dart';

/// Design tokens strictly from /home/vansh/Downloads/design.md §14.
class _GrokColors {
  static const page = Color(0xFFF5F5F7);
  static const surface = Color(0xFFFFFFFF);
  static const textPrimary = Color(0xFF171719);
  static const textSecondary = Color(0xFF85858D);
  static const textMuted = Color(0xFFA8A8AF);
  static const positive = Color(0xFF32C66A);
  static const positiveSoft = Color(0xFFE5F8EB);
  static const negative = Color(0xFFE66B79);
  static const negativeSoft = Color(0xFFFCEBED);
  static const blue = Color(0xFF2788E8);
  static const blueSoft = Color(0xFFE8F2FC);
  static const purple = Color(0xFF9D5BE8);
  static const orange = Color(0xFFF2A52B);
  static const track = Color(0xFFE8E8EC);
  static const toast = Color(0xFF050505);
  static const border = Color(0xFFEEEEF1);

  static const double cardRadius = 20.0;
  static const double pillRadius = 999.0;
}

/// Dynamic event toast specification matching design.md §9.
class _EventToastData {
  final String title;
  final String subtitle;
  final String result;
  final Color resultColor;
  final IconData icon;

  const _EventToastData({
    required this.title,
    required this.subtitle,
    required this.result,
    required this.resultColor,
    required this.icon,
  });
}

/// Autonomous Grok Bot Dashboard Screen.
///
/// Implemented strictly according to `/home/vansh/Downloads/design.md` with:
/// - 100% REAL telemetry data from the backend (no hardcoded numbers)
/// - Single-column responsive layout
/// - Real equity, real trade stats, real win rate, real win streak
/// - Real-time rule counters from strategy signals & risk events
/// - Real coin decision stream evaluated against live market universe
/// - Real equity sparkline drawn from historical database snapshots
/// - Real-time event toast layer derived from fills and risk events
class AgentBotScreen extends StatefulWidget {
  const AgentBotScreen({super.key, required this.state});
  final AppState state;

  @override
  State<AgentBotScreen> createState() => _AgentBotScreenState();
}

class _AgentBotScreenState extends State<AgentBotScreen> {
  _EventToastData? _activeToast;
  Timer? _toastTimer;
  String? _lastFillId;

  @override
  void initState() {
    super.initState();
    widget.state.addListener(_onStateChange);
    // Initial sync
    _checkRealEvents();
  }

  @override
  void dispose() {
    widget.state.removeListener(_onStateChange);
    _toastTimer?.cancel();
    super.dispose();
  }

  void _onStateChange() {
    if (mounted) {
      _checkRealEvents();
    }
  }

  void _checkRealEvents() {
    // Show toast for real fills occurring in the terminal
    final fills = widget.state.fills;
    if (fills.isNotEmpty && fills.first.id != _lastFillId) {
      _lastFillId = fills.first.id;
      final f = fills.first;
      _showToast(_EventToastData(
        title: '${f.side.toUpperCase()} Order Filled · ${f.symbol}',
        subtitle: '${f.quantity} units @ \$${f.price}',
        result: '${f.feeLamports} lamports',
        resultColor: _GrokColors.positive,
        icon: Icons.check_circle_rounded,
      ));
    }
  }

  void _showToast(_EventToastData data) {
    if (!mounted) return;
    setState(() => _activeToast = data);
    _toastTimer?.cancel();
    _toastTimer = Timer(const Duration(seconds: 4), () {
      if (mounted) setState(() => _activeToast = null);
    });
  }

  Future<void> _synthesizeNewRule() async {
    try {
      await widget.state.synthesizeAgentRule('live market telemetry');
      final newRule = widget.state.agentOverview?.rules.firstOrNull;
      if (newRule != null) {
        _showToast(_EventToastData(
          title: 'Rule Synthesized #${newRule.id}',
          subtitle: newRule.title,
          result: 'ACTIVE',
          resultColor: _GrokColors.purple,
          icon: Icons.auto_awesome_rounded,
        ));
      }
    } catch (e) {
      _showToast(_EventToastData(
        title: 'Synthesis Failed',
        subtitle: e.toString(),
        result: 'ERR',
        resultColor: _GrokColors.negative,
        icon: Icons.error_outline_rounded,
      ));
    }
  }

  Future<void> _resetRules() async {
    try {
      await widget.state.resetAgentRules();
      _showToast(const _EventToastData(
        title: 'Rulebook Reset',
        subtitle: 'Reverted to baseline rules',
        result: 'RESET',
        resultColor: _GrokColors.blue,
        icon: Icons.restart_alt_rounded,
      ));
    } catch (e) {
      _showToast(_EventToastData(
        title: 'Reset Failed',
        subtitle: e.toString(),
        result: 'ERR',
        resultColor: _GrokColors.negative,
        icon: Icons.error_outline_rounded,
      ));
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final agent = state.agentOverview;

    if (agent == null) {
      return Container(
        color: _GrokColors.page,
        alignment: Alignment.center,
        child: const Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            CircularProgressIndicator(color: _GrokColors.blue, strokeWidth: 2.5),
            SizedBox(height: 16),
            Text(
              'SYNCING REAL-TIME AGENT TELEMETRY...',
              style: TextStyle(
                fontFamily: 'HackMono',
                fontSize: 12,
                fontWeight: FontWeight.bold,
                color: _GrokColors.textSecondary,
                letterSpacing: 0.5,
              ),
            ),
          ],
        ),
      );
    }

    return Container(
      color: _GrokColors.page,
      child: Stack(
        children: [
          // Scrollable single-column dashboard
          Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 540),
              child: ListView(
                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 16),
                children: [
                  _buildHeader(agent),
                  const SizedBox(height: 12),
                  _buildEquityHero(agent, state.equitySeries),
                  const SizedBox(height: 12),
                  _buildRulebookCard(agent),
                  const SizedBox(height: 12),
                  _buildDecisionsCard(agent),
                  const SizedBox(height: 12),
                  _buildGoalCard(agent),
                  const SizedBox(height: 16),
                  _buildBottomControls(),
                  const SizedBox(height: 32),
                ],
              ),
            ),
          ),

          // Floating event toast banner at top (§9)
          if (_activeToast != null)
            Positioned(
              top: 14,
              left: 16,
              right: 16,
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 540),
                  child: _buildToastWidget(_activeToast!),
                ),
              ),
            ),
        ],
      ),
    );
  }

  // --- §4 Header and status area ---------------------------------------------
  Widget _buildHeader(AgentOverview agent) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      child: Row(
        children: [
          // App icon on left: small, rounded-square light surface with abstract mark
          Container(
            width: 38,
            height: 38,
            decoration: BoxDecoration(
              color: _GrokColors.surface,
              borderRadius: BorderRadius.circular(10),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.04),
                  blurRadius: 6,
                  offset: const Offset(0, 2),
                ),
              ],
            ),
            alignment: Alignment.center,
            child: const Icon(
              Icons.bolt_rounded,
              color: _GrokColors.textPrimary,
              size: 22,
            ),
          ),
          const SizedBox(width: 12),
          // Title & subtitle
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  agent.name,
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                    color: _GrokColors.textPrimary,
                  ),
                ),
                Text(
                  agent.tagline,
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 11.5,
                    color: _GrokColors.textSecondary,
                  ),
                ),
              ],
            ),
          ),
          // Upper-right status: SIM badge + 5 agents
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            decoration: BoxDecoration(
              color: _GrokColors.surface,
              borderRadius: BorderRadius.circular(_GrokColors.pillRadius),
              border: Border.all(color: _GrokColors.border),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
                  decoration: BoxDecoration(
                    color: _GrokColors.textPrimary,
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: const Text(
                    'SIM',
                    style: TextStyle(
                      fontFamily: 'HackMono',
                      fontSize: 9,
                      fontWeight: FontWeight.w800,
                      color: Colors.white,
                    ),
                  ),
                ),
                const SizedBox(width: 6),
                Text(
                  '${agent.agentCount} agents',
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 11.5,
                    fontWeight: FontWeight.w600,
                    color: _GrokColors.textSecondary,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          // Learning status pill with green dot
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
            decoration: BoxDecoration(
              color: _GrokColors.positiveSoft,
              borderRadius: BorderRadius.circular(_GrokColors.pillRadius),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 6,
                  height: 6,
                  decoration: const BoxDecoration(
                    color: _GrokColors.positive,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 5),
                Text(
                  agent.status,
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: _GrokColors.positive,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  // --- §5 Equity summary / performance hero ----------------------------------
  Widget _buildEquityHero(AgentOverview agent, List<EquitySnapshot> series) {
    final isProfit = agent.profitUsd >= 0;

    return Container(
      decoration: BoxDecoration(
        color: _GrokColors.surface,
        borderRadius: BorderRadius.circular(_GrokColors.cardRadius),
        border: Border.all(color: _GrokColors.border, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.03),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // 1. Sim equity meta + Multiplier pill
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
                    decoration: BoxDecoration(
                      color: _GrokColors.textPrimary,
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: const Text(
                      'SIM',
                      style: TextStyle(
                        fontFamily: 'HackMono',
                        fontSize: 9,
                        fontWeight: FontWeight.w800,
                        color: Colors.white,
                      ),
                    ),
                  ),
                  const SizedBox(width: 6),
                  Text(
                    'Sim equity · started with \$${_formatMoney(agent.startedEquityUsd)}',
                    style: const TextStyle(
                      fontFamily: 'FiraSans',
                      fontSize: 11.5,
                      color: _GrokColors.textSecondary,
                    ),
                  ),
                ],
              ),
              // Multiplier pill (bright green fill and white text)
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                decoration: BoxDecoration(
                  color: isProfit ? _GrokColors.positive : _GrokColors.negative,
                  borderRadius: BorderRadius.circular(_GrokColors.pillRadius),
                ),
                child: Text(
                  '${agent.multiplier.toStringAsFixed(2)}×',
                  style: const TextStyle(
                    fontFamily: 'HackMono',
                    fontSize: 13,
                    fontWeight: FontWeight.w800,
                    color: Colors.white,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          // 2. Large current equity value (oversized, bold, tabular)
          Text(
            '\$${_formatMoney(agent.currentEquityUsd)}',
            style: const TextStyle(
              fontFamily: 'HackMono',
              fontSize: 38,
              fontWeight: FontWeight.w800,
              color: _GrokColors.textPrimary,
              letterSpacing: -0.5,
            ),
          ),
          const SizedBox(height: 4),
          // 3. Positive/negative absolute & percentage change, plus activity summary
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '${isProfit ? '+' : ''}${_formatMoney(agent.profitUsd)} (${agent.profitPct.toStringAsFixed(1)}%)',
                style: TextStyle(
                  fontFamily: 'HackMono',
                  fontSize: 14,
                  fontWeight: FontWeight.w700,
                  color: isProfit ? _GrokColors.positive : _GrokColors.negative,
                ),
              ),
              Text(
                '${agent.totalTrades} trades · ${agent.winRatePct.toStringAsFixed(0)}% win · streak ${agent.streak}',
                style: const TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 11.5,
                  fontWeight: FontWeight.w500,
                  color: _GrokColors.textSecondary,
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          // 4. REAL Thin green sparkline with subtle dotted baseline & endpoint dot
          SizedBox(
            height: 38,
            child: CustomPaint(
              painter: _DesignSpecSparklinePainter(
                color: isProfit ? _GrokColors.positive : _GrokColors.negative,
                series: series,
              ),
            ),
          ),
        ],
      ),
    );
  }

  // --- §6 Rulebook card ------------------------------------------------------
  Widget _buildRulebookCard(AgentOverview agent) {
    return Container(
      decoration: BoxDecoration(
        color: _GrokColors.surface,
        borderRadius: BorderRadius.circular(_GrokColors.cardRadius),
        border: Border.all(color: _GrokColors.border, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.03),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Section header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'Rulebook',
                style: TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: _GrokColors.textPrimary,
                ),
              ),
              Text(
                'started with ${agent.rulesStarted} · now ${agent.rulesNow}',
                style: const TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 11.5,
                  color: _GrokColors.textSecondary,
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          // Rule rows
          ListView.separated(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            itemCount: agent.rules.length,
            separatorBuilder: (_, _) => const Divider(
              height: 12,
              thickness: 0.5,
              color: _GrokColors.border,
            ),
            itemBuilder: (context, index) {
              final r = agent.rules[index];
              return _buildRuleRow(r);
            },
          ),
        ],
      ),
    );
  }

  Widget _buildRuleRow(AgentRule rule) {
    final pillColor = _categoryPillColor(rule.color);
    final isBlocked = rule.actionType.toLowerCase() == 'blocked' && rule.count > 0;

    return Container(
      padding: const EdgeInsets.symmetric(vertical: 2),
      decoration: BoxDecoration(
        color: isBlocked ? _GrokColors.negativeSoft.withValues(alpha: 0.4) : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          // 1. Small colored index pill
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
            decoration: BoxDecoration(
              color: pillColor.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(6),
            ),
            child: Text(
              '#${rule.id}',
              style: TextStyle(
                fontFamily: 'HackMono',
                fontSize: 11,
                fontWeight: FontWeight.w800,
                color: pillColor,
              ),
            ),
          ),
          const SizedBox(width: 8),
          // 2. Rule title + provenance
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Flexible(
                      child: Text(
                        rule.title,
                        style: const TextStyle(
                          fontFamily: 'FiraSans',
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: _GrokColors.textPrimary,
                        ),
                      ),
                    ),
                    if (rule.isNew) ...[
                      const SizedBox(width: 6),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                        decoration: BoxDecoration(
                          color: _GrokColors.positiveSoft,
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: const Text(
                          'NEW',
                          style: TextStyle(
                            fontFamily: 'HackMono',
                            fontSize: 9,
                            fontWeight: FontWeight.w800,
                            color: _GrokColors.positive,
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
                Text(
                  rule.provenance,
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 10.5,
                    color: _GrokColors.textSecondary,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          // 3. Right-aligned count with descriptor below
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                '${rule.count}',
                style: const TextStyle(
                  fontFamily: 'HackMono',
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: _GrokColors.textPrimary,
                ),
              ),
              Text(
                rule.actionType,
                style: const TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 10,
                  color: _GrokColors.textSecondary,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  // --- §7 Decisions card -----------------------------------------------------
  Widget _buildDecisionsCard(AgentOverview agent) {
    return Container(
      decoration: BoxDecoration(
        color: _GrokColors.surface,
        borderRadius: BorderRadius.circular(_GrokColors.cardRadius),
        border: Border.all(color: _GrokColors.border, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.03),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Section header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'Decisions',
                style: TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: _GrokColors.textPrimary,
                ),
              ),
              Text(
                '${agent.decisions.length} live coins vs rules',
                style: const TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 11.5,
                  color: _GrokColors.textSecondary,
                ),
              ),
            ],
          ),
          const SizedBox(height: 10),
          // Decision rows
          if (agent.decisions.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Center(
                child: Text(
                  'Awaiting live tokens from market cache...',
                  style: TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 12,
                    color: _GrokColors.textSecondary,
                  ),
                ),
              ),
            )
          else
            ListView.separated(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              itemCount: agent.decisions.length,
              separatorBuilder: (_, _) => const Divider(
                height: 12,
                thickness: 0.5,
                color: _GrokColors.border,
              ),
              itemBuilder: (context, index) {
                final d = agent.decisions[index];
                return _buildDecisionRow(d);
              },
            ),
        ],
      ),
    );
  }

  Widget _buildDecisionRow(CoinDecision d) {
    final isSkip = d.status == 'skip';
    final isPositive = d.pnlPct >= 0;

    return Container(
      padding: const EdgeInsets.symmetric(vertical: 2, horizontal: 4),
      decoration: BoxDecoration(
        color: isSkip ? _GrokColors.negativeSoft.withValues(alpha: 0.4) : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          // 1. Circular token avatar (28 px diameter)
          Container(
            width: 28,
            height: 28,
            decoration: BoxDecoration(
              color: isSkip ? _GrokColors.track : _GrokColors.blueSoft,
              shape: BoxShape.circle,
            ),
            alignment: Alignment.center,
            child: Text(
              d.symbol.replaceAll(r'$', '').characters.take(1).toString(),
              style: TextStyle(
                fontFamily: 'HackMono',
                fontSize: 12,
                fontWeight: FontWeight.w700,
                color: isSkip ? _GrokColors.textSecondary : _GrokColors.blue,
              ),
            ),
          ),
          const SizedBox(width: 10),
          // 2. Token symbol & secondary check details
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      d.symbol,
                      style: const TextStyle(
                        fontFamily: 'HackMono',
                        fontSize: 13,
                        fontWeight: FontWeight.w700,
                        color: _GrokColors.textPrimary,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Flexible(
                      child: Text(
                        d.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontFamily: 'FiraSans',
                          fontSize: 11,
                          color: _GrokColors.textSecondary,
                        ),
                      ),
                    ),
                  ],
                ),
                Text(
                  d.detail,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 10.5,
                    color: isSkip ? _GrokColors.negative : _GrokColors.textSecondary,
                    fontWeight: isSkip ? FontWeight.w600 : FontWeight.normal,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          // 3. Right-aligned outcome: percentage or skip label
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                isSkip ? 'SKIP' : '${isPositive ? '+' : ''}${d.pnlPct.toStringAsFixed(1)}%',
                style: TextStyle(
                  fontFamily: 'HackMono',
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: isSkip ? _GrokColors.textMuted : (isPositive ? _GrokColors.positive : _GrokColors.negative),
                ),
              ),
              Text(
                isSkip ? 'blocked' : 'active',
                style: const TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 10,
                  color: _GrokColors.textSecondary,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  // --- §8 Goal multiplier card -----------------------------------------------
  Widget _buildGoalCard(AgentOverview agent) {
    return Container(
      decoration: BoxDecoration(
        color: _GrokColors.surface,
        borderRadius: BorderRadius.circular(_GrokColors.cardRadius),
        border: Border.all(color: _GrokColors.border, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.03),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'Goal: 2×',
                style: TextStyle(
                  fontFamily: 'FiraSans',
                  fontSize: 16,
                  fontWeight: FontWeight.w700,
                  color: _GrokColors.textPrimary,
                ),
              ),
              Row(
                children: [
                  const Icon(Icons.check_circle_rounded, color: _GrokColors.blue, size: 14),
                  const SizedBox(width: 4),
                  Text(
                    agent.goalStatus,
                    style: const TextStyle(
                      fontFamily: 'FiraSans',
                      fontSize: 11.5,
                      fontWeight: FontWeight.w600,
                      color: _GrokColors.blue,
                    ),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 16),
          // Horizontal milestone stepper track (1.25x - 5x)
          SizedBox(
            height: 48,
            child: Row(
              children: [
                for (int i = 0; i < agent.milestones.length; i++) ...[
                  _buildMilestonePill(agent.milestones[i]),
                  if (i < agent.milestones.length - 1)
                    Expanded(
                      child: Container(
                        height: 2.5,
                        color: agent.milestones[i].reached
                            ? _GrokColors.positive
                            : _GrokColors.track,
                      ),
                    ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildMilestonePill(GoalMilestone milestone) {
    final reached = milestone.reached;
    final isTarget2x = milestone.label.contains('2');

    Color bg = _GrokColors.track;
    Color fg = _GrokColors.textMuted;
    Border? border;

    if (reached) {
      bg = _GrokColors.positiveSoft;
      fg = _GrokColors.positive;
      border = Border.all(color: _GrokColors.positive, width: 1.2);
    } else if (isTarget2x) {
      bg = _GrokColors.blueSoft;
      fg = _GrokColors.blue;
      border = Border.all(color: _GrokColors.blue, width: 1.5);
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(_GrokColors.pillRadius),
        border: border,
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (reached) ...[
            const Icon(Icons.check_rounded, color: _GrokColors.positive, size: 12),
            const SizedBox(width: 3),
          ],
          Text(
            milestone.label,
            style: TextStyle(
              fontFamily: 'HackMono',
              fontSize: 11,
              fontWeight: reached || isTarget2x ? FontWeight.w800 : FontWeight.w600,
              color: fg,
            ),
          ),
        ],
      ),
    );
  }

  // --- §9 Floating event toast widget ----------------------------------------
  Widget _buildToastWidget(_EventToastData toast) {
    return Container(
      decoration: BoxDecoration(
        color: _GrokColors.toast,
        borderRadius: BorderRadius.circular(26),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.28),
            blurRadius: 18,
            offset: const Offset(0, 6),
          ),
        ],
      ),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Row(
        children: [
          // Compact icon on left
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.15),
              shape: BoxShape.circle,
            ),
            child: Icon(toast.icon, color: Colors.white, size: 16),
          ),
          const SizedBox(width: 12),
          // Event title and detail
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  toast.title,
                  style: const TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 12.5,
                    fontWeight: FontWeight.w700,
                    color: Colors.white,
                  ),
                ),
                Text(
                  toast.subtitle,
                  style: TextStyle(
                    fontFamily: 'FiraSans',
                    fontSize: 10.5,
                    color: Colors.white.withValues(alpha: 0.7),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 10),
          // Key result on right
          Text(
            toast.result,
            style: TextStyle(
              fontFamily: 'HackMono',
              fontSize: 14,
              fontWeight: FontWeight.w800,
              color: toast.resultColor,
            ),
          ),
        ],
      ),
    );
  }

  // Interactive controls to test live bot synthesis
  Widget _buildBottomControls() {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        OutlinedButton.icon(
          onPressed: _synthesizeNewRule,
          icon: const Icon(Icons.auto_awesome_rounded, size: 15),
          label: const Text('Synthesize Rule', style: TextStyle(fontFamily: 'FiraSans', fontSize: 12)),
          style: OutlinedButton.styleFrom(
            foregroundColor: _GrokColors.textPrimary,
            side: const BorderSide(color: _GrokColors.border),
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
          ),
        ),
        const SizedBox(width: 12),
        OutlinedButton.icon(
          onPressed: _resetRules,
          icon: const Icon(Icons.restart_alt_rounded, size: 15),
          label: const Text('Reset Rules', style: TextStyle(fontFamily: 'FiraSans', fontSize: 12)),
          style: OutlinedButton.styleFrom(
            foregroundColor: _GrokColors.textPrimary,
            side: const BorderSide(color: _GrokColors.border),
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
          ),
        ),
        const SizedBox(width: 12),
        OutlinedButton.icon(
          onPressed: () => widget.state.refresh(),
          icon: const Icon(Icons.refresh_rounded, size: 15),
          label: const Text('Sync Telemetry', style: TextStyle(fontFamily: 'FiraSans', fontSize: 12)),
          style: OutlinedButton.styleFrom(
            foregroundColor: _GrokColors.textPrimary,
            side: const BorderSide(color: _GrokColors.border),
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
          ),
        ),
      ],
    );
  }

  Color _categoryPillColor(String name) {
    switch (name.toLowerCase()) {
      case 'purple':
        return _GrokColors.purple;
      case 'cyan':
      case 'blue':
        return _GrokColors.blue;
      case 'amber':
      case 'orange':
        return _GrokColors.orange;
      case 'green':
        return _GrokColors.positive;
      default:
        return _GrokColors.textSecondary;
    }
  }

  String _formatMoney(double n) {
    if (n >= 1000) {
      final whole = n.floor();
      final frac = ((n - whole) * 100).round().toString().padLeft(2, '0');
      final s = whole.toString();
      final buf = StringBuffer();
      for (int i = 0; i < s.length; i++) {
        if (i > 0 && (s.length - i) % 3 == 0) buf.write(',');
        buf.write(s[i]);
      }
      return '${buf.toString()}.$frac';
    }
    return n.toStringAsFixed(2);
  }
}

/// Thin green sparkline with subtle dotted baseline and endpoint dot (§5)
/// mapped to real database equity series.
class _DesignSpecSparklinePainter extends CustomPainter {
  _DesignSpecSparklinePainter({required this.color, required this.series});
  final Color color;
  final List<EquitySnapshot> series;

  @override
  void paint(Canvas canvas, Size size) {
    // 1. Dotted baseline
    final basePaint = Paint()
      ..color = _GrokColors.track
      ..strokeWidth = 1.0;
    const double dashWidth = 3;
    const double dashSpace = 3;
    double startX = 0;
    final y = size.height * 0.88;
    while (startX < size.width) {
      canvas.drawLine(Offset(startX, y), Offset(startX + dashWidth, y), basePaint);
      startX += dashWidth + dashSpace;
    }

    // 2. Trend points from real series
    final valid = series.where((s) => s.equityD > 0).toList();
    if (valid.length < 2) {
      final flatPaint = Paint()
        ..color = color
        ..strokeWidth = 2.0;
      canvas.drawLine(Offset(0, size.height * 0.5), Offset(size.width, size.height * 0.5), flatPaint);
      canvas.drawCircle(Offset(size.width, size.height * 0.5), 3.5, flatPaint);
      return;
    }

    final vals = valid.map((s) => s.equityD).toList();
    final minV = vals.reduce(math.min);
    final maxV = vals.reduce(math.max);
    final spanV = (maxV - minV).abs() < 1e-6 ? 1.0 : (maxV - minV);

    final points = <Offset>[];
    for (int i = 0; i < valid.length; i++) {
      final x = (i / (valid.length - 1)) * size.width;
      final normY = (valid[i].equityD - minV) / spanV;
      final yPos = size.height * 0.85 - (normY * size.height * 0.70);
      points.add(Offset(x, yPos));
    }

    final path = Path()..moveTo(points.first.dx, points.first.dy);
    for (int i = 1; i < points.length; i++) {
      path.lineTo(points[i].dx, points[i].dy);
    }

    // Stroke
    final strokePaint = Paint()
      ..color = color
      ..strokeWidth = 2.2
      ..style = PaintingStyle.stroke;
    canvas.drawPath(path, strokePaint);

    // Subtle gradient fill under curve
    final fillPath = Path.from(path)
      ..lineTo(size.width, size.height)
      ..lineTo(0, size.height)
      ..close();

    final fillPaint = Paint()
      ..shader = LinearGradient(
        colors: [color.withValues(alpha: 0.15), color.withValues(alpha: 0.0)],
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
      ).createShader(Rect.fromLTWH(0, 0, size.width, size.height))
      ..style = PaintingStyle.fill;
    canvas.drawPath(fillPath, fillPaint);

    // Small endpoint dot
    final endPoint = points.last;
    final dotPaint = Paint()..color = color;
    canvas.drawCircle(endPoint, 3.5, dotPaint);
  }

  @override
  bool shouldRepaint(covariant _DesignSpecSparklinePainter oldDelegate) => true;
}
