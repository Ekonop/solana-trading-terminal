import 'package:flutter/material.dart';

import '../config.dart';
import '../models/models.dart';
import '../state/app_state.dart';
import '../theme.dart';
import '../widgets/widgets.dart';

class PortfolioScreen extends StatelessWidget {
  const PortfolioScreen({super.key, required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        backgroundColor: T.bg,
        body: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
              child: Row(
                children: [
                  Text('PORTFOLIO', style: Type.hero.copyWith(fontSize: 16, letterSpacing: 0.2)),
                  const SizedBox(width: 12),
                  const SimBadge(),
                  const Spacer(),
                  TermButton(
                    label: 'NEW PAPER ORDER',
                    icon: Icons.add_rounded,
                    onPressed: () => _showOrderSheet(context),
                  ),
                ],
              ),
            ),
            Container(
              margin: const EdgeInsets.fromLTRB(16, 10, 16, 0),
              color: T.panel,
              child: const TabBar(
                indicatorColor: T.accent,
                indicatorSize: TabBarIndicatorSize.label,
                labelColor: T.fg,
                unselectedLabelColor: T.fg3,
                labelStyle: Type.labelCaps,
                unselectedLabelStyle: Type.labelCaps,
                dividerColor: Colors.transparent,
                tabs: [
                  Tab(text: 'POSITIONS'),
                  Tab(text: 'ORDERS'),
                  Tab(text: 'FILLS'),
                ],
              ),
            ),
            Expanded(
              child: TabBarView(
                children: [
                  _PositionsTab(state: state),
                  _OrdersTab(state: state),
                  _FillsTab(state: state),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _showOrderSheet(BuildContext context) {
    showDialog<void>(
      context: context,
      builder: (_) => OrderDialog(state: state),
    );
  }
}

class _PositionsTab extends StatelessWidget {
  const _PositionsTab({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    return _ListShell(
      panelTitle: 'OPEN POSITIONS',
      empty: 'NO OPEN POSITIONS',
      table: TermTable(
        columns: const [
          TermColumn('ASSET'),
          TermColumn('DIR'),
          TermColumn('QTY', align: TextAlign.right),
          TermColumn('AVG ENTRY', align: TextAlign.right),
          TermColumn('MARK PRICE', align: TextAlign.right),
          TermColumn('NOTIONAL', align: TextAlign.right),
          TermColumn('uPNL', align: TextAlign.right),
          TermColumn('OPENED'),
          TermColumn('ACTION', align: TextAlign.center),
        ],
        rows: [
          for (final p in state.positions)
            () {
              final quote = state.markets.cast<MarketQuote?>().firstWhere(
                    (m) => m?.mint == p.mint,
                    orElse: () => null,
                  );
              return [
                SymbolCell(symbol: p.symbol, network: quote?.network),
                StatusChip(
                  label: p.direction,
                  color: p.direction.toLowerCase() == 'long' ? T.up : T.down,
                ),
                Text(p.quantity, style: Type.dataBold),
                Text(p.avgEntryPrice, style: Type.data),
                Text(p.currentPrice, style: Type.data),
                Text(Fmt.usd(p.notionalUsd), style: Type.data),
                DirText.fromJson(p.unrealizedPnlUsd, bold: true),
                TimeCell(p.openedAt),
                GestureDetector(
                  onTap: () {
                    showDialog<void>(
                      context: context,
                      builder: (_) => OrderDialog(
                        state: state,
                        initialMint: p.mint,
                      ),
                    );
                  },
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                    decoration: BoxDecoration(
                      color: T.down.withValues(alpha: 0.12),
                      borderRadius: BorderRadius.circular(T.radiusXs),
                      border: Border.all(color: T.down.withValues(alpha: 0.35)),
                    ),
                    child: Text(
                      'EXIT',
                      style: Type.labelCaps.copyWith(
                        color: T.down,
                        fontSize: 9,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                ),
              ];
            }(),
        ],
      ),
    );
  }
}

class _OrdersTab extends StatelessWidget {
  const _OrdersTab({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    return _ListShell(
      panelTitle: 'ORDERS',
      empty: 'NO ORDERS',
      table: TermTable(
        columns: const [
          TermColumn('ID'),
          TermColumn('SYM'),
          TermColumn('SIDE', align: TextAlign.center),
          TermColumn('TYPE', align: TextAlign.center),
          TermColumn('QTY', align: TextAlign.right),
          TermColumn('FILLED', align: TextAlign.right),
          TermColumn('STATUS'),
          TermColumn('CREATED'),
        ],
        rows: [
          for (final o in state.orders)
            [
              Text(
                o.id.length <= 8 ? o.id : o.id.substring(0, 8),
                style: Type.dataDim,
              ),
              SymbolCell(symbol: o.symbol),
              StatusChip(
                label: o.side,
                color: o.side.toLowerCase() == 'buy' ? T.up : T.down,
              ),
              Text(o.orderType.toUpperCase(), style: Type.labelCaps.copyWith(fontSize: 8)),
              Text(o.quantity, style: Type.data),
              Text(o.filledQty, style: Type.data),
              StatusChip(label: o.status, state: o.status),
              TimeCell(o.createdAt),
            ],
        ],
      ),
    );
  }
}

class _FillsTab extends StatelessWidget {
  const _FillsTab({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    return _ListShell(
      panelTitle: 'FILLS',
      empty: 'NO FILLS',
      table: TermTable(
        columns: const [
          TermColumn('T'),
          TermColumn('SYM'),
          TermColumn('SIDE', align: TextAlign.center),
          TermColumn('QTY', align: TextAlign.right),
          TermColumn('PRICE', align: TextAlign.right),
          TermColumn('FEE (LAMPORTS)', align: TextAlign.right),
        ],
        rows: [
          for (final f in state.fills)
            [
              TimeCell(f.filledAt, full: true),
              SymbolCell(symbol: f.symbol),
              StatusChip(
                label: f.side,
                color: f.side.toLowerCase() == 'buy' ? T.up : T.down,
              ),
              Text(f.quantity, style: Type.data),
              Text(f.price, style: Type.data),
              Text(f.feeLamports.toString(), style: Type.dataDim),
            ],
        ],
      ),
    );
  }
}

class _ListShell extends StatelessWidget {
  const _ListShell({required this.panelTitle, required this.empty, required this.table});
  final String panelTitle;
  final String empty;
  final Widget table;

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: TermPanel(title: panelTitle, children: [table]),
    );
  }
}

class OrderDialog extends StatefulWidget {
  const OrderDialog({super.key, required this.state, this.initialMint});
  final AppState state;
  final String? initialMint;

  @override
  State<OrderDialog> createState() => _OrderDialogState();
}

class _OrderDialogState extends State<OrderDialog> {
  late final TextEditingController _mint;
  final _size = TextEditingController(text: '50');
  final _price = TextEditingController();
  String _side = 'BUY';
  String _type = 'market';
  String? _err;
  bool _submitting = false;

  @override
  void initState() {
    super.initState();
    _mint = TextEditingController(
      text: widget.initialMint ??
          (widget.state.markets.isNotEmpty
              ? widget.state.markets.first.mint
              : 'so11111111111111111111111111111111111111112'),
    );
  }

  @override
  void dispose() {
    _mint.dispose();
    _size.dispose();
    _price.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final selectedQuote = widget.state.markets.cast<MarketQuote?>().firstWhere(
          (m) => m?.mint == _mint.text.trim(),
          orElse: () => null,
        );

    return AlertDialog(
      backgroundColor: T.panel,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(T.radius),
        side: const BorderSide(color: T.hairlineStrong),
      ),
      titlePadding: const EdgeInsets.fromLTRB(20, 20, 20, 12),
      contentPadding: const EdgeInsets.symmetric(horizontal: 20),
      actionsPadding: const EdgeInsets.fromLTRB(20, 12, 20, 18),
      title: Row(
        children: [
          Text('NEW PAPER ORDER', style: Type.panelTitle.copyWith(fontSize: 14)),
          const SizedBox(width: 8),
          const SimBadge(),
          const Spacer(),
          if (selectedQuote != null) NetworkBadge(selectedQuote.network),
        ],
      ),
      content: SizedBox(
        width: 440,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Side & Type row
              Row(
                children: [
                  Expanded(
                    child: _seg('SIDE', ['BUY', 'SELL'], _side, (v) => setState(() => _side = v)),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: _seg('TYPE', ['market', 'limit'], _type, (v) => setState(() => _type = v)),
                  ),
                ],
              ),
              const SizedBox(height: 14),

              // Quick token selector
              Text('QUICK SELECT TOKEN', style: Type.labelCaps),
              const SizedBox(height: 6),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: [
                  for (final q in widget.state.markets.take(12))
                    GestureDetector(
                      onTap: () {
                        setState(() {
                          _mint.text = q.mint;
                          if (_type == 'limit' && _price.text.isEmpty) {
                            _price.text = q.price;
                          }
                        });
                      },
                      child: Container(
                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                        decoration: BoxDecoration(
                          color: _mint.text.trim() == q.mint ? T.accent.withValues(alpha: 0.2) : T.input,
                          borderRadius: BorderRadius.circular(T.radiusXs),
                          border: Border.all(
                            color: _mint.text.trim() == q.mint ? T.accent : T.hairline,
                          ),
                        ),
                        child: Text(
                          q.symbol,
                          style: Type.dataBold.copyWith(
                            fontSize: 11,
                            color: _mint.text.trim() == q.mint ? T.accent : T.fg2,
                          ),
                        ),
                      ),
                    ),
                ],
              ),
              const SizedBox(height: 12),

              // Mint input
              TextField(
                controller: _mint,
                style: Type.data.copyWith(fontSize: 12),
                decoration: InputDecoration(
                  labelText: 'TOKEN MINT / ADDRESS',
                  labelStyle: Type.labelCaps,
                  filled: true,
                  fillColor: T.input,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(T.radiusSm),
                    borderSide: const BorderSide(color: T.hairline),
                  ),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(T.radiusSm),
                    borderSide: const BorderSide(color: T.hairline),
                  ),
                ),
                onChanged: (_) => setState(() {}),
              ),
              if (selectedQuote != null) ...[
                const SizedBox(height: 6),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 4),
                  child: Row(
                    children: [
                      Text(
                        '${selectedQuote.name} · Mark: ${Fmt.usd(selectedQuote.price)}',
                        style: Type.body.copyWith(fontSize: 11, color: T.fg3),
                      ),
                      const Spacer(),
                      Text('24h Vol: ${_compactUsd(selectedQuote.volume24hUsd)}',
                          style: Type.body.copyWith(fontSize: 11, color: T.fg3)),
                    ],
                  ),
                ),
              ],
              const SizedBox(height: 12),

              // Size input + quick presets
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Expanded(
                    child: TextField(
                      controller: _size,
                      style: Type.dataBold,
                      keyboardType: const TextInputType.numberWithOptions(decimal: true),
                      decoration: InputDecoration(
                        labelText: 'NOTIONAL (USD)',
                        prefixText: '\$ ',
                        labelStyle: Type.labelCaps,
                        filled: true,
                        fillColor: T.input,
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(T.radiusSm),
                          borderSide: const BorderSide(color: T.hairline),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(T.radiusSm),
                          borderSide: const BorderSide(color: T.hairline),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  for (final amt in ['25', '50', '100', '250'])
                    Padding(
                      padding: const EdgeInsets.only(left: 4),
                      child: GestureDetector(
                        onTap: () => setState(() => _size.text = amt),
                        child: Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
                          decoration: BoxDecoration(
                            color: _size.text == amt ? T.accent.withValues(alpha: 0.2) : T.panel2,
                            borderRadius: BorderRadius.circular(T.radiusXs),
                            border: Border.all(
                              color: _size.text == amt ? T.accent : T.hairline,
                            ),
                          ),
                          child: Text('\$$amt', style: Type.dataBold.copyWith(fontSize: 11)),
                        ),
                      ),
                    ),
                ],
              ),

              if (_type == 'limit') ...[
                const SizedBox(height: 12),
                TextField(
                  controller: _price,
                  style: Type.dataBold,
                  keyboardType: const TextInputType.numberWithOptions(decimal: true),
                  decoration: InputDecoration(
                    labelText: 'LIMIT PRICE (USD)',
                    prefixText: '\$ ',
                    labelStyle: Type.labelCaps,
                    filled: true,
                    fillColor: T.input,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(T.radiusSm),
                      borderSide: const BorderSide(color: T.hairline),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(T.radiusSm),
                      borderSide: const BorderSide(color: T.hairline),
                    ),
                  ),
                ),
              ],

              if (_err != null) ...[
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: T.down.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(T.radiusSm),
                    border: Border.all(color: T.down.withValues(alpha: 0.4)),
                  ),
                  child: Text(_err!, style: Type.data.copyWith(color: T.down, fontSize: 11)),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: Text('CANCEL', style: Type.labelCaps.copyWith(color: T.fg3)),
        ),
        TermButton(
          label: _side == 'BUY' ? 'SUBMIT BUY ORDER' : 'SUBMIT SELL ORDER',
          icon: _side == 'BUY' ? Icons.arrow_upward_rounded : Icons.arrow_downward_rounded,
          onPressed: _submitting ? null : _submit,
        ),
      ],
    );
  }

  Widget _seg(String label, List<String> items, String selected, ValueChanged<String> onPick) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: Type.labelCaps),
        const SizedBox(height: 6),
        Row(
          children: [
            for (final it in items)
              Expanded(
                child: Padding(
                  padding: EdgeInsets.only(right: it == items.last ? 0 : 6),
                  child: GestureDetector(
                    onTap: () => onPick(it),
                    child: Container(
                      alignment: Alignment.center,
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      decoration: BoxDecoration(
                        color: selected == it
                            ? (it == 'BUY'
                                ? T.up.withValues(alpha: 0.2)
                                : (it == 'SELL' ? T.down.withValues(alpha: 0.2) : T.accent.withValues(alpha: 0.2)))
                            : T.input,
                        borderRadius: BorderRadius.circular(T.radiusSm),
                        border: Border.all(
                          color: selected == it
                              ? (it == 'BUY' ? T.up : (it == 'SELL' ? T.down : T.accent))
                              : T.hairline,
                        ),
                      ),
                      child: Text(
                        it.toUpperCase(),
                        style: Type.labelCaps.copyWith(
                          color: selected == it
                              ? (it == 'BUY' ? T.up : (it == 'SELL' ? T.down : T.accent))
                              : T.fg2,
                          fontWeight: selected == it ? FontWeight.w700 : FontWeight.w500,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ],
    );
  }

  static String _compactUsd(String raw) {
    final v = double.tryParse(raw) ?? 0;
    if (v >= 1e9) return '\$${(v / 1e9).toStringAsFixed(2)}B';
    if (v >= 1e6) return '\$${(v / 1e6).toStringAsFixed(2)}M';
    if (v >= 1e3) return '\$${(v / 1e3).toStringAsFixed(2)}K';
    return '\$${v.toStringAsFixed(0)}';
  }

  Future<void> _submit() async {
    setState(() {
      _submitting = true;
      _err = null;
    });
    final body = <String, dynamic>{
      'mint': _mint.text.trim(),
      'side': _side.toLowerCase(),
      'order-type': _type,
      'size-usd': _size.text.trim(),
      if (_type == 'limit' && _price.text.trim().isNotEmpty)
        'limit-price': _price.text.trim(),
    };
    try {
      await widget.state.api.submitOrder(body);
      await widget.state.refresh();
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) {
        setState(() {
          _err = e.toString();
          _submitting = false;
        });
      }
    }
  }
}