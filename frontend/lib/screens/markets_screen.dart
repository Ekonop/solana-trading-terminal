import 'package:flutter/material.dart';

import '../config.dart';
import '../models/models.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';
import 'portfolio_screen.dart';

class MarketsScreen extends StatefulWidget {
  const MarketsScreen({super.key, required this.state});
  final AppState state;

  @override
  State<MarketsScreen> createState() => _MarketsScreenState();
}

class _MarketsScreenState extends State<MarketsScreen> {
  String _selectedNetwork = 'ALL';
  String _searchQuery = '';
  final _searchController = TextEditingController();

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final allMarkets = widget.state.markets;
    final health = widget.state.statusJson?['provider'] is Map<String, dynamic>
        ? widget.state.statusJson!['provider'] as Map<String, dynamic>
        : null;
    final providerName = health?['name']?.toString() ?? '—';
    final providerState = health?['state']?.toString() ?? 'unknown';

    // Unique networks present in market list
    final Set<String> networks = {'ALL'};
    for (final m in allMarkets) {
      networks.add(m.network.toUpperCase());
    }

    // Filter by network and search query
    final filtered = allMarkets.where((m) {
      if (_selectedNetwork != 'ALL' &&
          m.network.toUpperCase() != _selectedNetwork) {
        return false;
      }
      if (_searchQuery.isNotEmpty) {
        final q = _searchQuery.toLowerCase();
        final matchSym = m.symbol.toLowerCase().contains(q);
        final matchName = m.name.toLowerCase().contains(q);
        final matchMint = m.mint.toLowerCase().contains(q);
        if (!matchSym && !matchName && !matchMint) return false;
      }
      return true;
    }).toList();

    // Compute aggregates
    double totalVol = 0;
    double totalLiq = 0;
    MarketQuote? topVolMarket;
    double maxVol = 0;

    for (final m in allMarkets) {
      final vol = double.tryParse(m.volume24hUsd) ?? 0;
      final liq = double.tryParse(m.liquidityUsd) ?? 0;
      totalVol += vol;
      totalLiq += liq;
      if (vol > maxVol) {
        maxVol = vol;
        topVolMarket = m;
      }
    }

    return Scaffold(
      backgroundColor: T.bg,
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Page Header
            Row(
              children: [
                Text(
                  'MULTI-CHAIN MARKETS',
                  style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2),
                ),
                const SizedBox(width: 10),
                const SimBadge(),
                const Spacer(),
                StatusChip(
                  label: '$providerName · $providerState',
                  state: providerState,
                ),
              ],
            ),
            const SizedBox(height: 14),

