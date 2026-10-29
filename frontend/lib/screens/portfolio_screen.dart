import 'package:flutter/material.dart';

import '../config.dart';
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
      builder: (_) => _OrderDialog(state: state),
    );
  }
}

class _PositionsTab extends StatelessWidget {
  const _PositionsTab({required this.state});
  final AppState state;

  @override
  Widget build(BuildContext context) {
    return _ListShell(
      panelTitle: 'POSITIONS',
      empty: 'NO POSITIONS',
      table: TermTable(
        columns: const [
          TermColumn('SYM'),
          TermColumn('DIR'),
          TermColumn('QTY', align: TextAlign.right),
          TermColumn('AVG ENTRY', align: TextAlign.right),
          TermColumn('LAST', align: TextAlign.right),
          TermColumn('NOTIONAL', align: TextAlign.right),
          TermColumn('uPNL', align: TextAlign.right),
          TermColumn('OPENED'),
        ],
        rows: [
          for (final p in state.positions)
            [
              SymbolCell(symbol: p.symbol),
              StatusChip(
                label: p.direction,
                color: p.direction.toLowerCase() == 'long' ? T.up : T.down,
              ),
              Text(p.quantity, style: Type.data),
              Text(p.avgEntryPrice, style: Type.data),
              Text(p.currentPrice, style: Type.data),
              Text(Fmt.usd(p.notionalUsd), style: Type.data),
              DirText.fromJson(p.unrealizedPnlUsd, bold: true),
              TimeCell(p.openedAt),
            ],
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

class _OrderDialog extends StatefulWidget {
  const _OrderDialog({required this.state});
  final AppState state;

  @override
  State<_OrderDialog> createState() => _OrderDialogState();
}

class _OrderDialogState extends State<_OrderDialog> {
  final _mint =
      TextEditingController(text: 'so11111111111111111111111111111111111111112');
  final _size = TextEditingController(text: '50');
  final _price = TextEditingController();
  String _side = 'BUY';
  String _type = 'market';
  String? _err;
  bool _submitting = false;

  @override
  void dispose() {
    _mint.dispose();
    _size.dispose();
    _price.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      backgroundColor: T.panel2,
      shape: const RoundedRectangleBorder(borderRadius: BorderRadius.zero),
      title: Text('NEW PAPER ORDER', style: Type.panelTitle),
      content: SizedBox(
        width: 380,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _seg('SIDE', ['BUY', 'SELL'], _side, (v) => setState(() => _side = v)),
            const SizedBox(height: 10),
            _seg('TYPE', ['market', 'limit'], _type, (v) => setState(() => _type = v)),
            const SizedBox(height: 10),
            TextField(
              controller: _mint,
              style: Type.data,
              decoration: const InputDecoration(labelText: 'MINT'),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: _size,
              style: Type.data,
              decoration: const InputDecoration(labelText: 'SIZE (USD)'),
            ),
            if (_type == 'limit') ...[
              const SizedBox(height: 8),
              TextField(
                controller: _price,
                style: Type.data,
                decoration: const InputDecoration(labelText: 'LIMIT PRICE (USD)'),
              ),
            ],
            if (_err != null) ...[
              const SizedBox(height: 8),
              Text(_err!, style: Type.data.copyWith(color: T.down, fontSize: 10)),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _submitting ? null : () => Navigator.of(context).pop(),
          child: const Text('CANCEL'),
        ),
        TermButton(label: 'SUBMIT', onPressed: _submitting ? null : _submit),
      ],
    );
  }

  Widget _seg(String label, List<String> items, String selected, ValueChanged<String> onPick) {
    return Row(
      children: [
        SizedBox(width: 90, child: Text(label, style: Type.labelCaps)),
        for (final it in items)
          Padding(
            padding: const EdgeInsets.only(right: 6),
            child: GestureDetector(
              onTap: () => onPick(it),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                color: selected == it ? T.accent : T.input,
                child: Text(
                  it.toUpperCase(),
                  style: Type.labelCaps.copyWith(
                    color: selected == it ? T.bg : T.fg2,
                  ),
                ),
              ),
            ),
          ),
      ],
    );
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