import 'package:flutter/material.dart';

import '../config.dart';
import '../theme.dart';

/// Terminal panel: softly raised card with a caps-title bar. DESIGN.md §5.
class TermPanel extends StatelessWidget {
  const TermPanel({
    super.key,
    required this.title,
    this.trailing,
    this.children,
    this.padding = const EdgeInsets.all(14),
    this.minHeight,
  });

  final String title;
  final Widget? trailing;
  final List<Widget>? children;
  final EdgeInsets padding;
  final double? minHeight;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: T.panel,
        borderRadius: BorderRadius.circular(T.radius),
        border: Border.all(color: T.hairline, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.035),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
            decoration: BoxDecoration(
              border: Border(
                bottom: BorderSide(color: T.hairline.withValues(alpha: 0.55), width: 0.5),
              ),
            ),
            child: Row(
              children: [
                Expanded(child: Text(title.toUpperCase(), style: Type.panelTitle)),
                ?trailing,
              ],
            ),
          ),
          Padding(
            padding: padding,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: children ?? const [],
            ),
          ),
        ],
      ),
    );
  }
}

/// Uppercase mono chip with a colored dot — DESIGN.md §5.
class StatusChip extends StatelessWidget {
  const StatusChip({super.key, required this.label, this.state, this.color});

  final String label;
  final String? state;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final c = color ?? (state != null ? StatusTone(state!).color : T.fg3);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: c.withValues(alpha: 0.13),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(width: 5, height: 5, decoration: BoxDecoration(color: c, shape: BoxShape.circle)),
          const SizedBox(width: 5),
          Text(label.toUpperCase(),
              style: Type.labelCaps.copyWith(color: c, letterSpacing: 0.1, fontSize: 9.5)),
        ],
      ),
    );
  }
}

/// KPI: caps label + hero mono value + optional delta pill. DESIGN.md §5.
class KpiCard extends StatelessWidget {
  const KpiCard({
    super.key,
    required this.label,
    required this.value,
    this.delta,
    this.valueColor,
    this.deltaColor,
    this.valueSize,
  });

  final String label;
  final String value;
  final String? delta;
  final Color? valueColor;
  final Color? deltaColor;
  final double? valueSize;