            // Top Metric Summary Cards
            Row(
              children: [
                Expanded(
                  child: KpiCard(
                    label: 'Tracked Pairs',
                    value: '${allMarkets.length}',
                    delta: '${networks.length - 1} NETWORKS',
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: '24h Global Vol',
                    value: _compactUsd(totalVol.toString()),
                    delta: 'AGGREGATED',
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Total Liquidity',
                    value: _compactUsd(totalLiq.toString()),
                    delta: 'DEX POOLS',
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: KpiCard(
                    label: 'Top Volume Asset',
                    value: topVolMarket?.symbol ?? '—',
                    delta: topVolMarket != null
                        ? _compactUsd(topVolMarket.volume24hUsd)
                        : null,
                    valueColor: T.accent,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),

            // Network Filters & Search Bar Row
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: T.panel,
                borderRadius: BorderRadius.circular(T.radius),
                border: Border.all(color: T.hairline),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      // Network Pills
                      Expanded(
                        child: SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: Row(
                            children: [
                              for (final net in networks)
                                Padding(
                                  padding: const EdgeInsets.only(right: 6),
                                  child: _networkPill(net, allMarkets),
                                ),
                            ],
                          ),
                        ),
                      ),
                      const SizedBox(width: 12),
                      // Search Field
                      SizedBox(
                        width: 200,
                        height: 34,
                        child: TextField(
                          controller: _searchController,
                          style: Type.data.copyWith(fontSize: 12),
                          decoration: InputDecoration(
                            hintText: 'Search asset / mint...',
                            hintStyle: Type.labelCaps.copyWith(color: T.fg4),
                            prefixIcon: const Icon(Icons.search_rounded,
                                size: 14, color: T.fg3),
                            suffixIcon: _searchQuery.isNotEmpty
                                ? GestureDetector(
                                    onTap: () {
                                      setState(() {
                                        _searchController.clear();
                                        _searchQuery = '';
                                      });
                                    },
                                    child: const Icon(Icons.close_rounded,
                                        size: 14, color: T.fg3),
                                  )
                                : null,
                            filled: true,
                            fillColor: T.input,
                            contentPadding: const EdgeInsets.symmetric(
                                horizontal: 10, vertical: 0),
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(T.radiusSm),
                              borderSide: const BorderSide(color: T.hairline),
                            ),
                            enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(T.radiusSm),
                              borderSide: const BorderSide(color: T.hairline),
                            ),
                          ),
                          onChanged: (v) => setState(() => _searchQuery = v.trim()),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),

            // Quotes Panel Table
            TermPanel(
              title: 'MARKET ASSETS',
              trailing: Text(
                '${filtered.length} OF ${allMarkets.length} PAIRS',
                style: Type.labelCaps,
              ),
              children: [
                TermTable(
                  columns: const [
                    TermColumn('ASSET'),
                    TermColumn('PRICE (USD)', align: TextAlign.right),
                    TermColumn('24H VOL', align: TextAlign.right),
                    TermColumn('LIQUIDITY', align: TextAlign.right),
                    TermColumn('STATUS', align: TextAlign.center),
                    TermColumn('AGE', align: TextAlign.center),
                    TermColumn('QUICK TRADE', align: TextAlign.center),
                  ],
                  rows: [
                    for (final m in filtered)
                      [
                        SymbolCell(
                          symbol: m.symbol,
                          name: m.name,
                          network: m.network,
                        ),
                        Text(Fmt.usd(m.price), style: Type.dataBold),
                        Text(_compactUsd(m.volume24hUsd), style: Type.data),
                        Text(_compactUsd(m.liquidityUsd), style: Type.data),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Container(
                              width: 6,
                              height: 6,
                              decoration: BoxDecoration(
                                shape: BoxShape.circle,
                                color: m.stale ? T.warn : T.up,
                              ),
                            ),
                            const SizedBox(width: 4),
                            Text(
                              m.stale ? 'STALE' : 'LIVE',
                              style: Type.labelCaps.copyWith(
                                color: m.stale ? T.warn : T.up,
                                fontSize: 9,
                              ),
                            ),
                          ],
                        ),
                        TimeCell(m.observedAt),
                        GestureDetector(
                          onTap: () {
                            showDialog<void>(
                              context: context,
                              builder: (_) => OrderDialog(
                                state: widget.state,
                                initialMint: m.mint,
                              ),
                            );
                          },
                          child: Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 10, vertical: 4),
                            decoration: BoxDecoration(
                              color: T.accent.withValues(alpha: 0.12),
                              borderRadius: BorderRadius.circular(T.radiusXs),
                              border: Border.all(
                                color: T.accent.withValues(alpha: 0.4),
                              ),
                            ),
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                const Icon(Icons.bolt_rounded,
                                    size: 11, color: T.accent),
                                const SizedBox(width: 3),
                                Text(
                                  'TRADE',
                                  style: Type.labelCaps.copyWith(
                                    color: T.accent,
                                    fontSize: 9.5,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ],
                  ],
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _networkPill(String net, List<MarketQuote> all) {
    final isSelected = _selectedNetwork == net;
    final count = net == 'ALL'
        ? all.length
        : all.where((m) => m.network.toUpperCase() == net).length;
    final col = net == 'ALL' ? T.accent : T.networkColor(net);

    return GestureDetector(
      onTap: () => setState(() => _selectedNetwork = net),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        decoration: BoxDecoration(
          color: isSelected ? col.withValues(alpha: 0.2) : T.input,
          borderRadius: BorderRadius.circular(T.radiusSm),
          border: Border.all(
            color: isSelected ? col : T.hairline,
            width: isSelected ? 1.2 : 1.0,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              net,
              style: Type.labelCaps.copyWith(
                color: isSelected ? col : T.fg2,
                fontWeight: isSelected ? FontWeight.w700 : FontWeight.w500,
                fontSize: 10,
              ),
            ),
            const SizedBox(width: 5),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
              decoration: BoxDecoration(
                color: isSelected ? col.withValues(alpha: 0.3) : T.hairline,
                borderRadius: BorderRadius.circular(T.radiusXs),
              ),
              child: Text(
                '$count',
                style: Type.labelCaps.copyWith(
                  color: isSelected ? T.fg : T.fg3,
                  fontSize: 8.5,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  static String _compactUsd(String raw) {
    final v = double.tryParse(raw) ?? 0;
    if (v >= 1e9) return '\$${(v / 1e9).toStringAsFixed(2)}B';
    if (v >= 1e6) return '\$${(v / 1e6).toStringAsFixed(2)}M';
    if (v >= 1e3) return '\$${(v / 1e3).toStringAsFixed(2)}K';
    return '\$${v.toStringAsFixed(0)}';
  }
}