  @override
  Widget build(BuildContext context) {
    final del = delta;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: T.panel,
        borderRadius: BorderRadius.circular(T.radius),
        border: Border.all(color: T.hairline, width: 0.8),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.035),
            blurRadius: 8,
            offset: const Offset(0, 2),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label.toUpperCase(), style: Type.labelCaps),
          const SizedBox(height: 10),
          Text(
            value,
            style: Type.hero.copyWith(color: valueColor ?? T.fg, fontSize: valueSize ?? 19),
            overflow: TextOverflow.ellipsis,
          ),
          if (del != null) ...[
            const SizedBox(height: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: (deltaColor ?? T.fg3).withValues(alpha: 0.13),
                borderRadius: BorderRadius.circular(999),
              ),
              child: Text(
                del,
                style: Type.data.copyWith(color: deltaColor ?? T.fg2, fontSize: 10.5, fontWeight: FontWeight.w600),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// Simple table with right-aligned numeric columns — DESIGN.md §5 tables.
class TermTable extends StatelessWidget {
  const TermTable({
    super.key,
    required this.columns,
    required this.rows,
    this.emptyText = 'NO DATA',
  });

  final List<TermColumn> columns;
  final List<List<Widget>> rows;
  final String emptyText;

  @override
  Widget build(BuildContext context) {
    if (rows.isEmpty) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 18),
        child: Center(
          child: Text(emptyText, style: Type.labelCaps.copyWith(color: T.fg3)),
        ),
      );
    }
    return Table(
      columnWidths: {
        for (var i = 0; i < columns.length; i++) i: columns[i].flex,
      },
      border: TableBorder(
        horizontalInside: BorderSide(color: T.hairline.withValues(alpha: 0.6), width: 0.5),
      ),
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      children: [
        TableRow(
          children: [
            for (final c in columns)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Text(
                  c.header.toUpperCase(),
                  textAlign: c.align,
                  style: Type.labelCaps,
                ),
              ),
          ],
        ),
        for (final row in rows)
          TableRow(
            decoration: const BoxDecoration(),
            children: [
              for (var i = 0; i < columns.length; i++)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: DefaultTextStyle.merge(
                    style: Type.data,
                    textAlign: columns[i].align,
                    child: row[i],
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

class TermColumn {
  const TermColumn(this.header, {this.flex = const FlexColumnWidth(), this.align = TextAlign.left});
  final String header;
  final FlexColumnWidth flex;
  final TextAlign align;
}

/// Colored directional value (up/down/flat) — DESIGN.md §2 rules.
class DirText extends StatelessWidget {
  const DirText(this.text, {super.key, this.dimWhenZero = false, this.fontSize = 12, this.bold = false});

  final String text;
  final bool dimWhenZero;
  final double fontSize;
  final bool bold;

  factory DirText.fromJson(dynamic raw, {double fontSize = 12, bool bold = false}) {
    final v = double.tryParse(raw?.toString() ?? '0') ?? 0;
    return DirText(
      (v > 0 ? '+' : '') + (raw?.toString() ?? '0'),
      fontSize: fontSize,
      bold: bold,
      dimWhenZero: v == 0,
    );
  }

  @override
  Widget build(BuildContext context) {
    final v = double.tryParse(text.replaceAll('+', '')) ?? 0;
    Color c;
    if (v > 0) {
      c = T.up;
    } else if (v < 0) {
      c = T.down;
    } else {
      c = dimWhenZero ? T.fg3 : T.fg2;
    }
    return Text(text, style: Type.data.copyWith(color: c, fontSize: fontSize, fontWeight: bold ? FontWeight.w700 : FontWeight.w400));
  }
}

/// WS live dot — DESIGN.md §5/§7.
class LiveDot extends StatelessWidget {
  const LiveDot({super.key, required this.connected});
  final bool connected;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 6,
      height: 6,
      decoration: BoxDecoration(
        color: connected ? T.accent : T.warn,
        shape: BoxShape.circle,
        boxShadow: connected
            ? [BoxShadow(color: T.accent.withValues(alpha: 0.6), blurRadius: 6)]
            : null,
      ),
    );
  }
}

/// "SIM" marker — everything here is simulated; never hide it. DESIGN.md §6.
class SimBadge extends StatelessWidget {
  const SimBadge({super.key});
  @override
  Widget build(BuildContext context) {
    return StatusChip(label: 'SIM', color: T.gold);
  }
}

/// Error surface for the API status banner.
class ErrorBanner extends StatelessWidget {
  const ErrorBanner({super.key, required this.message});
  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      color: T.down.withValues(alpha: 0.12),
      child: Row(
        children: [
          Icon(Icons.warning_amber_rounded, color: T.down, size: 12),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'BACKEND UNREACHABLE — $message',
              style: Type.labelCaps.copyWith(color: T.down),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

/// Network badge for multichain market identification.
class NetworkBadge extends StatelessWidget {
  const NetworkBadge(this.network, {super.key});
  final String network;

  @override
  Widget build(BuildContext context) {
    final col = T.networkColor(network);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1.5),
      decoration: BoxDecoration(
        color: col.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(T.radiusXs),
        border: Border.all(color: col.withValues(alpha: 0.35), width: 0.8),
      ),
      child: Text(
        network.toUpperCase(),
        style: Type.labelCaps.copyWith(
          color: col,
          fontSize: 8,
          fontWeight: FontWeight.w700,
          letterSpacing: 0.3,
        ),
      ),
    );
  }
}

/// Small symbol/name header used across tables.
class SymbolCell extends StatelessWidget {
  const SymbolCell({super.key, required this.symbol, this.name, this.network});
  final String symbol;
  final String? name;
  final String? network;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(symbol, style: Type.dataBold),
            if (name != null && name!.isNotEmpty)
              Text(name!, style: Type.labelCaps.copyWith(color: T.fg3, fontSize: 8), overflow: TextOverflow.ellipsis),
          ],
        ),
        if (network != null && network!.isNotEmpty) ...[
          const SizedBox(width: 6),
          NetworkBadge(network!),
        ],
      ],
    );
  }
}

/// Time cell in mono, compact.
class TimeCell extends StatelessWidget {
  const TimeCell(this.micros, {super.key, this.full = false});
  final int micros;
  final bool full;

  @override
  Widget build(BuildContext context) {
    return Text(full ? Fmt.microsFull(micros) : Fmt.micros(micros), style: Type.dataDim.copyWith(fontSize: 11));
  }
}

/// Row-striped hover panel for strategy cards.
class BigNumber extends StatelessWidget {
  const BigNumber({super.key, required this.value, this.sub, this.color});
  final String value;
  final String? sub;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(value, style: Type.hero.copyWith(color: color ?? T.fg, fontSize: 18)),
        if (sub != null) ...[
          const SizedBox(height: 2),
          Text(sub!, style: Type.labelCaps),
        ],
      ],
    );
  }
}

/// Tabular label+value row.
class KVRow extends StatelessWidget {
  const KVRow({super.key, required this.k, required this.v, this.vColor});
  final String k;
  final String v;
  final Color? vColor;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        children: [
          SizedBox(width: 120, child: Text(k.toUpperCase(), style: Type.labelCaps)),
          Expanded(child: Text(v, style: Type.data.copyWith(color: vColor ?? T.fg))),
        ],
      ),
    );
  }
}

/// Sign-aware percent chip (pill).
class DeltaChip extends StatelessWidget {
  const DeltaChip({super.key, required this.pct, this.suffix = '%'});
  final double pct;
  final String suffix;

  @override
  Widget build(BuildContext context) {
    final c = pct > 0
        ? T.up
        : pct < 0
            ? T.down
            : T.fg3;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
      decoration: BoxDecoration(
        color: c.withValues(alpha: 0.13),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        '${pct > 0 ? '+' : ''}${pct.toStringAsFixed(2)}$suffix',
        style: Type.data.copyWith(color: c, fontSize: 10.5, fontWeight: FontWeight.w700),
      ),
    );
  }
}